use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{ListMessagesRequest, ListMessagesResponse};
use crate::id_types::SessionId;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::session::SessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodListMessages {
    pub session_service: Arc<SessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListMessages {
    type Request = ListMessagesRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            "ListMessages: received request"
        );

        let session_id: SessionId = req.session_id.into();

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        let _verified = self.session_service.verify_session_access(session_id, user_pub_id)?;

        // If connected via app, verify session belongs to that app
        if let Some(app_public_id) = self.app_connection_registry.get(ctx.connection_id).await {
            if !self.session_service.is_for_app(session_id, app_public_id)? {
                eyre::bail!("Session does not belong to this app");
            }
        }

        let messages = self.session_service.list_messages(session_id)?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            count = messages.len(),
            "ListMessages: completed successfully"
        );

        Ok(ListMessagesResponse { data: messages })
    }
}