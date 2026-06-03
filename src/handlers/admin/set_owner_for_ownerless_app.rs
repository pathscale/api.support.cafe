use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{SetOwnerForOwnerlessAppRequest, SetOwnerForOwnerlessAppResponse};
use crate::id_types::AppPublicId;
use crate::service::app::AppService;

#[derive(Clone)]
pub struct MethodSetOwnerForOwnerlessApp {
    pub app_service: Arc<AppService>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetOwnerForOwnerlessApp {
    type Request = SetOwnerForOwnerlessAppRequest;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            app_public_id = %req.app_public_id,
            user_pub_id = %req.user_pub_id,
            "MethodSetOwnerForOwnerlessApp: received request"
        );

        let app_public_id: AppPublicId = req.app_public_id.into();
        self.app_service
            .set_owner_for_ownerless_app(app_public_id, req.user_pub_id.into())
            .await?;

        Ok(SetOwnerForOwnerlessAppResponse {})
    }
}
