use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{SubscribeEventsRequest, SubscribeEventsResponse};
use crate::handlers::utils::subscription_router::SubscriptionRouter;
use crate::id_types::SessionId;
use crate::service::bot::{ChatMessageEvent, SessionKey};
use crate::service::session::SessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodSubscribeEvents {
    pub event_router: Arc<SubscriptionRouter<SessionKey, ChatMessageEvent>>,
    pub session_service: Arc<SessionService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSubscribeEvents {
    type Request = SubscribeEventsRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %req.session_id,
            unsub = ?req.unsub,
            "SubscribeEvents: received request"
        );

        let session_id: SessionId = req.session_id.into();
        let connection_id = ctx.connection_id;

        let user_pub_id = self
            .user_connection_registry
            .get(connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        let verified = self.session_service.verify_session_access(session_id, user_pub_id)?;

        let key: SessionKey = (verified.app_public_id, verified.session_id);

        if req.unsub.unwrap_or(false) {
            self.event_router.unsubscribe(connection_id).await;
        } else {
            self.event_router.subscribe(ctx, vec![key]).await;
        }

        tracing::debug!(
            connection_id = connection_id,
            session_id = %req.session_id,
            "SubscribeEvents: completed successfully"
        );

        Ok(SubscribeEventsResponse { data: vec![] })
    }
}