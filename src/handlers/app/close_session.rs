use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{CloseSessionRequest, CloseSessionResponse};
use crate::id_types::SessionId;
use crate::service::session::SessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodCloseSession {
    pub session_service: Arc<SessionService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCloseSession {
    type Request = CloseSessionRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            "CloseSession: received request"
        );

        let session_id: SessionId = req.session_id.into();

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        self.session_service.close_session(session_id, user_pub_id).await?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            "CloseSession: completed successfully"
        );

        Ok(CloseSessionResponse {})
    }
}