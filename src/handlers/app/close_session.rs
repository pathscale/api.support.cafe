use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::select::SelectQueryExecutor;

use crate::codegen::model::{CloseSessionRequest, CloseSessionResponse};
use crate::db::schema::chat_session::{ChatSessionWorkTable, ClosedAtByIdQuery};
use crate::id_types::{PackedNanoId, SessionId};

pub struct MethodCloseSession {
    pub chat_session_table: Arc<ChatSessionWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCloseSession {
    type Request = CloseSessionRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let session_id: SessionId = req.session_id.into();
        let packed_session_id: PackedNanoId = session_id.pack()?;
        let all_sessions = self.chat_session_table.select_all().execute()?;
        let session = all_sessions
            .iter()
            .find(|s| s.session_id == packed_session_id)
            .ok_or_else(|| eyre::eyre!("Session not found"))?;
        let closed_at = Utc::now().timestamp_millis();
        self.chat_session_table
            .update_closed_at_by_id(ClosedAtByIdQuery { closed_at: Some(closed_at) }, session.id)
            .await?;
        Ok(CloseSessionResponse {})
    }
}