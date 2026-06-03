use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{ListSupportUsersRequest, ListSupportUsersResponse, SupportUser};
use crate::db::schema::support_user::SupportUserWorkTable;
use crate::id_types::{AppPublicId, PackedNanoId};
use crate::service::app::AppService;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodListSupportUsers {
    pub app_service: Arc<AppService>,
    pub support_user_table: Arc<SupportUserWorkTable>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListSupportUsers {
    type Request = ListSupportUsersRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let app_public_id: AppPublicId = req.app_public_id.into();
        let actor_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        self.app_service
            .ensure_app_member(app_public_id, actor_pub_id)?;

        let packed_pub_id: PackedNanoId = app_public_id.pack()?;
        let rows = self
            .support_user_table
            .select_by_app_public_id(packed_pub_id)
            .execute()?;
        let data: Vec<SupportUser> = rows
            .into_iter()
            .map(|r| SupportUser {
                id: r.id as i64,
                app_public_id: r.app_public_id.unpack().expect("valid packed nanoid"),
                tg_handle: r.tg_handle,
                chat_id: r.chat_id,
                is_active: r.is_active,
            })
            .collect();
        Ok(ListSupportUsersResponse { data })
    }
}
