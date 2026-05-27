use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{AppConfig, ListAppsRequest, ListAppsResponse};
use crate::db::schema::app_config::AppConfigWorkTable;

pub struct MethodListApps {
    pub app_config_table: Arc<AppConfigWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListApps {
    type Request = ListAppsRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        _req: Self::Request,
    ) -> Response<Self::Request> {
        let rows = self.app_config_table.select_all().execute()?;
        let data: Vec<AppConfig> = rows
            .into_iter()
            .map(|r| AppConfig {
                app_public_id: r.public_id.unpack().expect("valid packed nanoid"),
                tg_bot_token: r.tg_bot_token,
                api_key: r.api_key,
                app_name: r.app_name,
                active: r.active,
                created_at: r.created_at,
            })
            .collect();
        Ok(ListAppsResponse { data })
    }
}