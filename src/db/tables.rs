use std::sync::Arc;

use worktable::PersistedWorkTable;
use worktable::persistence::PersistenceEngine;
use worktable::prelude::{DiskConfig, SelectQueryExecutor};

#[cfg(feature = "s3-sync")]
use worktable::prelude::{S3Config as WtS3Config, S3DiskConfig};

use crate::config::DatabaseConfig;
#[cfg(feature = "s3-sync")]
use crate::config::S3Config;
use crate::db::message_partitions::{MessagePartitionFactory, PersistedMessages};
use crate::db::schema::app_config::AppConfigWorkTable;
use crate::db::schema::app_member::AppMemberWorkTable;
use crate::db::schema::app_slot::AppSlotWorkTable;
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_info::SupportInfoWorkTable;
use crate::db::schema::support_memory_message::SupportMemoryMessagePartitions;
use crate::db::schema::user::UserWorkTable;

#[cfg(not(feature = "s3-sync"))]
use crate::db::schema::{
    app_config::AppConfigPersistenceEngine, app_member::AppMemberPersistenceEngine,
    app_slot::AppSlotPersistenceEngine, chat_session::ChatSessionPersistenceEngine,
    support_info::SupportInfoPersistenceEngine, user::UserPersistenceEngine,
};

#[cfg(feature = "s3-sync")]
use crate::db::schema::{
    app_config::AppConfigS3SyncPersistenceEngine, app_member::AppMemberS3SyncPersistenceEngine,
    app_slot::AppSlotS3SyncPersistenceEngine, chat_session::ChatSessionS3SyncPersistenceEngine,
    support_info::SupportInfoS3SyncPersistenceEngine, user::UserS3SyncPersistenceEngine,
};

#[derive(Debug)]
pub struct Tables {
    pub app_config_table: Arc<AppConfigWorkTable>,
    pub app_slot_table: Arc<AppSlotWorkTable>,
    pub app_member_table: Arc<AppMemberWorkTable>,
    pub chat_session_table: Arc<ChatSessionWorkTable>,
    pub support_message_table: Arc<PersistedMessages>,
    pub support_memory_message_table: Arc<SupportMemoryMessagePartitions>,
    pub support_info_table: Arc<SupportInfoWorkTable>,
    pub user_table: Arc<UserWorkTable>,
}

/// Open every persisted message partition the slot registry knows about.
///
/// A partitioned table is created empty: the router has no idea which slots
/// exist until something asks for one, and asking is what opens the files. A
/// process that only opened partitions on demand would still serve every app
/// correctly, but it would discover a corrupt or unreadable partition on the
/// first message for that app rather than at boot, and `wait_for_ops` on
/// shutdown would only drain the apps that happened to be active. The registry
/// is the list of slots that exist, so this replays it.
async fn open_message_partitions(
    registry: &AppSlotWorkTable,
    messages: &PersistedMessages,
) -> eyre::Result<()> {
    for row in registry.select_all().execute()? {
        messages.get_or_create(row.slot).await?;
    }
    Ok(())
}

impl Tables {
    #[cfg(not(feature = "s3-sync"))]
    pub async fn new(config: DatabaseConfig) -> eyre::Result<Self> {
        let db_path = config.path.to_string_lossy().to_string();

        macro_rules! disk_load {
            ($Engine:ty, $Table:ty) => {{
                let cfg = DiskConfig::new_with_table_name(
                    db_path.clone(),
                    <$Table>::name_snake_case(),
                    <$Table>::version(),
                );
                let engine = <$Engine>::new(cfg).await?;
                Arc::new(<$Table>::load(engine).await?)
            }};
        }

        let app_config_table = disk_load!(AppConfigPersistenceEngine, AppConfigWorkTable);
        let app_slot_table = disk_load!(AppSlotPersistenceEngine, AppSlotWorkTable);
        let app_member_table = disk_load!(AppMemberPersistenceEngine, AppMemberWorkTable);
        let chat_session_table = disk_load!(ChatSessionPersistenceEngine, ChatSessionWorkTable);
        let support_message_table = Arc::new(PersistedMessages::new(MessagePartitionFactory::new(
            db_path.clone(),
        )));
        open_message_partitions(&app_slot_table, &support_message_table).await?;
        let support_memory_message_table = Arc::new(SupportMemoryMessagePartitions::new());
        let support_info_table = disk_load!(SupportInfoPersistenceEngine, SupportInfoWorkTable);
        let user_table = disk_load!(UserPersistenceEngine, UserWorkTable);

        Ok(Self {
            app_config_table,
            app_slot_table,
            app_member_table,
            chat_session_table,
            support_message_table,
            support_memory_message_table,
            support_info_table,
            user_table,
        })
    }

    #[cfg(feature = "s3-sync")]
    pub async fn new(config: DatabaseConfig, s3: &S3Config) -> eyre::Result<Self> {
        if s3.is_configured() {
            let wt_s3 = WtS3Config {
                bucket_name: s3.bucket_name.clone(),
                endpoint: s3.endpoint.clone(),
                access_key: s3.access_key.clone().unwrap(),
                secret_key: s3.secret_key.clone().unwrap(),
                region: None,
                prefix: Some(s3.prefix.clone()),
            };
            Self::new_s3(config, wt_s3).await
        } else {
            // Placeholder credentials: the S3 mirror fails, the disk half works.
            Self::new_s3(
                config,
                WtS3Config {
                    bucket_name: "placeholder".to_string(),
                    endpoint: "https://placeholder.local".to_string(),
                    access_key: "placeholder".to_string(),
                    secret_key: "placeholder".to_string(),
                    region: None,
                    prefix: None,
                },
            )
            .await
        }
    }

    #[cfg(feature = "s3-sync")]
    async fn new_s3(config: DatabaseConfig, wt_s3: WtS3Config) -> eyre::Result<Self> {
        let db_path = config.path.to_string_lossy().to_string();

        macro_rules! s3_load {
            ($Engine:ty, $Table:ty) => {{
                let cfg = S3DiskConfig {
                    disk: DiskConfig::new_with_table_name(
                        db_path.clone(),
                        <$Table>::name_snake_case(),
                        <$Table>::version(),
                    ),
                    s3: wt_s3.clone(),
                };
                let engine = <$Engine>::new(cfg).await?;
                Arc::new(<$Table>::load(engine).await?)
            }};
        }

        let app_config_table = s3_load!(AppConfigS3SyncPersistenceEngine, AppConfigWorkTable);
        let app_slot_table = s3_load!(AppSlotS3SyncPersistenceEngine, AppSlotWorkTable);
        let app_member_table = s3_load!(AppMemberS3SyncPersistenceEngine, AppMemberWorkTable);
        let chat_session_table = s3_load!(ChatSessionS3SyncPersistenceEngine, ChatSessionWorkTable);
        let support_message_table = Arc::new(PersistedMessages::new(MessagePartitionFactory::new(
            db_path.clone(),
            wt_s3.clone(),
        )));
        open_message_partitions(&app_slot_table, &support_message_table).await?;
        let support_memory_message_table = Arc::new(SupportMemoryMessagePartitions::new());
        let support_info_table = s3_load!(SupportInfoS3SyncPersistenceEngine, SupportInfoWorkTable);
        let user_table = s3_load!(UserS3SyncPersistenceEngine, UserWorkTable);

        Ok(Self {
            app_config_table,
            app_slot_table,
            app_member_table,
            chat_session_table,
            support_message_table,
            support_memory_message_table,
            support_info_table,
            user_table,
        })
    }

    pub async fn wait_for_ops(&self) -> worktable::persistence::PersistenceResult {
        let (app_config, app_slot, app_member, chat_session, support_info, user) = futures::join!(
            self.app_config_table.wait_for_ops(),
            self.app_slot_table.wait_for_ops(),
            self.app_member_table.wait_for_ops(),
            self.chat_session_table.wait_for_ops(),
            self.support_info_table.wait_for_ops(),
            self.user_table.wait_for_ops(),
        );
        app_config?;
        app_slot?;
        app_member?;
        chat_session?;
        support_info?;
        user?;

        self.support_message_table.wait_for_ops().await
    }
}
