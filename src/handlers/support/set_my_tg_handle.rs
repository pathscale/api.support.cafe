use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{EnumErrorCode, SetMyTgHandleRequest, SetMyTgHandleResponse};
use crate::service::app::{AppService, normalize_telegram_handle};
use crate::service::user_connection_registry::UserConnectionRegistry;

use super::signed_in_staff;

#[derive(Clone)]
pub struct MethodSetMyTgHandle {
    pub app_service: Arc<AppService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetMyTgHandle {
    type Request = SetMyTgHandleRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let user = signed_in_staff(&ctx, &self.user_connection_registry).await?;
        let member = self
            .app_service
            .telegram_member(req.app_public_id.into(), user)
            .internal()?
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Forbidden).with_message("Not a member of this app")
            })?;
        let handle = normalize_telegram_handle(&req.tg_handle).ok_or_else(|| {
            CustomError::new(EnumErrorCode::BadRequest).with_message("Invalid Telegram username")
        })?;
        let challenge = self
            .app_service
            .issue_telegram_link(&member, &handle, Instant::now())
            .await
            .internal()?
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Conflict)
                    .with_message("Telegram bot unavailable for this app")
            })?;
        Ok(SetMyTgHandleResponse { challenge })
    }
}
