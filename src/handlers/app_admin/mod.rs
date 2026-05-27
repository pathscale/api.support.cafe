mod add_support_user;
mod create_app;
mod edit_app;
mod list_apps;
mod list_support_users;
mod remove_support_user;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::app_admin::add_support_user::MethodAddSupportUser;
use crate::handlers::app_admin::create_app::MethodCreateApp;
use crate::handlers::app_admin::edit_app::MethodEditApp;
use crate::handlers::app_admin::list_apps::MethodListApps;
use crate::handlers::app_admin::list_support_users::MethodListSupportUsers;
use crate::handlers::app_admin::remove_support_user::MethodRemoveSupportUser;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodCreateApp {
        app_config_table: ctx.db.app_config_table.clone(),
        bot_router: ctx.bot_router.clone(),
    });
    server.add_handler(MethodEditApp {
        app_config_table: ctx.db.app_config_table.clone(),
        bot_router: ctx.bot_router.clone(),
    });
    server.add_handler(MethodListApps {
        app_config_table: ctx.db.app_config_table.clone(),
    });
    server.add_handler(MethodAddSupportUser {
        support_user_table: ctx.db.support_user_table.clone(),
    });
    server.add_handler(MethodListSupportUsers {
        support_user_table: ctx.db.support_user_table.clone(),
    });
    server.add_handler(MethodRemoveSupportUser {
        support_user_table: ctx.db.support_user_table.clone(),
    });
}