use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{AddAppMemberRequest, AddAppMemberResponse, AppMemberRole};
use crate::id_types::AppPublicId;
use crate::service::app::AppService;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodAddAppMember {
    pub app_service: Arc<AppService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodAddAppMember {
    type Request = AddAppMemberRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let app_public_id: AppPublicId = req.app_public_id.into();
        let actor_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        self.app_service
            .ensure_app_admin_or_owner(app_public_id, actor_pub_id)?;
        self.app_service
            .add_member(
                app_public_id,
                req.user_pub_id.into(),
                AppMemberRole::Support,
            )
            .await?;

        Ok(AddAppMemberResponse {})
    }
}
