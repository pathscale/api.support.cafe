use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{AppConfig, ListAppsRequest, ListAppsResponse};
use crate::service::app::AppService;

#[derive(Clone)]
pub struct MethodListApps {
    pub app_service: Arc<AppService>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListApps {
    type Request = ListAppsRequest;

    async fn handle(&self, ctx: RequestContext, _req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodListApps: received request"
        );

        let rows = self.app_service.list_apps()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            count = rows.len(),
            "MethodListApps: listed apps successfully"
        );

        let data: Vec<AppConfig> = rows
            .into_iter()
            .map(|r| AppConfig {
                app_public_id: r.public_id.unpack().expect("valid packed nanoid"),
                tg_bot_token: r.tg_bot_token,
                app_name: r.app_name,
                active: r.active,
                created_at: r.created_at,
            })
            .collect();

        Ok(ListAppsResponse { data })
    }
}