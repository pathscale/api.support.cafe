mod delete_app;
mod set_log_level;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::admin::delete_app::MethodDeleteApp;
use crate::handlers::admin::set_log_level::MethodSetLogLevel;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodDeleteApp {
        app_config_table: ctx.db.app_config_table.clone(),
        bot_service: ctx.bot_service.clone(),
    });
    server.add_handler(MethodSetLogLevel {
        log_service: ctx.log_service.clone(),
    });
}