mod delete_app;
mod get_all_apps;
mod get_users;
mod set_log_level;
mod set_owner_for_ownerless_app;
mod set_role;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::admin::delete_app::MethodDeleteApp;
use crate::handlers::admin::get_all_apps::MethodGetAllApps;
use crate::handlers::admin::get_users::MethodGetUsers;
use crate::handlers::admin::set_log_level::MethodSetLogLevel;
use crate::handlers::admin::set_owner_for_ownerless_app::MethodSetOwnerForOwnerlessApp;
use crate::handlers::admin::set_role::MethodSetRole;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodDeleteApp {
        app_service: ctx.app_service.clone(),
        bot_service: ctx.bot_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
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
        app_service: ctx.app_service.clone(),
    });
    server.add_handler(MethodSetOwnerForOwnerlessApp {
        app_service: ctx.app_service.clone(),
    });
}
