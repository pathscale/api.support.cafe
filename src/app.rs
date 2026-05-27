use std::sync::Arc;

use endpoint_libs::libs::toolbox::ArcToolbox;
use endpoint_libs::libs::toolbox::Toolbox;
use endpoint_libs::libs::ws::WebsocketServer;
use eyre::Result;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::db::tables::Tables;
use crate::handlers;
use crate::handlers::auth::register_auth_handlers;
use crate::handlers::utils::subscription_router::SubscriptionRouter;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::bot_router::{BotRouter, ChatMessageEvent, SessionKey};

pub struct AppCtx {
    pub config: Arc<Config>,
    pub db: Arc<Tables>,
    pub bot_router: Arc<BotRouter>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub event_router: Arc<SubscriptionRouter<SessionKey, ChatMessageEvent>>,
    pub toolbox: ArcToolbox,
    pub log_level: Arc<RwLock<tracing::Level>>,
}

pub struct App {
    ctx: Arc<AppCtx>,
}

impl App {
    pub async fn init(config: Config) -> Result<Self> {
        let db = Arc::new(Tables::new(config.database.clone()).await?);
        let toolbox = Toolbox::new();
        let bot_router = Arc::new(BotRouter::new(
            db.support_user_table.clone(),
            db.support_message_table.clone(),
        ));
        let app_connection_registry = Arc::new(AppConnectionRegistry::new());

        let event_stream = bot_router.take_event_stream().await?;
        let event_router = Arc::new(SubscriptionRouter::new(
            113,  // SubscribeEvents method code
            event_stream,
            toolbox.clone(),
        ));

        let ctx = Arc::new(AppCtx {
            config: Arc::new(config),
            db,
            bot_router,
            app_connection_registry,
            event_router,
            toolbox,
            log_level: Arc::new(RwLock::new(tracing::Level::INFO)),
        });

        Ok(Self { ctx })
    }

    fn register_handlers(&self, server: &mut WebsocketServer) {
        register_auth_handlers(server, &self.ctx);
        handlers::platform::register_handlers(server, &self.ctx);
        handlers::app::register_handlers(server, &self.ctx);
    }

    pub async fn run(self) -> Result<()> {
        let mut server = WebsocketServer::new(self.ctx.config.server.clone().into());

        self.register_handlers(&mut server);

        server.listen().await?;

        self.ctx.db.wait_for_ops().await;

        Ok(())
    }
}