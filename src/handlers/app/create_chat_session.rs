use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};
use honey_id_types::id_entities::UserPublicId;

use crate::codegen::model::{CreateChatSessionRequest, CreateChatSessionResponse, EnumErrorCode};
use crate::id_types::{AppPublicId, SessionId};
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::session::ChatSessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodCreateChatSession {
    pub session_service: Arc<ChatSessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateChatSession {
    type Request = CreateChatSessionRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            "CreateChatSession: received request"
        );

        // A session is opened by the end user it is for: a widget's App
        // connection for the visitor it connected as, or a signed-in user for
        // themselves. An app connection used to open sessions for any user id.
        let user_pub_id = UserPublicId::from(req.user_pub_id);
        if self.user_connection_registry.get(ctx.connection_id).await != Some(user_pub_id) {
            return Err(CustomError::new(EnumErrorCode::Forbidden)
                .with_message("A session can only be opened for yourself")
                .into());
        }

        let app_public_id = match self.app_connection_registry.get(ctx.connection_id).await {
            Some(app) => app,
            None => {
                let app: AppPublicId = req
                    .app_public_id
                    .ok_or_else(|| {
                        CustomError::new(EnumErrorCode::BadRequest)
                            .with_message("Name the desk in app_public_id")
                    })?
                    .into();
                if !self.session_service.app_accepts_chats(app).internal()? {
                    return Err(CustomError::new(EnumErrorCode::NotFound)
                        .with_message("No such app")
                        .into());
                }
                app
            }
        };

        let row = self
            .session_service
            .create_session(UserPublicId::from(req.user_pub_id), app_public_id)
            .await
            .internal()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %SessionId::from_packed(row.session_id).internal()?,
            "CreateChatSession: chat session created successfully"
        );

        Ok(CreateChatSessionResponse {
            session_id: row.session_id.unpack().expect("valid nanoid"),
            created_at: row.created_at,
        })
    }
}
