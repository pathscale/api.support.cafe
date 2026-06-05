pub mod get_my_info;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::user::get_my_info::MethodGetMyInfo;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodGetMyInfo {
        user_service: ctx.user_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}
