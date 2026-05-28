use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{CreateAppRequest, CreateAppResponse};
use crate::service::app::AppService;
use crate::service::bot::BotService;

#[derive(Clone)]
pub struct MethodCreateApp {
    pub app_service: Arc<AppService>,
    pub bot_service: Arc<BotService>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateApp {
    type Request = CreateAppRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodCreateApp: received request"
        );

        let result = self
            .app_service
            .create_app(req.tg_bot_token.clone(), req.app_name.clone())?;

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