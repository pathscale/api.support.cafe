use std::sync::Arc;
use std::time::Duration;

use endpoint_libs::libs::signal::wait_for_signals;
use endpoint_libs::libs::toolbox::ArcToolbox;
use endpoint_libs::libs::toolbox::Toolbox;
use endpoint_libs::libs::ws::WebsocketServer;
use eyre::Result;
use honey_id_types::HoneyIdClient;
use honey_id_types::handlers::convenience_utils::token_management::TokenWorkTableStorage;
use honey_id_types::id_entities::UserPublicId;
use crate::service::log::LogService;
use tracing::{info, warn};

use crate::codegen::model::UserRole;
use crate::config::Config;
use crate::db::tables::Tables;
use crate::handlers;
use crate::handlers::auth_api::register_auth_api_handlers;
use crate::handlers::utils::subscription_router::SubscriptionRouter;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::app::AppService;
use crate::service::bot::{BotService, ChatMessageEvent, SessionKey};

pub struct AppCtx {
    pub config: Arc<Config>,
    pub db: Arc<Tables>,
    pub bot_service: Arc<BotService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub app_service: Arc<AppService>,
    pub event_router: Arc<SubscriptionRouter<SessionKey, ChatMessageEvent>>,
    pub toolbox: ArcToolbox,
    pub log_service: Arc<LogService>,
    pub honey_id_client: Arc<HoneyIdClient>,
    pub token_storage: Arc<TokenWorkTableStorage>,
}

pub struct App {
    ctx: Arc<AppCtx>,
}

impl App {
    pub async fn init(config: Config, log_service: Arc<LogService>) -> Result<Self> {
        #[cfg(feature = "s3-sync")]
        let db = Arc::new(Tables::new(config.database.clone(), &config.s3).await?);

        #[cfg(not(feature = "s3-sync"))]
        let db = Arc::new(Tables::new(config.database.clone()).await?);
        let toolbox = Toolbox::new();
        let app_connection_registry = Arc::new(AppConnectionRegistry::new());
        let app_service = Arc::new(AppService::new(db.app_config_table.clone()));
        let bot_service = Arc::new(BotService::new(
            db.support_user_table.clone(),
            db.support_message_table.clone(),
            app_service.clone(),
        ));

        let event_stream = bot_service.take_event_stream().await?;
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
            bot_service,
            app_connection_registry,
            app_service,
            event_router,
            toolbox,
            log_service,
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
            self.ctx.app_connection_registry.clone(),
        );
        handlers::admin::register_handlers(server, &self.ctx);
        handlers::app_admin::register_handlers(server, &self.ctx);
        handlers::app::register_handlers(server, &self.ctx);
    }

    pub async fn run(self) -> Result<()> {
        self.bootstrap_admin().await?;
        self.ctx.bot_service.bootstrap_bots().await?;

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
        self.ctx.bot_service.shutdown().await;
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

    async fn bootstrap_admin(&self) -> Result<()> {
        if let Some(admin_id) = &self.ctx.config.honey_id.admin_pub_id {
            bootstrap_admin_user(&self.ctx.db, *admin_id).await?;
        }
        Ok(())
    }
}

async fn bootstrap_admin_user(
    tables: &Tables,
    admin_pub_id: psc_nanoid::Nanoid<16, psc_nanoid::alphabet::Base62Alphabet>,
) -> Result<()> {
    let user_pub_id: UserPublicId = admin_pub_id.into();
    info!(%user_pub_id, "Admin pub ID configured, attempting to assign Admin role");

    let packed_id = user_pub_id
        .pack()
        .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

    if let Some(mut user) = tables.user_table.select_by_pub_id(packed_id) {
        user.role = UserRole::Admin;
        tables.user_table.update(user).await?;
        info!("Assigned Admin role for user {user_pub_id}");
        Ok(())
    } else {
        eyre::bail!(
            "Configured admin user does not exist in database. Sign up first, then restart the server"
        )
    }
}