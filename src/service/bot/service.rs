use std::collections::HashMap;
use std::sync::Arc;

use tracing::info;
use tokio::time::{sleep, Duration};

use crate::db::schema::app_config::AppConfigRow;
use crate::id_types::AppPublicId;
use crate::id_types::SessionId;
use crate::service::app::AppService;
use crate::service::bot::router::{BotRouter, BotStatus, SupportEventStream};

pub struct BotService {
    bot_router: Arc<BotRouter>,
    app_service: Arc<AppService>,
}

impl BotService {
    pub fn new(support_user_table: Arc<crate::db::schema::support_user::SupportUserWorkTable>,
               support_message_table: Arc<crate::db::schema::support_message::SupportMessageWorkTable>,
               app_service: Arc<AppService>) -> Self {
        let bot_router = Arc::new(BotRouter::new(support_user_table, support_message_table));
        Self { bot_router, app_service }
    }

    pub async fn take_event_stream(&self) -> eyre::Result<SupportEventStream> {
        self.bot_router.take_event_stream().await
    }

    pub async fn register_bot(&self, app_public_id: AppPublicId, token: String) -> eyre::Result<()> {
        self.bot_router.register_bot(app_public_id, token).await
    }

    pub async fn unregister_bot(&self, app_public_id: AppPublicId) {
        self.bot_router.unregister_bot(app_public_id).await
    }

    pub async fn bootstrap_bots(&self) -> eyre::Result<()> {
        let apps = self.app_service.list_apps()?;
        let active_apps: Vec<&AppConfigRow> = apps.iter().filter(|a| a.active).collect();

        for app in active_apps {
            let app_public_id = AppPublicId::from_packed(app.public_id)?;
            self.register_bot(app_public_id, app.tg_bot_token.clone()).await?;
            info!(?app_public_id, "Bot restored from persisted config");
            sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }

    pub async fn get_status(&self, app_public_id: AppPublicId) -> Option<BotStatus> {
        self.bot_router.get_status(app_public_id).await
    }

    pub async fn get_all_statuses(&self) -> HashMap<AppPublicId, BotStatus> {
        self.bot_router.get_all_statuses().await
    }

    pub async fn shutdown(&self) {
        self.bot_router.shutdown().await
    }

    pub async fn send_message(
        &self,
        app_public_id: AppPublicId,
        session_id: SessionId,
        content: String,
        sender_name: String,
    ) -> eyre::Result<i64> {
        self.bot_router.send_message(app_public_id, session_id, content, sender_name).await
    }
}