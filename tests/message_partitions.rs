//! Disk build only. With `s3-sync` compiled, `Tables::new` mirrors to S3 and
//! an unconfigured one resolves a placeholder hostname, so opening any table
//! needs DNS before it reaches the partitioning this covers. The partitioning
//! itself does not vary with the feature: only the engine behind it does.
#![cfg(not(feature = "s3-sync"))]

//! A message must come back to the app that stored it after a restart.
//!
//! Partitioning made the slot half of an on-disk name: one app's persisted
//! messages live in `support_message_p<slot>`. That is only safe while a slot
//! means the same app across a restart, which is why the slot registry is
//! persisted and why `Tables::new` replays it to open the partitions it names.
//! Both halves fail silently if broken. An in-memory registry would renumber
//! apps from zero on boot and hand app B the directory app A wrote, and a boot
//! that did not replay the registry would return an empty history rather than
//! an error. So this writes, restarts, and reads.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use support_cafe::config::DatabaseConfig;
use support_cafe::db::schema::app_slot::AppSlotRow;
use support_cafe::db::schema::support_message::SupportMessageRow;
use support_cafe::db::tables::Tables;
use support_cafe::id_types::{NanoId, PackedNanoId};
use worktable::prelude::SelectQueryExecutor;

/// A directory of its own per test, removed on the way in rather than the way
/// out so a failed run leaves its files behind to look at.
fn scratch_dir(name: &str) -> PathBuf {
    static NONCE: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "support-cafe-{name}-{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch dir");
    path
}

fn packed_id() -> PackedNanoId {
    PackedNanoId::pack(&NanoId::new()).expect("pack a fresh nanoid")
}

fn message(app: PackedNanoId, session: PackedNanoId, content: &str) -> SupportMessageRow {
    SupportMessageRow {
        id: 0,
        message_id: packed_id(),
        session_id: session,
        app_public_id: app,
        incoming: true,
        sent_by: "test".to_string(),
        sent_at: 1,
        content: content.to_string(),
        tg_chat_id: None,
    }
}

/// Mint a slot the way `MessageStore::app_slot` does.
async fn mint_slot(tables: &Tables, app: PackedNanoId) -> u32 {
    let slot: u32 = tables.app_slot_table.get_next_pk().into();
    tables
        .app_slot_table
        .insert(AppSlotRow {
            slot,
            app_public_id: app,
        })
        .await
        .expect("mint a slot");
    slot
}

#[test]
fn partitions_survive_a_restart_under_the_same_app() {
    nagoya::block_on(partitions_survive_a_restart_under_the_same_app_body());
}

async fn partitions_survive_a_restart_under_the_same_app_body() {
    let path = scratch_dir("restart");
    let config = DatabaseConfig { path: path.clone() };

    let app_a = packed_id();
    let app_b = packed_id();
    let session_a = packed_id();
    let session_b = packed_id();

    let (slot_a, slot_b) = {
        let tables = Tables::new(config.clone()).await.expect("open");

        let slot_a = mint_slot(&tables, app_a).await;
        let slot_b = mint_slot(&tables, app_b).await;
        assert_ne!(slot_a, slot_b, "two apps must not share a slot");

        for (slot, app, session, content) in [
            (slot_a, app_a, session_a, "a's message"),
            (slot_b, app_b, session_b, "b's message"),
        ] {
            let table = tables
                .support_message_table
                .get_or_create(slot)
                .await
                .expect("open a partition");
            let mut row = message(app, session, content);
            row.id = table.get_next_pk().into();
            table.insert(row).await.expect("insert");
        }

        tables.wait_for_ops().await.expect("flush");
        (slot_a, slot_b)
    };

    // Restart.
    let tables = Tables::new(config).await.expect("reopen");

    assert_eq!(
        tables
            .app_slot_table
            .select_by_app_public_id(app_a)
            .expect("app a's slot survived")
            .slot,
        slot_a,
    );
    assert_eq!(
        tables
            .app_slot_table
            .select_by_app_public_id(app_b)
            .expect("app b's slot survived")
            .slot,
        slot_b,
    );

    for (slot, session, expected) in [
        (slot_a, session_a, "a's message"),
        (slot_b, session_b, "b's message"),
    ] {
        // `partition`, not `get_or_create`: the boot replay is what has to have
        // opened this, and creating it here would hide the failure.
        let table = tables
            .support_message_table
            .partition(slot)
            .expect("boot opened the partition the registry named");
        let rows = table
            .select_by_session_id(session)
            .execute()
            .expect("select");
        assert_eq!(rows.len(), 1, "slot {slot} holds exactly its own message");
        assert_eq!(rows[0].content, expected);
    }

    // And no app's partition holds another's rows.
    let a = tables.support_message_table.partition(slot_a).unwrap();
    assert!(
        a.select_by_session_id(session_b)
            .execute()
            .expect("select")
            .is_empty(),
        "app a's partition must not hold app b's messages",
    );

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn opening_a_partition_twice_yields_one_table() {
    nagoya::block_on(opening_a_partition_twice_yields_one_table_body());
}

async fn opening_a_partition_twice_yields_one_table_body() {
    let path = scratch_dir("race");
    let tables = Tables::new(DatabaseConfig { path: path.clone() })
        .await
        .expect("open");

    let app = packed_id();
    let slot = mint_slot(&tables, app).await;

    let first = tables
        .support_message_table
        .get_or_create(slot)
        .await
        .expect("open");
    let second = tables
        .support_message_table
        .get_or_create(slot)
        .await
        .expect("open again");

    let session = packed_id();
    let mut row = message(app, session, "once");
    row.id = first.get_next_pk().into();
    first.insert(row).await.expect("insert");

    // If the second call had opened a second table over the same files, this
    // would be empty and the two would be racing each other's writes to disk.
    assert_eq!(
        second
            .select_by_session_id(session)
            .execute()
            .expect("select")
            .len(),
        1,
    );

    tables.wait_for_ops().await.expect("flush");
    let _ = std::fs::remove_dir_all(&path);
}
