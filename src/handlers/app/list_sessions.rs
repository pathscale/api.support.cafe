use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{ChatSession, ListSessionsRequest, ListSessionsResponse};
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::session::SessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodListSessions {
    pub session_service: Arc<SessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListSessions {
    type Request = ListSessionsRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        _req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "ListSessions: received request"
        );

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        // Optional app filter - if connected via app, filter by that app
        let app_filter = self.app_connection_registry.get(ctx.connection_id).await;

        let sessions = self.session_service.list_sessions(user_pub_id, app_filter)?;

        let data: Vec<ChatSession> = sessions
            .into_iter()
            .map(|s| ChatSession {
                session_id: s.session_id.into(),
                app_public_id: s.app_public_id.into(),
                user_pub_id: s.user_pub_id.into(),
                created_at: s.created_at,
                closed_at: s.closed_at,
            })
            .collect();

        tracing::debug!(
            connection_id = ctx.connection_id,
            count = data.len(),
            "ListSessions: completed successfully"
        );

        Ok(ListSessionsResponse { data })
    }
}