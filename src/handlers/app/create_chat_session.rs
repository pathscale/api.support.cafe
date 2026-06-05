use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use honey_id_types::id_entities::UserPublicId;

use crate::codegen::model::{CreateChatSessionRequest, CreateChatSessionResponse};
use crate::id_types::SessionId;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::session::ChatSessionService;

#[derive(Clone)]
pub struct MethodCreateChatSession {
    pub session_service: Arc<ChatSessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateChatSession {
    type Request = CreateChatSessionRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            "CreateChatSession: received request"
        );

        let app_public_id = self
            .app_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated as app"))?;

        let row = self
            .session_service
            .create_session(UserPublicId::from(req.user_pub_id), app_public_id)?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %SessionId::from_packed(row.session_id)?,
            "CreateChatSession: chat session created successfully"
        );

        Ok(CreateChatSessionResponse {
            session_id: row.session_id.unpack().expect("valid nanoid"),
            created_at: row.created_at,
        })
    }
}
