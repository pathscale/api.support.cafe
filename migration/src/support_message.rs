use support_cafe::id_types::{NanoId, PackedNanoId};
use worktable::migration::Migration;
use worktable::migration_engine;
use worktable::prelude::*;
use worktable::worktable;

worktable!(
    name: SupportMessage,
    version: 2,
    persist: true,
    columns: {
        id: i64 primary_key autoincrement,
        message_id: PackedNanoId,
        session_id: PackedNanoId,
        app_public_id: PackedNanoId,
        incoming: bool,
        sent_by: String,
        sent_at: i64,
        content: String,
        tg_chat_id: i64 optional,
    },
    indexes: {
        message_id_idx: message_id unique,
        session_id_idx: session_id,
        app_public_id_idx: app_public_id,
        sent_at_idx: sent_at,
    },
);

mod v1 {
    use super::PackedNanoId;
    use worktable::prelude::*;
    use worktable::worktable_version;

    worktable_version!(
        name: SupportMessage,
        version: 1,
        columns: {
            id: i64 primary_key autoincrement,
            session_id: PackedNanoId,
            app_public_id: PackedNanoId,
            incoming: bool,
            sent_by: String,
            sent_at: i64,
            content: String,
            tg_chat_id: i64 optional,
        },
        indexes: {
            session_id_idx: session_id,
        },
    );
}

#[derive(Default)]
pub struct Context;

pub struct Migrator;

impl Migration<v1::SupportMessageRow, SupportMessageRow> for Migrator {
    type Context = Context;

    fn migrate(row: v1::SupportMessageRow, _ctx: &Self::Context) -> SupportMessageRow {
        SupportMessageRow {
            id: row.id,
            message_id: new_message_id(),
            session_id: row.session_id,
            app_public_id: row.app_public_id,
            incoming: row.incoming,
            sent_by: row.sent_by,
            sent_at: row.sent_at,
            content: row.content,
            tg_chat_id: row.tg_chat_id,
        }
    }
}

fn new_message_id() -> PackedNanoId {
    PackedNanoId::pack(&NanoId::new()).expect("generated nanoid packs")
}

migration_engine!(
    migration: Migrator,
    current: SupportMessageWorkTable,
    ctx: Context,
    version_tables: {
        1 => v1::SupportMessageWorkTable,
    },
);
