use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::GetMyInfoRequest;
use crate::service::user::UserService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodGetMyInfo {
    pub user_service: Arc<UserService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodGetMyInfo {
    type Request = GetMyInfoRequest;

    async fn handle(&self, ctx: RequestContext, _req: Self::Request) -> Response<Self::Request> {
        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| eyre::eyre!("Connection not authenticated"))?;

        self.user_service.get_my_info(user_pub_id)
    }
}
