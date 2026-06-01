pub mod get_my_tg_handle;
pub mod set_my_tg_handle;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::support::get_my_tg_handle::MethodGetMyTgHandle;
use crate::handlers::support::set_my_tg_handle::MethodSetMyTgHandle;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodSetMyTgHandle {
        support_info_table: ctx.db.support_info_table.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodGetMyTgHandle {
        support_info_table: ctx.db.support_info_table.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}
