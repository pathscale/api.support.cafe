use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::select::SelectQueryExecutor;

use crate::codegen::model::{RemoveSupportUserRequest, RemoveSupportUserResponse};
use crate::db::schema::support_user::{SupportUserPrimaryKey, SupportUserWorkTable};
use crate::id_types::PackedNanoId;

pub struct MethodRemoveSupportUser {
    pub support_user_table: Arc<SupportUserWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodRemoveSupportUser {
    type Request = RemoveSupportUserRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let packed_pub_id: PackedNanoId = PackedNanoId::pack(&req.app_public_id)
            .map_err(|e| eyre::eyre!("Pack error: {e}"))?;
        let all_users = self.support_user_table.select_all().execute()?;
        let target = all_users.into_iter().find(|r| r.app_public_id == packed_pub_id && r.tg_handle == req.tg_handle)
            .ok_or_else(|| eyre::eyre!("Support user not found"))?;
        self.support_user_table
            .delete(SupportUserPrimaryKey::from(target.id))
            .await?;
        Ok(RemoveSupportUserResponse {})
    }
}
