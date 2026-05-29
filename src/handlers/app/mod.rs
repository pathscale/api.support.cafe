pub mod auth;
mod close_session;
mod create_session;
mod list_messages;
mod list_sessions;
mod send_message;
mod subscribe_events;

use std::sync::Arc;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::app::close_session::MethodCloseSession;
use crate::handlers::app::create_session::MethodCreateSession;
use crate::handlers::app::list_messages::MethodListMessages;
use crate::handlers::app::list_sessions::MethodListSessions;
use crate::handlers::app::send_message::MethodSendMessage;
use crate::handlers::app::subscribe_events::MethodSubscribeEvents;
use crate::handlers::utils::subscription_router::SubscriptionRouter;

pub async fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    let event_stream = ctx.bot_service.take_event_stream().await.expect("event stream already taken");
    let event_router = Arc::new(SubscriptionRouter::new(1, event_stream, server.toolbox.clone()));

    server.add_handler(MethodCreateSession {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
    });
    server.add_handler(MethodSendMessage {
        session_service: ctx.session_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodListMessages {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodSubscribeEvents {
        event_router,
        session_service: ctx.session_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodCloseSession {
        session_service: ctx.session_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodListSessions {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}