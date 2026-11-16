pub mod get_my_tg_handle;
pub mod set_my_tg_handle;

use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::WebsocketServer;
use honey_id_types::id_entities::UserPublicId;

use crate::app::AppCtx;
use crate::codegen::model::{EnumErrorCode, UserRole};
use crate::handlers::support::get_my_tg_handle::MethodGetMyTgHandle;
use crate::handlers::support::set_my_tg_handle::MethodSetMyTgHandle;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodSetMyTgHandle {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodGetMyTgHandle {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}

async fn signed_in_staff(
    ctx: &RequestContext,
    registry: &UserConnectionRegistry,
) -> Result<UserPublicId, CustomError> {
    if ctx.roles.contains(&(UserRole::App as u32))
        || ![UserRole::Admin, UserRole::AppAdmin, UserRole::Support]
            .iter()
            .any(|role| ctx.roles.contains(&(*role as u32)))
    {
        return Err(CustomError::new(EnumErrorCode::Forbidden)
            .with_message("Sign in with a support account to link Telegram"));
    }
    registry.get(ctx.connection_id).await.ok_or_else(|| {
        CustomError::new(EnumErrorCode::Unauthorized).with_message("Connection not authenticated")
    })
}
