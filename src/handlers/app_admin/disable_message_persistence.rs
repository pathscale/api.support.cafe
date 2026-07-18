use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{
    DisableMessagePersistenceRequest, DisableMessagePersistenceResponse, EnumErrorCode,
};
use crate::id_types::AppPublicId;
use crate::service::app::AppService;
use crate::service::message_store::MessageStore;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodDisableMessagePersistence {
    pub app_service: Arc<AppService>,
    pub message_store: Arc<MessageStore>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodDisableMessagePersistence {
    type Request = DisableMessagePersistenceRequest;
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
            .ensure_app_admin_or_owner(app_public_id, actor_pub_id)
            .internal()?;
        self.message_store
            .set_app_persistence(app_public_id, false)
            .await
            .internal()?;

        Ok(DisableMessagePersistenceResponse {})
    }
}
