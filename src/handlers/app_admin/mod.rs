mod add_app_member;
mod add_support_user;
mod create_app;
mod edit_app;
mod list_app_members;
mod list_apps;
mod list_support_users;
mod remove_support_user;
mod set_app_member_role;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::app_admin::add_app_member::MethodAddAppMember;
use crate::handlers::app_admin::add_support_user::MethodAddSupportUser;
use crate::handlers::app_admin::create_app::MethodCreateApp;
use crate::handlers::app_admin::edit_app::MethodEditApp;
use crate::handlers::app_admin::list_app_members::MethodListAppMembers;
use crate::handlers::app_admin::list_apps::MethodListApps;
use crate::handlers::app_admin::list_support_users::MethodListSupportUsers;
use crate::handlers::app_admin::remove_support_user::MethodRemoveSupportUser;
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
    server.add_handler(MethodAddSupportUser {
        app_service: ctx.app_service.clone(),
        support_user_table: ctx.db.support_user_table.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodRemoveSupportUser {
        app_service: ctx.app_service.clone(),
        support_user_table: ctx.db.support_user_table.clone(),
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
    server.add_handler(MethodListSupportUsers {
        app_service: ctx.app_service.clone(),
        support_user_table: ctx.db.support_user_table.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}
