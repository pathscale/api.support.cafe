use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{ChatMessage, ListMessagesRequest, ListMessagesResponse};
use crate::db::schema::support_message::SupportMessageWorkTable;
use crate::id_types::{PackedNanoId, SessionId};

pub struct MethodListMessages {
    pub support_message_table: Arc<SupportMessageWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListMessages {
    type Request = ListMessagesRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let session_id: SessionId = req.session_id.into();
        let packed_session_id: PackedNanoId = session_id.pack()?;
        let all_msgs = self.support_message_table.select_all().execute()?;
        let data: Vec<ChatMessage> = all_msgs
            .into_iter()
            .filter(|r| r.session_id == packed_session_id)
            .map(|r| ChatMessage {
                incoming: r.incoming,
                sent_at: r.sent_at,
                content: r.content,
            })
            .collect();
        Ok(ListMessagesResponse { data })
    }
}