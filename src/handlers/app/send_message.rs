use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{SendMessageRequest, SendMessageResponse};
use crate::id_types::SessionId;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::bot::BotService;

pub struct MethodSendMessage {
    pub bot_service: Arc<BotService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSendMessage {
    type Request = SendMessageRequest;

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

        let sent_at = self.bot_service
            .send_message(
                app_public_id,
                session_id,
                req.content,
                "User".to_string(),
            )
            .await
            .map_err(|e| eyre::eyre!("Failed to send message: {e}"))?;

        Ok(SendMessageResponse { sent_at })
    }
}