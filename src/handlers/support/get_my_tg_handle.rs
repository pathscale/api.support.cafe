use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{EnumErrorCode, GetMyTgHandleRequest, GetMyTgHandleResponse};
use crate::service::app::AppService;
use crate::service::user_connection_registry::UserConnectionRegistry;

use super::signed_in_staff;

#[derive(Clone)]
pub struct MethodGetMyTgHandle {
    pub app_service: Arc<AppService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodGetMyTgHandle {
    type Request = GetMyTgHandleRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let user = signed_in_staff(&ctx, &self.user_connection_registry).await?;
        let app = req.app_public_id.into();
        let member = self
            .app_service
            .telegram_member(app, user)
            .internal()?
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Forbidden).with_message("Not a member of this app")
            })?;
        let status = self
            .app_service
            .telegram_link_status(app, &member, Instant::now())
            .await;
        Ok(GetMyTgHandleResponse {
            tg_handle: status.handle,
            link_state: status.state.to_owned(),
        })
    }
}
