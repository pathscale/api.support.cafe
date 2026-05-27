use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{SubscribeEventsRequest, SubscribeEventsResponse};
use crate::id_types::SessionId;
use crate::handlers::utils::subscription_router::SubscriptionRouter;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::bot_router::{ChatMessageEvent, SessionKey};

pub struct MethodSubscribeEvents {
    pub event_router: Arc<SubscriptionRouter<SessionKey, ChatMessageEvent>>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSubscribeEvents {
    type Request = SubscribeEventsRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let app_public_id = self.app_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated as app"))?;

        let session_id: SessionId = req.session_id.into();
        let key: SessionKey = (app_public_id, session_id);

        if req.unsub.unwrap_or(false) {
            self.event_router.unsubscribe(ctx.connection_id).await;
        } else {
            self.event_router.subscribe(ctx, vec![key]).await;
        }

        Ok(SubscribeEventsResponse { data: vec![] })
    }
}