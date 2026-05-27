use std::sync::Arc;

use worktable::PersistedWorkTable;
use worktable::persistence::PersistenceEngine;
use worktable::prelude::DiskConfig;

#[cfg(feature = "s3-sync")]
use worktable::prelude::{S3Config as WtS3Config, S3DiskConfig};

use crate::config::DatabaseConfig;
#[cfg(feature = "s3-sync")]
use crate::config::S3Config;
use crate::db::schema::app_config::AppConfigWorkTable;
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_message::SupportMessageWorkTable;
use crate::db::schema::support_user::SupportUserWorkTable;
use crate::db::schema::user::UserWorkTable;

#[cfg(not(feature = "s3-sync"))]
use crate::db::schema::{
    app_config::AppConfigPersistenceEngine,
    chat_session::ChatSessionPersistenceEngine,
    support_message::SupportMessagePersistenceEngine,
    support_user::SupportUserPersistenceEngine,
    user::UserPersistenceEngine,
};

#[cfg(feature = "s3-sync")]
use crate::db::schema::{
    app_config::AppConfigS3SyncPersistenceEngine,
    chat_session::ChatSessionS3SyncPersistenceEngine,
    support_message::SupportMessageS3SyncPersistenceEngine,
    support_user::SupportUserS3SyncPersistenceEngine,
    user::UserS3SyncPersistenceEngine,
};

#[derive(Debug)]
pub struct Tables {
    pub app_config_table: Arc<AppConfigWorkTable>,
    pub support_user_table: Arc<SupportUserWorkTable>,
    pub chat_session_table: Arc<ChatSessionWorkTable>,
    pub support_message_table: Arc<SupportMessageWorkTable>,
    pub user_table: Arc<UserWorkTable>,
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
        let support_user_table = disk_load!(SupportUserPersistenceEngine, SupportUserWorkTable);
        let chat_session_table = disk_load!(ChatSessionPersistenceEngine, ChatSessionWorkTable);
        let support_message_table = disk_load!(SupportMessagePersistenceEngine, SupportMessageWorkTable);
        let user_table = disk_load!(UserPersistenceEngine, UserWorkTable);

        Ok(Self {
            app_config_table,
            support_user_table,
            chat_session_table,
            support_message_table,
            user_table,
        })
    }

    #[cfg(feature = "s3-sync")]
    pub async fn new(config: DatabaseConfig, s3: &S3Config) -> eyre::Result<Self> {
        if s3.is_configured() {
            let db_path = config.path.to_string_lossy().to_string();
            let wt_s3 = WtS3Config {
                bucket_name: s3.bucket_name.clone(),
                endpoint: s3.endpoint.clone(),
                access_key: s3.access_key.clone().unwrap(),
                secret_key: s3.secret_key.clone().unwrap(),
                region: None,
                prefix: Some(s3.prefix.clone()),
            };

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
            let support_user_table = s3_load!(SupportUserS3SyncPersistenceEngine, SupportUserWorkTable);
            let chat_session_table = s3_load!(ChatSessionS3SyncPersistenceEngine, ChatSessionWorkTable);
            let support_message_table = s3_load!(SupportMessageS3SyncPersistenceEngine, SupportMessageWorkTable);
            let user_table = s3_load!(UserS3SyncPersistenceEngine, UserWorkTable);

            Ok(Self {
                app_config_table,
                support_user_table,
                chat_session_table,
                support_message_table,
                user_table,
            })
        } else {
            Self::new_disk_only(config).await
        }
    }

    #[cfg(feature = "s3-sync")]
    async fn new_disk_only(config: DatabaseConfig) -> eyre::Result<Self> {
        let db_path = config.path.to_string_lossy().to_string();

        // Use placeholder S3 config - sync will fail but disk ops will work
        let placeholder_s3 = WtS3Config {
            bucket_name: "placeholder".to_string(),
            endpoint: "https://placeholder.local".to_string(),
            access_key: "placeholder".to_string(),
            secret_key: "placeholder".to_string(),
            region: None,
            prefix: None,
        };

        macro_rules! s3_load {
            ($Engine:ty, $Table:ty) => {{
                let cfg = S3DiskConfig {
                    disk: DiskConfig::new_with_table_name(
                        db_path.clone(),
                        <$Table>::name_snake_case(),
                        <$Table>::version(),
                    ),
                    s3: placeholder_s3.clone(),
                };
                let engine = <$Engine>::new(cfg).await?;
                Arc::new(<$Table>::load(engine).await?)
            }};
        }

        let app_config_table = s3_load!(AppConfigS3SyncPersistenceEngine, AppConfigWorkTable);
        let support_user_table = s3_load!(SupportUserS3SyncPersistenceEngine, SupportUserWorkTable);
        let chat_session_table = s3_load!(ChatSessionS3SyncPersistenceEngine, ChatSessionWorkTable);
        let support_message_table = s3_load!(SupportMessageS3SyncPersistenceEngine, SupportMessageWorkTable);
        let user_table = s3_load!(UserS3SyncPersistenceEngine, UserWorkTable);

        Ok(Self {
            app_config_table,
            support_user_table,
            chat_session_table,
            support_message_table,
            user_table,
        })
    }

    pub async fn wait_for_ops(&self) {
        self.app_config_table.wait_for_ops().await;
        self.support_user_table.wait_for_ops().await;
        self.chat_session_table.wait_for_ops().await;
        self.support_message_table.wait_for_ops().await;
        self.user_table.wait_for_ops().await;
    }
}