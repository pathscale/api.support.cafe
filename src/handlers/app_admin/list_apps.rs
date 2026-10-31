use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{
    AppConfig, AppMemberRole, EnumErrorCode, ListAppsRequest, ListAppsResponse,
};
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

        // The bot token controls the app's Telegram bot, so only someone who may
        // edit the app gets it: a platform admin, or the app's Owner or Admin. A
        // Support member sees the app with the token blanked. The gate on this
        // endpoint (Admin or AppAdmin) is global, and AppAdmin comes from owning
        // any one app, so it cannot decide this per app.
        // TODO: Split this into separate endpoints for platform admin and regular users
        let rows: Vec<_> = if self.app_service.is_platform_admin(user_pub_id).internal()? {
            self.app_service
                .list_apps()
                .internal()?
                .into_iter()
                .map(|r| (r, true))
                .collect()
        } else {
            self.app_service
                .list_apps_for_user(user_pub_id)
                .internal()?
                .into_iter()
                .map(|(r, role)| (r, may_see_bot_token(role)))
                .collect()
        };

        tracing::debug!(
            connection_id = ctx.connection_id,
            count = rows.len(),
            "MethodListApps: listed apps successfully"
        );

        let data: Vec<AppConfig> = rows
            .into_iter()
            .map(|(r, may_edit)| AppConfig {
                app_public_id: r.public_id.unpack().expect("valid packed nanoid"),
                tg_bot_token: if may_edit {
                    r.tg_bot_token
                } else {
                    String::new()
                },
                app_name: r.app_name,
                active: r.active,
                message_persistence_enabled: r.message_persistence_enabled,
                created_at: r.created_at,
            })
            .collect();

        Ok(ListAppsResponse { data })
    }
}

/// Whether a member with this role in an app may see the app's bot token: the
/// same roles that may edit the app.
fn may_see_bot_token(role: AppMemberRole) -> bool {
    matches!(role, AppMemberRole::Owner | AppMemberRole::Admin)
}

#[cfg(test)]
mod tests {
    use super::may_see_bot_token;
    use crate::codegen::model::AppMemberRole;

    #[test]
    fn only_owners_and_admins_see_the_bot_token() {
        assert!(may_see_bot_token(AppMemberRole::Owner));
        assert!(may_see_bot_token(AppMemberRole::Admin));
        assert!(!may_see_bot_token(AppMemberRole::Support));
    }
}
