use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{SetRoleRequest, SetRoleResponse};
use crate::db::schema::user::{UserWorkTable, RoleByPubIdQuery};
use crate::db::util::PackedUserPubId;

#[derive(Clone)]
pub struct MethodSetRole {
    pub user_table: Arc<UserWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetRole {
    type Request = SetRoleRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            role = ?req.role,
            "MethodSetRole: received request"
        );

        let packed_pub_id = PackedUserPubId::pack(&req.user_pub_id.into())
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        self.user_table
            .update_role_by_pub_id(RoleByPubIdQuery { role: req.role }, packed_pub_id)
            .await
            .map_err(|e| eyre::eyre!("Failed to update role: {}", e))?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            user_pub_id = %req.user_pub_id,
            "MethodSetRole: role updated successfully"
        );

        Ok(SetRoleResponse {})
    }
}