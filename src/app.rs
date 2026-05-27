use std::sync::Arc;
use std::time::Duration;

use endpoint_libs::libs::signal::wait_for_signals;
use endpoint_libs::libs::toolbox::ArcToolbox;
use endpoint_libs::libs::toolbox::Toolbox;
use endpoint_libs::libs::ws::WebsocketServer;
use eyre::Result;
use honey_id_types::HoneyIdClient;
use honey_id_types::handlers::convenience_utils::token_management::TokenWorkTableStorage;
use tokio::sync::RwLock;
use tracing::warn;

use crate::config::Config;
use crate::db::tables::Tables;
use crate::handlers;
use crate::handlers::auth_api::register_auth_api_handlers;
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
    pub honey_id_client: Arc<HoneyIdClient>,
    pub token_storage: Arc<TokenWorkTableStorage>,
}

pub struct App {
    ctx: Arc<AppCtx>,
}

impl App {
    pub async fn init(config: Config) -> Result<Self> {
        #[cfg(feature = "s3-sync")]
        let db = Arc::new(Tables::new(config.database.clone(), &config.s3).await?);

        #[cfg(not(feature = "s3-sync"))]
        let db = Arc::new(Tables::new(config.database.clone()).await?);
        let toolbox = Toolbox::new();
        let bot_router = Arc::new(BotRouter::new(
            db.support_user_table.clone(),
            db.support_message_table.clone(),
        ));
        let app_connection_registry = Arc::new(AppConnectionRegistry::new());

        let event_stream = bot_router.take_event_stream().await?;
        let event_router = Arc::new(SubscriptionRouter::new(
            113, // SubscribeEvents method code
            event_stream,
            toolbox.clone(),
        ));

        let honey_id_client = Arc::new(HoneyIdClient::new(config.honey_id.clone()));
        let token_storage = Arc::new(TokenWorkTableStorage::default());

        let ctx = Arc::new(AppCtx {
            config: Arc::new(config),
            db,
            bot_router,
            app_connection_registry,
            event_router,
            toolbox,
            log_level: Arc::new(RwLock::new(tracing::Level::INFO)),
            honey_id_client,
            token_storage,
        });

        Ok(Self { ctx })
    }

    fn register_handlers(&self, server: &mut WebsocketServer) {
        register_auth_api_handlers(
            server,
            self.ctx.db.clone(),
            self.ctx.token_storage.clone(),
            self.ctx.honey_id_client.clone(),
        );
        handlers::platform::register_handlers(server, &self.ctx);
        handlers::app::register_handlers(server, &self.ctx);
    }

    pub async fn run(self) -> Result<()> {
        use tokio::signal::unix::{SignalKind, signal};

        let mut server = WebsocketServer::new(self.ctx.config.server.clone().into());

        self.register_handlers(&mut server);

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = server.listen() => {},
            _ = wait_for_signals(&mut sigterm, &mut sigint) => {}
        };

        // no matter if it was server issue or thread return signal, go with graceful termination procedure
        tokio::select! {
            _ = self.ctx.db.wait_for_ops() =>{
                warn!("Gracefully terminated all threads");
            },
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                std::process::exit(20);
            }
        };

        Ok(())
    }
}
