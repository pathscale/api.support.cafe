mod delete_app;
mod get_all_apps;
mod get_users;
mod set_log_level;
mod set_role;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::admin::delete_app::MethodDeleteApp;
use crate::handlers::admin::get_all_apps::MethodGetAllApps;
use crate::handlers::admin::get_users::MethodGetUsers;
use crate::handlers::admin::set_log_level::MethodSetLogLevel;
use crate::handlers::admin::set_role::MethodSetRole;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodDeleteApp {
        app_config_table: ctx.db.app_config_table.clone(),
        bot_service: ctx.bot_service.clone(),
    });
    server.add_handler(MethodSetLogLevel {
        log_service: ctx.log_service.clone(),
    });
    server.add_handler(MethodGetUsers {
        user_table: ctx.db.user_table.clone(),
    });
    server.add_handler(MethodSetRole {
        user_table: ctx.db.user_table.clone(),
    });
    server.add_handler(MethodGetAllApps {
        app_config_table: ctx.db.app_config_table.clone(),
    });
}