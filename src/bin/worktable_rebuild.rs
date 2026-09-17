use std::path::Path;

use eyre::{Result, WrapErr};
use support_cafe::config;
use support_cafe::db::message_partitions::partition_table_name;
use support_cafe::db::schema::{
    app_config::{AppConfigS3SyncPersistenceEngine, AppConfigWorkTable},
    app_member::{AppMemberS3SyncPersistenceEngine, AppMemberWorkTable},
    app_slot::{AppSlotS3SyncPersistenceEngine, AppSlotWorkTable},
    chat_session::{ChatSessionS3SyncPersistenceEngine, ChatSessionWorkTable},
    support_info::{SupportInfoS3SyncPersistenceEngine, SupportInfoWorkTable},
    support_message::{SupportMessageS3SyncPersistenceEngine, SupportMessageWorkTable},
    user::{UserS3SyncPersistenceEngine, UserWorkTable},
};
use worktable::PersistedWorkTable;
use worktable::persistence::{LoadMode, PersistenceEngine};
use worktable::prelude::{
    DiskConfig, PrimaryKeyGeneratorState, S3Config, S3DiskConfig, SelectQueryExecutor,
};

fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

    let config = config::load()?;
    eyre::ensure!(
        config.s3.is_configured(),
        "S3 credentials are not configured"
    );
    let target_prefix = std::env::var("WORKTABLE_REBUILD_TARGET_PREFIX")
        .wrap_err("WORKTABLE_REBUILD_TARGET_PREFIX must name a new S3 prefix")?;
    eyre::ensure!(
        target_prefix != config.s3.prefix,
        "target S3 prefix must differ from the source prefix"
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    runtime.block_on(rebuild(config, target_prefix))
}

async fn rebuild(config: config::Config, target_prefix: String) -> Result<()> {
    let source_root = Path::new("/tmp/worktable-rebuild/source")
        .to_string_lossy()
        .into_owned();
    let target_root = Path::new("/tmp/worktable-rebuild/target")
        .to_string_lossy()
        .into_owned();
    let source_s3 = S3Config {
        bucket_name: config.s3.bucket_name,
        endpoint: config.s3.endpoint,
        access_key: config.s3.access_key.expect("checked above"),
        secret_key: config.s3.secret_key.expect("checked above"),
        region: None,
        prefix: Some(config.s3.prefix),
    };
    let target_s3 = S3Config {
        prefix: Some(target_prefix.clone()),
        ..source_s3.clone()
    };

    println!("rebuilding persisted tables into S3 prefix {target_prefix}");

    // Rebuild the table stored under an explicit directory name, and hand back
    // its rows. The name is a parameter rather than `name_snake_case()` because
    // `support_message` is no longer one directory: it is partitioned, one
    // directory per app, named by slot.
    macro_rules! rebuild_named {
        ($label:expr, $engine:ty, $table:ty, $name:expr) => {{
            let table_name: String = $name;
            let source_config = S3DiskConfig {
                disk: DiskConfig::new_with_table_name(
                    source_root.clone(),
                    &table_name,
                    <$table>::version(),
                ),
                s3: source_s3.clone(),
            };
            let source_engine = <$engine>::new(source_config).await?;
            let source = <$table>::load_with(source_engine, LoadMode::Recovery).await?;
            let rows = source.select_all().execute()?;
            let pk_state = source.pk_gen_state();

            let target_config = S3DiskConfig {
                disk: DiskConfig::new_with_table_name(
                    target_root.clone(),
                    &table_name,
                    <$table>::version(),
                ),
                s3: target_s3.clone(),
            };
            let target_engine = <$engine>::new(target_config).await?;
            let mut target = <$table>::new(target_engine).await?;
            target.0.pk_gen = PrimaryKeyGeneratorState::from_state(pk_state);
            let row_count = rows.len();
            for row in &rows {
                target.insert(row.clone()).await?;
            }
            target.wait_for_ops().await?;
            println!("rebuilt {} rows={row_count}", $label);
            rows
        }};
    }

    macro_rules! rebuild_table {
        ($label:literal, $engine:ty, $table:ty) => {{
            rebuild_named!(
                $label,
                $engine,
                $table,
                <$table>::name_snake_case().to_string()
            )
        }};
    }

    rebuild_table!(
        "app_config",
        AppConfigS3SyncPersistenceEngine,
        AppConfigWorkTable
    );
    rebuild_table!(
        "app_member",
        AppMemberS3SyncPersistenceEngine,
        AppMemberWorkTable
    );
    rebuild_table!(
        "chat_session",
        ChatSessionS3SyncPersistenceEngine,
        ChatSessionWorkTable
    );
    // The slot registry first: it is the list of message partitions that exist,
    // and without it the rebuilt prefix would have message directories no
    // process could route to.
    let app_slots = rebuild_table!("app_slot", AppSlotS3SyncPersistenceEngine, AppSlotWorkTable);
    for row in &app_slots {
        rebuild_named!(
            format!("support_message_p{}", row.slot),
            SupportMessageS3SyncPersistenceEngine,
            SupportMessageWorkTable,
            partition_table_name(row.slot)
        );
    }
    rebuild_table!(
        "support_info",
        SupportInfoS3SyncPersistenceEngine,
        SupportInfoWorkTable
    );
    rebuild_table!("user", UserS3SyncPersistenceEngine, UserWorkTable);

    println!("rebuild complete prefix={target_prefix}");
    Ok(())
}
