use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{AddSupportUserRequest, AddSupportUserResponse};
use crate::db::schema::support_user::{SupportUserRow, SupportUserWorkTable};
use crate::id_types::{AppPublicId, PackedNanoId};
use crate::service::app::AppService;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodAddSupportUser {
    pub app_service: Arc<AppService>,
    pub support_user_table: Arc<SupportUserWorkTable>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodAddSupportUser {
    type Request = AddSupportUserRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let app_public_id: AppPublicId = req.app_public_id.into();
        let actor_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        self.app_service
            .ensure_app_admin_or_owner(app_public_id, actor_pub_id)?;

        let packed_pub_id: PackedNanoId = app_public_id.pack()?;
        let row = SupportUserRow {
            id: self.support_user_table.get_next_pk().into(),
            app_public_id: packed_pub_id,
            tg_handle: req.tg_handle,
            chat_id: None,
            is_active: true,
        };
        self.support_user_table.insert(row)?;
        Ok(AddSupportUserResponse {})
    }
}
