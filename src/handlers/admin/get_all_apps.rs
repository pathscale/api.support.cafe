use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{AppInfo, GetAllAppsRequest, GetAllAppsResponse};
use crate::service::app::AppService;

#[derive(Clone)]
pub struct MethodGetAllApps {
    pub app_service: Arc<AppService>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodGetAllApps {
    type Request = GetAllAppsRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, _req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodGetAllApps: received request"
        );

        let rows = self.app_service.list_apps().internal()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            count = rows.len(),
            "MethodGetAllApps: listed apps successfully"
        );

        let data: Vec<AppInfo> = rows.into_iter().map(Into::into).collect();

        Ok(GetAllAppsResponse { data })
    }
}
