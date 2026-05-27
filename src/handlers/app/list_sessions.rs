use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::select::SelectQueryExecutor;

use crate::codegen::model::{ChatSession, ListSessionsRequest, ListSessionsResponse};
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::id_types::PackedNanoId;
use crate::service::app_connection_registry::AppConnectionRegistry;

pub struct MethodListSessions {
    pub chat_session_table: Arc<ChatSessionWorkTable>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListSessions {
    type Request = ListSessionsRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        _req: Self::Request,
    ) -> Response<Self::Request> {
        let app_public_id = self.app_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated as app"))?;

        let app_public_id_packed: PackedNanoId = app_public_id.pack()?;

        let rows = self.chat_session_table.select_all().execute()?;
        let data: Vec<ChatSession> = rows
            .into_iter()
            .filter(|r| r.app_public_id == app_public_id_packed)
            .map(|r| ChatSession {
                session_id: r.session_id.unpack().expect("valid packed nanoid"),
                app_public_id: r.app_public_id.unpack().expect("valid packed nanoid"),
                user_pub_id: r.user_pub_id.unpack().expect("valid packed nanoid"),
                created_at: r.created_at,
                closed_at: r.closed_at,
            })
            .collect();
        Ok(ListSessionsResponse { data })
    }
}