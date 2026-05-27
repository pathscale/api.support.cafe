use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use psc_nanoid::{alphabet::Base62Alphabet, Nanoid};

use crate::codegen::model::{CreateSessionRequest, CreateSessionResponse};
use crate::db::schema::chat_session::{ChatSessionRow, ChatSessionWorkTable};
use crate::id_types::{AppPublicId, SessionId, UserPubId};
use crate::service::app_connection_registry::AppConnectionRegistry;

pub struct MethodCreateSession {
    pub chat_session_table: Arc<ChatSessionWorkTable>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateSession {
    type Request = CreateSessionRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        _req: Self::Request,
    ) -> Response<Self::Request> {
        let app_public_id: AppPublicId = self.app_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated as app"))?;

        let session_id_nanoid = Nanoid::<16, Base62Alphabet>::new();
        let session_id: SessionId = session_id_nanoid.into();

        let user_pub_id_nanoid = Nanoid::<16, Base62Alphabet>::new();
        let user_pub_id: UserPubId = user_pub_id_nanoid.into();

        let created_at = Utc::now().timestamp_millis();

        let row = ChatSessionRow {
            id: self.chat_session_table.get_next_pk().into(),
            session_id: session_id.pack()?,
            app_public_id: app_public_id.pack()?,
            user_pub_id: user_pub_id.pack()?,
            created_at,
            closed_at: None,
        };
        self.chat_session_table.insert(row)?;

        Ok(CreateSessionResponse {
            session_id: session_id_nanoid,
            created_at,
        })
    }
}