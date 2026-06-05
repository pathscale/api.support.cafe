pub mod auth;
mod close_chat_session;
mod create_chat_session;
mod list_chat_sessions;
mod list_messages;
mod send_message;
mod subscribe_events;

use std::sync::Arc;

use endpoint_libs::libs::ws::WebsocketServer;

use crate::app::AppCtx;
use crate::handlers::app::close_chat_session::MethodCloseChatSession;
use crate::handlers::app::create_chat_session::MethodCreateChatSession;
use crate::handlers::app::list_chat_sessions::MethodListChatSessions;
use crate::handlers::app::list_messages::MethodListMessages;
use crate::handlers::app::send_message::MethodSendMessage;
use crate::handlers::app::subscribe_events::MethodSubscribeEvents;
use crate::handlers::utils::subscription_router::SubscriptionRouter;

pub async fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    let event_stream = ctx.bot_service.take_event_stream().await.expect("event stream already taken");
    let event_router = Arc::new(SubscriptionRouter::new(1, event_stream, server.toolbox.clone()));

    server.add_handler(MethodCreateChatSession {
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
    server.add_handler(MethodCloseChatSession {
        session_service: ctx.session_service.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodListChatSessions {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}
