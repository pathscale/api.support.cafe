use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{EditAppRequest, EditAppResponse};
use crate::id_types::AppPublicId;
use crate::service::app::{AppService, AppUpdate};
use crate::service::bot::BotService;

#[derive(Clone)]
pub struct MethodEditApp {
    pub app_service: Arc<AppService>,
    pub bot_service: Arc<BotService>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodEditApp {
    type Request = EditAppRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            app_public_id = %req.app_public_id,
            "MethodEditApp: received request"
        );

        let app_public_id: AppPublicId = req.app_public_id.into();

        let update = AppUpdate {
            tg_bot_token: req.tg_bot_token.clone(),
            app_name: req.app_name.clone(),
            active: req.active,
        };

        self.app_service.edit_app(app_public_id, update).await?;

        if let Some(token) = &req.tg_bot_token {
            self.bot_service.unregister_bot(app_public_id).await;
            self.bot_service.register_bot(app_public_id, token.clone()).await?;
        }

        if let Some(active) = req.active {
            if active {
                let app = self.app_service.get_app(app_public_id)?;
                if let Some(app) = app {
                    self.bot_service.register_bot(app_public_id, app.tg_bot_token).await?;
                }
            } else {
                self.bot_service.unregister_bot(app_public_id).await;
            }
        }

        tracing::debug!(
            connection_id = ctx.connection_id,
            app_public_id = %app_public_id,
            "MethodEditApp: app updated successfully"
        );

        Ok(EditAppResponse {})
    }
}