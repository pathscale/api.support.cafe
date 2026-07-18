use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{DeleteAppRequest, DeleteAppResponse, EnumErrorCode};
use crate::id_types::AppPublicId;
use crate::service::app::AppService;
use crate::service::bot::BotService;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodDeleteApp {
    pub app_service: Arc<AppService>,
    pub bot_service: Arc<BotService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodDeleteApp {
    type Request = DeleteAppRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let app_public_id: AppPublicId = req.app_public_id.into();
        let actor_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Connection not authenticated")
            })?;

        self.app_service
            .ensure_app_owner(app_public_id, actor_pub_id)
            .internal()?;
        self.bot_service.unregister_bot(app_public_id).await;
        self.app_service.delete_app(app_public_id).await.internal()?;
        Ok(DeleteAppResponse {})
    }
}
