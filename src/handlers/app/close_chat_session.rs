use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{CloseChatSessionRequest, CloseChatSessionResponse, EnumErrorCode};
use crate::id_types::SessionId;
use crate::service::session::ChatSessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodCloseChatSession {
    pub session_service: Arc<ChatSessionService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCloseChatSession {
    type Request = CloseChatSessionRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            "CloseChatSession: received request"
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

        self.session_service
            .close_session(session_id, user_pub_id)
            .await
            .internal()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            "CloseChatSession: completed successfully"
        );

        Ok(CloseChatSessionResponse {})
    }
}
