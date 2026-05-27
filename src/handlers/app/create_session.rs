use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use psc_nanoid::{alphabet::Base62Alphabet, Nanoid};

use crate::codegen::model::{CreateSessionRequest, CreateSessionResponse};
use crate::db::schema::chat_session::{ChatSessionRow, ChatSessionWorkTable};
use crate::id_types::{AppPublicId, SessionId, UserPubId, PackedNanoId};
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
        let session_id: SessionId = PackedNanoId::pack(&session_id_nanoid)
            .map_err(|e| eyre::eyre!("Failed to pack session_id: {e}"))?
            .into();

        let user_pub_id_nanoid = Nanoid::<16, Base62Alphabet>::new();
        let user_pub_id: UserPubId = PackedNanoId::pack(&user_pub_id_nanoid)
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {e}"))?
            .into();

        let created_at = Utc::now().timestamp_millis();

        let row = ChatSessionRow {
            id: self.chat_session_table.get_next_pk().into(),
            session_id: session_id.into(),
            app_public_id: app_public_id.into(),
            user_pub_id: user_pub_id.into(),
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