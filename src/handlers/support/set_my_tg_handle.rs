use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{EnumErrorCode, SetMyTgHandleRequest, SetMyTgHandleResponse};
use crate::db::schema::support_info::{
    ChatIdByUserPubIdQuery, SupportInfoRow, TgHandleByUserPubIdQuery,
};
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodSetMyTgHandle {
    pub support_info_table: Arc<crate::db::schema::support_info::SupportInfoWorkTable>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetMyTgHandle {
    type Request = SetMyTgHandleRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "SetMyTgHandle: received request"
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

        if let Some(row) = self.support_info_table.select(packed_id) {
            if row.tg_handle != req.tg_handle {
                self.support_info_table
                    .update_chat_id_by_user_pub_id(
                        ChatIdByUserPubIdQuery { chat_id: None },
                        packed_id,
                    )
                    .await
                    .internal()?;
                self.support_info_table
                    .update_tg_handle_by_user_pub_id(
                        TgHandleByUserPubIdQuery {
                            tg_handle: req.tg_handle,
                        },
                        packed_id,
                    )
                    .await
                    .internal()?;
            }
        } else {
            self.support_info_table
                .insert(SupportInfoRow {
                    user_pub_id: packed_id,
                    tg_handle: req.tg_handle,
                    chat_id: None,
                })
                .internal()?;
        }

        tracing::debug!(
            connection_id = ctx.connection_id,
            "SetMyTgHandle: completed successfully"
        );

        Ok(SetMyTgHandleResponse {})
    }
}
