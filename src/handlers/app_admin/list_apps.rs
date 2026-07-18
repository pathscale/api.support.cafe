use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{AppConfig, EnumErrorCode, ListAppsRequest, ListAppsResponse};
use crate::service::app::AppService;
use crate::service::user_connection_registry::UserConnectionRegistry;

#[derive(Clone)]
pub struct MethodListApps {
    pub app_service: Arc<AppService>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodListApps {
    type Request = ListAppsRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, _req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodListApps: received request"
        );

        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Connection not authenticated")
            })?;

        // TODO: Split this into separate endpoints for platform admin and regular users
        let rows = if self.app_service.is_platform_admin(user_pub_id).internal()? {
            self.app_service.list_apps().internal()?
        } else {
            self.app_service.list_apps_for_user(user_pub_id).internal()?
        };

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
                message_persistence_enabled: r.message_persistence_enabled,
                created_at: r.created_at,
            })
            .collect();

        Ok(ListAppsResponse { data })
    }
}
