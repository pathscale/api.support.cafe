use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{EditAppRequest, EditAppResponse, EnumErrorCode};
use crate::id_types::AppPublicId;
use crate::service::app::{AppService, AppUpdate};
use crate::service::bot::BotService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodEditApp {
    pub app_service: Arc<AppService>,
    pub bot_service: Arc<BotService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodEditApp {
    type Request = EditAppRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            app_public_id = %req.app_public_id,
            "MethodEditApp: received request"
        );

        let app_public_id: AppPublicId = req.app_public_id.into();
        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Connection not authenticated")
            })?;

        self.app_service
            .ensure_app_admin_or_owner(app_public_id, user_pub_id)
            .internal()?;

        let update = AppUpdate {
            tg_bot_token: req.tg_bot_token.clone(),
            app_name: req.app_name.clone(),
            active: req.active,
            message_persistence_enabled: req.message_persistence_enabled,
        };

        self.app_service
            .edit_app(app_public_id, update)
            .await
            .internal()?;

        if let Some(token) = &req.tg_bot_token {
            self.bot_service.unregister_bot(app_public_id).await;
            self.bot_service
                .register_bot(app_public_id, token.clone())
                .await
                .internal()?;
        }

        if let Some(active) = req.active {
            if active {
                let app = self.app_service.get_app(app_public_id).internal()?;
                if let Some(app) = app {
                    self.bot_service
                        .register_bot(app_public_id, app.tg_bot_token)
                        .await
                        .internal()?;
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
