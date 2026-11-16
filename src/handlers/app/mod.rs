pub mod auth;
mod close_chat_session;
mod create_chat_session;
mod list_chat_sessions;
mod list_messages;
mod send_message;
mod submit_contact_message;
mod subscribe_events;

use std::sync::Arc;

use endpoint_libs::libs::toolbox::CustomError;
use endpoint_libs::libs::ws::{WebsocketServer, handler::HandlerError};

use crate::app::AppCtx;
use crate::codegen::model::EnumErrorCode;
use crate::handlers::app::close_chat_session::MethodCloseChatSession;
use crate::handlers::app::create_chat_session::MethodCreateChatSession;
use crate::handlers::app::list_chat_sessions::MethodListChatSessions;
use crate::handlers::app::list_messages::MethodListMessages;
use crate::handlers::app::send_message::MethodSendMessage;
use crate::handlers::app::submit_contact_message::MethodSubmitContactMessage;
use crate::handlers::app::subscribe_events::MethodSubscribeEvents;
use crate::handlers::utils::subscription_router::SubscriptionRouter;
use crate::service::chat_limits::ChatWriteError;
use crate::service::session::SessionAccessError;

fn map_session_error(error: eyre::Report) -> HandlerError<CustomError> {
    if let Some(error) = error.downcast_ref::<ChatWriteError>() {
        let code = match error {
            ChatWriteError::RateExceeded => EnumErrorCode::TooManyRequests,
            ChatWriteError::MessageTooLarge => EnumErrorCode::PayloadTooLarge,
        };
        return CustomError::new(code)
            .with_message(error.to_string())
            .into();
    }
    if matches!(
        error.downcast_ref::<SessionAccessError>(),
        Some(SessionAccessError::NotFound | SessionAccessError::Denied(_))
    ) {
        return CustomError::new(EnumErrorCode::Forbidden)
            .with_message("Session access denied")
            .into();
    }
    HandlerError::internal(error)
}

pub async fn register_handlers(server: &mut WebsocketServer, ctx: &AppCtx) {
    let event_stream = ctx
        .bot_service
        .take_event_stream()
        .expect("event stream already taken");
    let event_router = Arc::new(SubscriptionRouter::new(
        1,
        event_stream,
        server.toolbox.clone(),
    ));

    server.add_handler(MethodCreateChatSession {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodSendMessage {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
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
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodCloseChatSession {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodListChatSessions {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
    server.add_handler(MethodSubmitContactMessage {
        session_service: ctx.session_service.clone(),
        app_connection_registry: ctx.app_connection_registry.clone(),
        user_connection_registry: ctx.user_connection_registry.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_error_mapping_preserves_public_errors_and_keeps_reports_internal() {
        let forbidden = CustomError::new(EnumErrorCode::Forbidden).code;
        let assert_public_error = |error: eyre::Report| match map_session_error(error) {
            HandlerError::Public(error) => {
                assert_eq!(error.code, forbidden);
                assert_eq!(error.params["message"], "Session access denied");
            }
            HandlerError::Internal(report) => panic!("expected public error, got {report:?}"),
            HandlerError::NoResponse => panic!("expected public error, got NoResponse"),
        };

        assert_public_error(SessionAccessError::NotFound.into());
        assert_public_error(
            SessionAccessError::Denied("Session does not belong to this app").into(),
        );
        assert_public_error(
            SessionAccessError::Denied("Session does not belong to this user").into(),
        );

        assert!(matches!(
            map_session_error(std::io::Error::other("staff lookup failed").into()),
            HandlerError::Internal(report) if report.is::<std::io::Error>()
        ));
    }

    #[cfg(not(feature = "s3-sync"))]
    #[test]
    fn resource_admission_chat_handlers_share_budget_across_connections() {
        use endpoint_libs::libs::toolbox::RequestContext;
        use endpoint_libs::libs::ws::handler::RequestHandler;
        use honey_id_types::id_entities::UserPublicId;

        use crate::codegen::model::{
            CreateChatSessionRequest, SendMessageRequest, SubmitContactMessageRequest,
        };
        use crate::config::DatabaseConfig;
        use crate::db::schema::app_config::AppConfigRow;
        use crate::db::tables::Tables;
        use crate::id_types::{AppPublicId, NanoId};
        use crate::service::app::AppService;
        use crate::service::app_connection_registry::AppConnectionRegistry;
        use crate::service::bot::BotService;
        use crate::service::message_store::MessageStore;
        use crate::service::session::ChatSessionService;
        use crate::service::user_connection_registry::UserConnectionRegistry;

        let path = std::env::temp_dir().join(format!("cafe-admission-{}", NanoId::new()));
        std::fs::create_dir(&path).unwrap();
        nagoya::block_on(async {
            let db = Tables::new(DatabaseConfig { path: path.clone() })
                .await
                .unwrap();
            let app = AppPublicId::from(NanoId::new());
            db.app_config_table
                .insert(AppConfigRow {
                    id: db.app_config_table.get_next_pk().into(),
                    public_id: app.pack().unwrap(),
                    tg_bot_token: String::new(),
                    app_name: None,
                    active: true,
                    message_persistence_enabled: false,
                    created_at: 0,
                })
                .await
                .unwrap();
            let messages = Arc::new(MessageStore::new(
                db.app_config_table.clone(),
                db.chat_session_table.clone(),
                db.support_message_table.clone(),
                db.support_memory_message_table.clone(),
                db.app_slot_table.clone(),
            ));
            let apps = Arc::new(AppService::new(
                db.app_config_table.clone(),
                db.app_member_table.clone(),
                db.support_info_table.clone(),
                db.user_table.clone(),
                messages.clone(),
                db.telegram_link_table.clone(),
            ));
            let bots = Arc::new(BotService::new(
                db.app_member_table.clone(),
                db.chat_session_table.clone(),
                db.support_info_table.clone(),
                messages.clone(),
                apps.clone(),
            ));
            drop(bots.take_event_stream().unwrap());
            let sessions = Arc::new(ChatSessionService::new(
                db.chat_session_table.clone(),
                bots,
                messages,
                apps,
            ));
            let app_registry = Arc::new(AppConnectionRegistry::new());
            let user_registry = Arc::new(UserConnectionRegistry::new());
            let user = UserPublicId::default();
            for connection in [1, 2] {
                app_registry.register(connection, app).await;
                user_registry.register(connection, user).await;
            }
            let create = MethodCreateChatSession {
                session_service: sessions.clone(),
                app_connection_registry: app_registry.clone(),
                user_connection_registry: user_registry.clone(),
            };
            let send = MethodSendMessage {
                session_service: sessions.clone(),
                app_connection_registry: app_registry.clone(),
                user_connection_registry: user_registry.clone(),
            };
            let submit = MethodSubmitContactMessage {
                session_service: sessions.clone(),
                app_connection_registry: app_registry.clone(),
                user_connection_registry: user_registry.clone(),
            };
            let mut first = RequestContext::empty();
            first.connection_id = 1;
            let mut second = RequestContext::empty();
            second.connection_id = 2;
            let create_request = CreateChatSessionRequest {
                user_pub_id: user.into(),
                app_public_id: None,
            };
            let session_id = create
                .handle(first.clone(), create_request.clone())
                .await
                .unwrap()
                .session_id;
            let message = SendMessageRequest {
                session_id,
                content: "é".repeat(5_001),
            };
            assert!(matches!(
                send.handle(first.clone(), message).await,
                Err(HandlerError::Public(err)) if err.code.to_u32() == EnumErrorCode::PayloadTooLarge as u32
            ));
            let message = SendMessageRequest {
                session_id,
                content: "é".repeat(5_000),
            };
            for _ in 0..60 {
                send.handle(first.clone(), message.clone()).await.unwrap();
            }
            let denied = send.handle(second.clone(), message).await;
            assert!(matches!(
                denied,
                Err(HandlerError::Public(err)) if err.code.to_u32() == EnumErrorCode::TooManyRequests as u32
            ));
            let denied = submit
                .handle(
                    second.clone(),
                    SubmitContactMessageRequest {
                        name: "Visitor".to_owned(),
                        email: "visitor@example.test".to_owned(),
                        message: "Hello".to_owned(),
                    },
                )
                .await;
            assert!(matches!(
                denied,
                Err(HandlerError::Public(err)) if err.code.to_u32() == EnumErrorCode::TooManyRequests as u32
            ));
            assert_eq!(sessions.list_sessions(user, Some(app)).unwrap().len(), 1);
            assert_eq!(
                sessions
                    .list_messages(session_id.into())
                    .await
                    .unwrap()
                    .len(),
                60
            );
            for _ in 0..4 {
                create
                    .handle(second.clone(), create_request.clone())
                    .await
                    .unwrap();
            }
            assert!(matches!(
                create.handle(second, create_request).await,
                Err(HandlerError::Public(err)) if err.code.to_u32() == EnumErrorCode::TooManyRequests as u32
            ));
            db.wait_for_ops().await.unwrap();
        });
        std::fs::remove_dir_all(path).unwrap();
    }
}
