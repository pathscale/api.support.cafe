use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{EnumErrorCode, SendMessageRequest, SendMessageResponse};
use crate::id_types::SessionId;
use crate::service::session::ChatSessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodSendMessage {
    pub session_service: Arc<ChatSessionService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSendMessage {
    type Request = SendMessageRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            content_len = req.content.len(),
            "SendMessage: received request"
        );

        let session_id: SessionId = req.session_id.into();

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Connection not authenticated")
            })?;

        let sent_at = self
            .session_service
            .send_message(session_id, req.content, user_pub_id)
            .await
            .internal()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            sent_at,
            "SendMessage: completed successfully"
        );

        Ok(SendMessageResponse { sent_at })
    }
}
