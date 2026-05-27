// Platform handlers are disabled until endpoint schemas are added to config
// pub mod auth;
// mod add_support_user;
// mod create_app;
// mod delete_app;
// mod edit_app;
// mod list_apps;
// mod list_support_users;
// mod remove_support_user;
// mod set_log_level;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;

pub fn register_handlers(_server: &mut WebsocketServer, _ctx: &AppCtx) {
    // Platform handlers disabled - endpoint schemas not defined in config
    // Uncomment and add schema definitions to enable these handlers
}