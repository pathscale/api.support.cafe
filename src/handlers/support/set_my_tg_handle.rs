use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use crate::codegen::model::{SetMyTgHandleRequest, SetMyTgHandleResponse};
use crate::db::schema::support_info::SupportInfoRow;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodSetMyTgHandle {
    pub support_info_table: Arc<crate::db::schema::support_info::SupportInfoWorkTable>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetMyTgHandle {
    type Request = SetMyTgHandleRequest;

    async fn handle(
        &self,
        ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "SetMyTgHandle: received request"
        );

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        let packed_id = user_pub_id
            .pack()
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        if let Some(mut row) = self.support_info_table.select(packed_id) {
            row.tg_handle = req.tg_handle;
            self.support_info_table.update(row).await?;
        } else {
            self.support_info_table.insert(SupportInfoRow {
                user_pub_id: packed_id,
                tg_handle: req.tg_handle,
            })?;
        }

        tracing::debug!(
            connection_id = ctx.connection_id,
            "SetMyTgHandle: completed successfully"
        );

        Ok(SetMyTgHandleResponse {})
    }
}
