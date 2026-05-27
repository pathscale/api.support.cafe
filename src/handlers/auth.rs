use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::codegen::model::EnumEndpoint;

pub fn register_auth_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    let mut auth_controller = endpoint_libs::libs::ws::EndpointAuthController::default();
    auth_controller.add_auth_endpoint(
        EnumEndpoint::AppConnect.schema(),
        crate::handlers::app::auth::MethodAppConnect {
            app_connection_registry: ctx.app_connection_registry.clone(),
        },
    );
    server.set_auth_controller(auth_controller);
}