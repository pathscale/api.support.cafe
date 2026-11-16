use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use super::map_session_error;
use crate::codegen::model::{
    ChatMessage, EnumErrorCode, SubscribeEventsRequest, SubscribeEventsResponse,
};
use crate::handlers::utils::subscription_router::SubscriptionRouter;
use crate::id_types::{AppPublicId, SessionId};
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::bot::SessionKey;
use crate::service::session::ChatSessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodSubscribeEvents {
    pub event_router: Arc<SubscriptionRouter<SessionKey, ChatMessage>>,
    pub session_service: Arc<ChatSessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSubscribeEvents {
    type Request = SubscribeEventsRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
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
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Connection not authenticated")
            })?;

        let connection_app_id = self.app_connection_registry.get(connection_id).await;
        let (row, _) = self
            .session_service
            .verify_session_access_for_connection(session_id, user_pub_id, connection_app_id)
            .map_err(map_session_error)?;
        let app_public_id = AppPublicId::from_packed(row.app_public_id).internal()?;

        let key: SessionKey = (app_public_id, session_id);

        if req.unsub.unwrap_or(false) {
            self.event_router.unsubscribe(connection_id).await;
        } else {
            self.event_router.subscribe(ctx, vec![key]).await;
        }

        tracing::debug!(
            connection_id = connection_id,
            session_id = ?session_id,
            "SubscribeEvents: completed successfully"
        );

        Ok(SubscribeEventsResponse { data: vec![] })
    }
}
