mod add_app_member;
mod create_app;
mod disable_message_persistence;
mod disable_support_user;
mod edit_app;
mod enable_message_persistence;
mod enable_support_user;
mod list_app_members;
mod list_apps;
mod set_app_member_role;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::app_admin::add_app_member::MethodAddAppMember;
use crate::handlers::app_admin::create_app::MethodCreateApp;
use crate::handlers::app_admin::disable_message_persistence::MethodDisableMessagePersistence;
use crate::handlers::app_admin::disable_support_user::MethodDisableSupportUser;
use crate::handlers::app_admin::edit_app::MethodEditApp;
use crate::handlers::app_admin::enable_message_persistence::MethodEnableMessagePersistence;
use crate::handlers::app_admin::enable_support_user::MethodEnableSupportUser;
use crate::handlers::app_admin::list_app_members::MethodListAppMembers;
use crate::handlers::app_admin::list_apps::MethodListApps;
use crate::handlers::app_admin::set_app_member_role::MethodSetAppMemberRole;

pub fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    server.add_handler(MethodCreateApp {
        app_service: ctx.app_service.clone(),
        bot_service: ctx.bot_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodEditApp {
        app_service: ctx.app_service.clone(),
        bot_service: ctx.bot_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodListApps {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodEnableSupportUser {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodDisableSupportUser {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodEnableMessagePersistence {
        app_service: ctx.app_service.clone(),
        message_store: ctx.message_store.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodDisableMessagePersistence {
        app_service: ctx.app_service.clone(),
        message_store: ctx.message_store.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodAddAppMember {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodSetAppMemberRole {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodListAppMembers {
        app_service: ctx.app_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}
