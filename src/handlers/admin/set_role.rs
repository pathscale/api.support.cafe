use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{SetRoleRequest, SetRoleResponse};
use crate::db::schema::user::{UserColumns, UserWorkTable};
use crate::db::util::PackedUserPubId;

#[derive(Clone)]
pub struct MethodSetRole {
    pub user_table: Arc<UserWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetRole {
    type Request = SetRoleRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            role = ?req.role,
            "MethodSetRole: received request"
        );

        let packed_pub_id = PackedUserPubId::pack(&req.user_pub_id)
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))
            .internal()?;

        self.user_table
            .update_by_pub_id(packed_pub_id, UserColumns::ROLE, req.role)
            .await
            .map_err(|e| eyre::eyre!("Failed to update role: {}", e))
            .internal()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            "MethodSetRole: role updated successfully"
        );

        Ok(SetRoleResponse {})
    }
}
