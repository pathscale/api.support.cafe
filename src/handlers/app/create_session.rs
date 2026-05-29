use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{CreateSessionRequest, CreateSessionResponse};
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::session::SessionService;

#[derive(Clone)]
pub struct MethodCreateSession {
    pub session_service: Arc<SessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateSession {
    type Request = CreateSessionRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            "CreateSession: received request"
        );

        let app_public_id = self
            .app_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated as app"))?;

        let result = self.session_service.create_session(
            req.user_pub_id.into(),
            app_public_id,
        )?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %result.session_id,
            "CreateSession: session created successfully"
        );

        Ok(CreateSessionResponse {
            session_id: result.session_id.into(),
            created_at: result.created_at,
        })
    }
}