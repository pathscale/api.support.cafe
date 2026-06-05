use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{CreateAppRequest, CreateAppResponse};
use crate::service::app::AppService;
use crate::service::bot::BotService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodCreateApp {
    pub app_service: Arc<AppService>,
    pub bot_service: Arc<BotService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateApp {
    type Request = CreateAppRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodCreateApp: received request"
        );

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        let result = self
            .app_service
            .create_app(
                req.tg_bot_token.clone(),
                req.app_name.clone(),
                req.message_persistence_enabled.unwrap_or(false),
                user_pub_id,
            )
            .await?;

        self.bot_service
            .register_bot(result.app_public_id, req.tg_bot_token)
            .await?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            app_public_id = %result.app_public_id,
            "MethodCreateApp: app created successfully"
        );

        Ok(CreateAppResponse {
            app_public_id: result.app_public_id.into(),
            created_at: result.created_at,
        })
    }
}
