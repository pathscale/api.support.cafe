use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{EnumErrorCode, GetMyTgHandleRequest, GetMyTgHandleResponse};
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodGetMyTgHandle {
    pub support_info_table: Arc<crate::db::schema::support_info::SupportInfoWorkTable>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodGetMyTgHandle {
    type Request = GetMyTgHandleRequest;
    type Error = CustomError;

    async fn handle(
        &self,
        ctx: RequestContext,
        _req: Self::Request,
    ) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "GetMyTgHandle: received request"
        );

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Connection not authenticated")
            })?;

        let packed_id = user_pub_id
            .pack()
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))
            .internal()?;

        let tg_handle = self
            .support_info_table
            .select(packed_id)
            .map(|row| row.tg_handle);

        tracing::debug!(
            connection_id = ctx.connection_id,
            "GetMyTgHandle: completed successfully"
        );

        Ok(GetMyTgHandleResponse { tg_handle })
    }
}
