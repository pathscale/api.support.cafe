use std::sync::Arc;

use worktable::PersistedWorkTable;
use worktable::persistence::PersistenceEngine;
use worktable::prelude::DiskConfig;

use crate::config::DatabaseConfig;
use crate::db::schema::app_config::AppConfigWorkTable;
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_message::SupportMessageWorkTable;
use crate::db::schema::support_user::SupportUserWorkTable;

use crate::db::schema::{
    app_config::AppConfigPersistenceEngine,
    chat_session::ChatSessionPersistenceEngine,
    support_message::SupportMessagePersistenceEngine,
    support_user::SupportUserPersistenceEngine,
};

#[derive(Debug)]
pub struct Tables {
    pub app_config_table: Arc<AppConfigWorkTable>,
    pub support_user_table: Arc<SupportUserWorkTable>,
    pub chat_session_table: Arc<ChatSessionWorkTable>,
    pub support_message_table: Arc<SupportMessageWorkTable>,
}

impl Tables {
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

        Ok(Self {
            app_config_table,
            support_user_table,
            chat_session_table,
            support_message_table,
        })
    }

    pub async fn wait_for_ops(&self) {
        self.app_config_table.wait_for_ops().await;
        self.support_user_table.wait_for_ops().await;
        self.chat_session_table.wait_for_ops().await;
        self.support_message_table.wait_for_ops().await;
    }
}
