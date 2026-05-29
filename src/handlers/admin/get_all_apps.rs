use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{AppInfo, GetAllAppsRequest, GetAllAppsResponse};
use crate::db::schema::app_config::AppConfigWorkTable;

#[derive(Clone)]
pub struct MethodGetAllApps {
    pub app_config_table: Arc<AppConfigWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodGetAllApps {
    type Request = GetAllAppsRequest;

    async fn handle(&self, ctx: RequestContext, _req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodGetAllApps: received request"
        );

        let rows = self.app_config_table.select_all().execute()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            count = rows.len(),
            "MethodGetAllApps: listed apps successfully"
        );

        let data: Vec<AppInfo> = rows.into_iter().map(Into::into).collect();

        Ok(GetAllAppsResponse { data })
    }
}