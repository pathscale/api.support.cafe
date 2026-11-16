use std::fmt;
use std::sync::Arc;

use chrono::Utc;
use honey_id_types::id_entities::UserPublicId;
use psc_nanoid::{Nanoid, alphabet::Base62Alphabet};

use crate::codegen::model::ChatMessage;
use crate::db::schema::chat_session::{ChatSessionColumns, ChatSessionRow, ChatSessionWorkTable};
use crate::id_types::{AppPublicId, PackedNanoId, SessionId};
use crate::service::app::{AppService, AppStaffDenied};
use crate::service::bot::BotService;
use crate::service::chat_limits::{ChatWrite, ChatWriteError, ChatWriteLimiter};
use crate::service::message_store::MessageStore;
use worktable::prelude::SelectQueryExecutor;

/// Service for session operations.
/// Who is acting on a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    /// The visitor the chat belongs to.
    Visitor,
    /// The owner or an admin of the chat's desk, answering as support.
    Staff,
}

#[derive(Debug)]
pub(crate) enum SessionAccessError {
    Denied(&'static str),
    NotFound,
}

impl fmt::Display for SessionAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Denied(message) => message,
            Self::NotFound => "Session not found",
        })
    }
}

impl std::error::Error for SessionAccessError {}

pub struct ChatSessionService {
    chat_session_table: Arc<ChatSessionWorkTable>,
    bot_service: Arc<BotService>,
    message_store: Arc<MessageStore>,
    app_service: Arc<AppService>,
    write_limiter: ChatWriteLimiter,
}

impl ChatSessionService {
    pub fn new(
        chat_session_table: Arc<ChatSessionWorkTable>,
        bot_service: Arc<BotService>,
        message_store: Arc<MessageStore>,
        app_service: Arc<AppService>,
    ) -> Self {
        Self {
            chat_session_table,
            bot_service,
            message_store,
            app_service,
            write_limiter: ChatWriteLimiter::new(),
        }
    }

    /// Create a new chat session. Returns the created row.
    pub async fn create_session(
        &self,
        user_pub_id: UserPublicId,
        app_public_id: AppPublicId,
    ) -> eyre::Result<ChatSessionRow> {
        self.write_limiter
            .charge(app_public_id, user_pub_id, ChatWrite::Session)?;
        self.create_session_row(user_pub_id, app_public_id).await
    }

    /// A contact form creates a session and sends its first message. Reserve
    /// both units before creating the row so throttling cannot leave it empty.
    pub(crate) async fn submit_contact_message(
        &self,
        user_pub_id: UserPublicId,
        app_public_id: AppPublicId,
        content: String,
    ) -> eyre::Result<(ChatSessionRow, i64)> {
        self.write_limiter
            .charge(app_public_id, user_pub_id, ChatWrite::Contact)?;
        let row = self.create_session_row(user_pub_id, app_public_id).await?;
        let session_id = SessionId::from_packed(row.session_id)?;
        let sent_at = self
            .send_authorized(
                app_public_id,
                session_id,
                content,
                user_pub_id,
                ChatRole::Visitor,
            )
            .await?;
        Ok((row, sent_at))
    }

    async fn create_session_row(
        &self,
        user_pub_id: UserPublicId,
        app_public_id: AppPublicId,
    ) -> eyre::Result<ChatSessionRow> {
        tracing::debug!(
            app_public_id = %app_public_id,
            user_pub_id = %user_pub_id,
            "SessionService::create_session: creating"
        );

        let session_id_nanoid = Nanoid::<16, Base62Alphabet>::new();
        let session_id: SessionId = session_id_nanoid.into();

        let created_at = Utc::now().timestamp_millis();

        let row = ChatSessionRow {
            id: self.chat_session_table.get_next_pk().into(),
            session_id: session_id.pack()?,
            app_public_id: app_public_id.pack()?,
            user_pub_id: user_pub_id.pack()?,
            created_at,
            closed_at: None,
        };
        self.chat_session_table.insert(row.clone()).await?;

        tracing::debug!(
            session_id = %session_id,
            app_public_id = %app_public_id,
            user_pub_id = %user_pub_id,
            "SessionService::create_session: completed"
        );

        Ok(row)
    }

    /// Close a session by setting closed_at timestamp.
    pub async fn close_session(
        &self,
        session_id: SessionId,
        user_pub_id: UserPublicId,
        connection_app_id: Option<AppPublicId>,
    ) -> eyre::Result<()> {
        tracing::debug!(
            session_id = %session_id,
            user_pub_id = %user_pub_id,
            "SessionService::close_session: closing"
        );

        let (row, _) =
            self.verify_session_access_for_connection(session_id, user_pub_id, connection_app_id)?;

        let closed_at = Utc::now().timestamp_millis();
        self.chat_session_table
            .update_by_id(row.id, ChatSessionColumns::CLOSED_AT, Some(closed_at))
            .await?;

        tracing::debug!(
            session_id = %session_id,
            closed_at,
            "SessionService::close_session: completed"
        );

        Ok(())
    }

    /// Send a message into a session: from its visitor, or from the desk's
    /// staff, which the visitor sees as a support reply like one from Telegram.
    pub async fn send_message(
        &self,
        session_id: SessionId,
        content: String,
        user_pub_id: UserPublicId,
        connection_app_id: Option<AppPublicId>,
    ) -> eyre::Result<i64> {
        tracing::debug!(
            session_id = %session_id,
            content_len = content.len(),
            user_pub_id = %user_pub_id,
            "SessionService::send_message: sending"
        );

        let (row, role) =
            self.verify_session_access_for_connection(session_id, user_pub_id, connection_app_id)?;
        let app_public_id = AppPublicId::from_packed(row.app_public_id)?;
        if role == ChatRole::Visitor {
            const MAX_CONTENT_CHARS: usize = 5_000;
            if content.chars().take(MAX_CONTENT_CHARS + 1).count() > MAX_CONTENT_CHARS {
                return Err(ChatWriteError::MessageTooLarge.into());
            }
            self.write_limiter
                .charge(app_public_id, user_pub_id, ChatWrite::Message)?;
        }
        self.send_authorized(app_public_id, session_id, content, user_pub_id, role)
            .await
    }

    async fn send_authorized(
        &self,
        app_public_id: AppPublicId,
        session_id: SessionId,
        content: String,
        user_pub_id: UserPublicId,
        role: ChatRole,
    ) -> eyre::Result<i64> {
        let sent = match role {
            ChatRole::Visitor => {
                self.bot_service
                    .send_message(app_public_id, session_id, content, "User".to_string())
                    .await
            }
            ChatRole::Staff => {
                self.bot_service
                    .send_support_reply(app_public_id, session_id, content, user_pub_id)
                    .await
            }
        };
        let sent_at = sent.map_err(|e| eyre::eyre!("Failed to send message: {e}"))?;

        tracing::debug!(
            session_id = %session_id,
            sent_at,
            "SessionService::send_message: completed"
        );

        Ok(sent_at)
    }

    /// Check if session belongs to specific app.
    pub fn is_for_app(
        &self,
        session_id: SessionId,
        app_public_id: AppPublicId,
    ) -> eyre::Result<bool> {
        tracing::debug!(
            session_id = %session_id,
            app_public_id = %app_public_id,
            "SessionService::is_for_app: checking"
        );

        let packed_session_id: PackedNanoId = session_id.pack()?;
        let packed_app_id: PackedNanoId = app_public_id.pack()?;

        let belongs = match self
            .chat_session_table
            .select_by_session_id(packed_session_id)
        {
            Some(session) => session.app_public_id == packed_app_id,
            None => false,
        };

        tracing::debug!(
            session_id = %session_id,
            app_public_id = %app_public_id,
            belongs,
            "SessionService::is_for_app: completed"
        );

        Ok(belongs)
    }

    /// Whether `user_pub_id` may act on the session, and as whom: the visitor
    /// it belongs to, or the owner or an admin of its desk. Before, only the
    /// visitor could, so a desk's managers could list their chats but not
    /// read, answer or close any of them.
    /// Verify access using the identity registered for this connection.
    /// App connections are visitors scoped to their registered app and visitor
    /// id; they must never use the signed-in staff fallback.
    pub fn verify_session_access_for_connection(
        &self,
        session_id: SessionId,
        user_pub_id: UserPublicId,
        connection_app_id: Option<AppPublicId>,
    ) -> eyre::Result<(ChatSessionRow, ChatRole)> {
        let packed_session_id: PackedNanoId = session_id.pack()?;
        let packed_user_id: PackedNanoId = user_pub_id.pack()?;

        let session = self
            .chat_session_table
            .select_by_session_id(packed_session_id)
            .ok_or(SessionAccessError::NotFound)?;

        Self::verify_row_access_for_connection(
            session,
            packed_user_id,
            connection_app_id,
            |app_public_id| {
                self.app_service
                    .ensure_app_admin_or_owner(app_public_id, user_pub_id)
            },
        )
    }

    /// Check a loaded session row without requiring a WorkTable or application service.
    fn verify_row_access_for_connection<F>(
        session: ChatSessionRow,
        packed_user_id: PackedNanoId,
        connection_app_id: Option<AppPublicId>,
        ensure_staff: F,
    ) -> eyre::Result<(ChatSessionRow, ChatRole)>
    where
        F: FnOnce(AppPublicId) -> eyre::Result<()>,
    {
        if let Some(connection_app_id) = connection_app_id {
            if session.app_public_id != connection_app_id.pack()? {
                return Err(eyre::Report::new(SessionAccessError::Denied(
                    "Session does not belong to this app",
                )));
            }
            if session.user_pub_id != packed_user_id {
                return Err(eyre::Report::new(SessionAccessError::Denied(
                    "Session does not belong to this user",
                )));
            }
            return Ok((session, ChatRole::Visitor));
        }

        if session.user_pub_id == packed_user_id {
            return Ok((session, ChatRole::Visitor));
        }
        let app_public_id = AppPublicId::from_packed(session.app_public_id)?;
        match ensure_staff(app_public_id) {
            Ok(()) => return Ok((session, ChatRole::Staff)),
            Err(error) if error.is::<AppStaffDenied>() => {}
            Err(error) => return Err(error),
        }
        tracing::warn!(
            session_id = ?session.session_id,
            requester_user_id = ?packed_user_id,
            "SessionService::verify_session_access: neither the visitor nor the desk's staff"
        );
        Err(eyre::Report::new(SessionAccessError::Denied(
            "Session does not belong to this user",
        )))
    }

    /// Whether `app` exists and is active, so a signed-in user may open a chat
    /// with it.
    pub fn app_accepts_chats(&self, app: AppPublicId) -> eyre::Result<bool> {
        Ok(self.app_service.get_app(app)?.is_some_and(|row| row.active))
    }

    /// List messages for session.
    pub async fn list_messages(&self, session_id: SessionId) -> eyre::Result<Vec<ChatMessage>> {
        let messages = self.message_store.list_messages(session_id).await?;

        Ok(messages)
    }

    /// List sessions for user, optionally filtered by app.
    pub fn list_sessions(
        &self,
        user_pub_id: UserPublicId,
        app_filter: Option<AppPublicId>,
    ) -> eyre::Result<Vec<ChatSessionRow>> {
        tracing::debug!(
            user_pub_id = %user_pub_id,
            app_filter = ?app_filter,
            "SessionService::list_sessions: querying"
        );

        let packed_user_id: PackedNanoId = user_pub_id.pack()?;

        let rows = self
            .chat_session_table
            .select_all()
            .execute()
            .inspect_err(|e| {
                tracing::error!(
                    user_pub_id = %user_pub_id,
                    error = %e,
                    "SessionService::list_sessions: query failed"
                );
            })?;

        let sessions: Vec<ChatSessionRow> = rows
            .into_iter()
            .filter(|r| r.user_pub_id == packed_user_id)
            .filter(|r| {
                if let Some(app_id) = &app_filter {
                    let packed_app_id: PackedNanoId = app_id.pack().expect("app_id packs");
                    r.app_public_id == packed_app_id
                } else {
                    true
                }
            })
            .collect();

        tracing::debug!(
            user_pub_id = %user_pub_id,
            count = sessions.len(),
            "SessionService::list_sessions: completed"
        );

        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{AppStaffDenied, ChatRole, ChatSessionService, SessionAccessError};
    use crate::db::schema::chat_session::ChatSessionRow;
    use crate::id_types::{AppPublicId, NanoId, PackedNanoId};

    fn packed_id() -> PackedNanoId {
        PackedNanoId::pack(&NanoId::new()).expect("fresh id packs")
    }

    fn session_row(app_public_id: PackedNanoId, user_pub_id: PackedNanoId) -> ChatSessionRow {
        ChatSessionRow {
            id: 1,
            session_id: packed_id(),
            app_public_id,
            user_pub_id,
            created_at: 0,
            closed_at: None,
        }
    }

    fn app_id(packed: PackedNanoId) -> AppPublicId {
        AppPublicId::from_packed(packed).expect("fresh app id unpacks")
    }

    #[test]
    fn app_connection_allows_its_visitor() {
        let app = packed_id();
        let user = packed_id();
        let (row, role) = ChatSessionService::verify_row_access_for_connection(
            session_row(app, user),
            user,
            Some(app_id(app)),
            |_| panic!("App scope must skip staff authorization"),
        )
        .expect("same app visitor is allowed");

        assert_eq!(row.app_public_id, app);
        assert_eq!(role, ChatRole::Visitor);
    }

    #[test]
    fn app_connection_rejects_another_app() {
        let session_app = packed_id();
        let connection_app = app_id(packed_id());
        let user = packed_id();
        let error = ChatSessionService::verify_row_access_for_connection(
            session_row(session_app, user),
            user,
            Some(connection_app),
            |_| panic!("App scope must skip staff authorization"),
        )
        .expect_err("cross-app access is rejected");

        assert_eq!(error.to_string(), "Session does not belong to this app");
    }

    #[test]
    fn app_connection_cannot_escalate_a_different_visitor_to_staff() {
        let app = packed_id();
        let stored_user = packed_id();
        let requester = packed_id();
        let staff_check_called = Cell::new(false);
        let error = ChatSessionService::verify_row_access_for_connection(
            session_row(app, stored_user),
            requester,
            Some(app_id(app)),
            |_| {
                staff_check_called.set(true);
                Ok(())
            },
        )
        .expect_err("app connection cannot use staff authorization");

        assert_eq!(error.to_string(), "Session does not belong to this user");
        assert!(!staff_check_called.get());
    }

    #[test]
    fn init_connection_uses_staff_fallback() {
        let app = packed_id();
        let stored_user = packed_id();
        let requester = packed_id();
        let expected_app = app_id(app);
        let (row, role) = ChatSessionService::verify_row_access_for_connection(
            session_row(app, stored_user),
            requester,
            None,
            |checked_app| {
                assert_eq!(checked_app, expected_app);
                Ok(())
            },
        )
        .expect("staff member is allowed");

        assert_eq!(row.app_public_id, app);
        assert_eq!(role, ChatRole::Staff);
    }

    #[test]
    fn staff_fallback_distinguishes_denial_from_operational_failure() {
        let app = packed_id();
        let visitor = packed_id();
        let requester = packed_id();

        let denied = ChatSessionService::verify_row_access_for_connection(
            session_row(app, visitor),
            requester,
            None,
            |_| Err(AppStaffDenied::NotMember.into()),
        )
        .expect_err("a non-member is denied");
        assert!(matches!(
            denied.downcast_ref::<SessionAccessError>(),
            Some(SessionAccessError::Denied(_))
        ));

        let failure = ChatSessionService::verify_row_access_for_connection(
            session_row(app, visitor),
            requester,
            None,
            |_| Err(std::io::Error::other("staff lookup failed").into()),
        )
        .expect_err("the operational failure is propagated");
        assert!(failure.is::<std::io::Error>());
    }
}
