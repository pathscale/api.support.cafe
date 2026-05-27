use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{AddSupportUserRequest, AddSupportUserResponse};
use crate::db::schema::support_user::{SupportUserRow, SupportUserWorkTable};
use crate::id_types::PackedNanoId;

pub struct MethodAddSupportUser {
    pub support_user_table: Arc<SupportUserWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodAddSupportUser {
    type Request = AddSupportUserRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let packed_pub_id: PackedNanoId = PackedNanoId::pack(&req.app_public_id)
            .map_err(|e| eyre::eyre!("Pack error: {e}"))?;
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
