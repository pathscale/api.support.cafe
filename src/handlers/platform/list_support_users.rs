use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{ListSupportUsersRequest, ListSupportUsersResponse, SupportUser};
use crate::db::schema::support_user::SupportUserWorkTable;
use crate::id_types::PackedNanoId;

pub struct MethodListSupportUsers {
    pub support_user_table: Arc<SupportUserWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListSupportUsers {
    type Request = ListSupportUsersRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let packed_pub_id: PackedNanoId = PackedNanoId::pack(&req.app_public_id)
            .map_err(|e| eyre::eyre!("Pack error: {e}"))?;
        let rows = self.support_user_table.select_by_app_public_id(packed_pub_id).execute()?;
        let data: Vec<SupportUser> = rows.into_iter().map(|r| SupportUser {
            id: r.id,
            app_public_id: r.app_public_id.unpack().expect("valid packed nanoid"),
            tg_handle: r.tg_handle,
            chat_id: r.chat_id,
            is_active: r.is_active,
        }).collect();
        Ok(ListSupportUsersResponse { data })
    }
}
