use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{AppMember, ListAppMembersRequest, ListAppMembersResponse};
use crate::id_types::AppPublicId;
use crate::service::app::AppService;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodListAppMembers {
    pub app_service: Arc<AppService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListAppMembers {
    type Request = ListAppMembersRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        let app_public_id: AppPublicId = req.app_public_id.into();
        let actor_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        self.app_service
            .ensure_app_member(app_public_id, actor_pub_id)?;

        let data: Vec<AppMember> = self
            .app_service
            .list_members(app_public_id)?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(ListAppMembersResponse { data })
    }
}
