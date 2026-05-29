use std::sync::Arc;

use chrono::Utc;
use eyre::bail;
use honey_id_types::id_entities::UserPublicId;
use psc_nanoid::{alphabet::Base62Alphabet, Nanoid};

use crate::codegen::model::ChatMessage;
use crate::db::schema::chat_session::{ChatSessionRow, ChatSessionWorkTable, ClosedAtByIdQuery};
use crate::db::schema::support_message::SupportMessageWorkTable;
use crate::id_types::{AppPublicId, PackedNanoId, SessionId};
use crate::service::bot::BotService;
use worktable::prelude::SelectQueryExecutor;

/// Session ownership verification result.
#[derive(Debug, Clone)]
pub struct VerifiedSession {
    pub session_id: SessionId,
    pub app_public_id: AppPublicId,
    pub user_pub_id: UserPublicId,
    pub row_id: u64,
}

/// Result of successful session creation.
#[derive(Debug, Clone)]
pub struct CreatedSession {
    pub session_id: SessionId,
    pub created_at: i64,
}

/// Session information for listing.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub app_public_id: AppPublicId,
    pub user_pub_id: UserPublicId,
    pub created_at: i64,
    pub closed_at: Option<i64>,
}

/// Service for session operations.
pub struct SessionService {
    chat_session_table: Arc<ChatSessionWorkTable>,
    support_message_table: Arc<SupportMessageWorkTable>,
    bot_service: Arc<BotService>,
}

impl SessionService {
    pub fn new(
        chat_session_table: Arc<ChatSessionWorkTable>,
        support_message_table: Arc<SupportMessageWorkTable>,
        bot_service: Arc<BotService>,
    ) -> Self {
        Self {
            chat_session_table,
            support_message_table,
            bot_service,
        }
    }

    /// Create a new chat session.
    pub fn create_session(
        &self,
        user_pub_id: UserPublicId,
        app_public_id: AppPublicId,
    ) -> eyre::Result<CreatedSession> {
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
        self.chat_session_table.insert(row)?;

        tracing::debug!(
            session_id = %session_id,
            app_public_id = %app_public_id,
            user_pub_id = %user_pub_id,
            "SessionService::create_session: completed"
        );

        Ok(CreatedSession { session_id, created_at })
    }

    /// Close a session by setting closed_at timestamp.
    pub async fn close_session(
        &self,
        session_id: SessionId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        tracing::debug!(
            session_id = %session_id,
            user_pub_id = %user_pub_id,
            "SessionService::close_session: closing"
        );

        let verified = self.verify_session_access(session_id, user_pub_id)?;

        let closed_at = Utc::now().timestamp_millis();
        self.chat_session_table
            .update_closed_at_by_id(
                ClosedAtByIdQuery { closed_at: Some(closed_at) },
                verified.row_id,
            )
            .await?;

        tracing::debug!(
            session_id = %session_id,
            closed_at,
            "SessionService::close_session: completed"
        );

        Ok(())
    }

    /// Send a message via BotService.
    /// User must own the session. App context is taken from session.
    pub async fn send_message(
        &self,
        session_id: SessionId,
        content: String,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<i64> {
        tracing::debug!(
            session_id = %session_id,
            content_len = content.len(),
            user_pub_id = %user_pub_id,
            "SessionService::send_message: sending"
        );

        let verified = self.verify_session_access(session_id, user_pub_id)?;

        let sent_at = self
            .bot_service
            .send_message(verified.app_public_id, session_id, content, "User".to_string())
            .await
            .map_err(|e| eyre::eyre!("Failed to send message: {e}"))?;

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

        let belongs = match self.chat_session_table.select_by_session_id(packed_session_id) {
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

    /// Verify session exists and belongs to user.
    pub fn verify_session_access(
        &self,
        session_id: SessionId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<VerifiedSession> {
        tracing::debug!(
            session_id = %session_id,
            user_pub_id = %user_pub_id,
            "SessionService::verify_session_access: checking"
        );

        let packed_session_id: PackedNanoId = session_id.pack()?;
        let packed_user_id: PackedNanoId = user_pub_id.pack()?;

        let session = self
            .chat_session_table
            .select_by_session_id(packed_session_id)
            .ok_or_else(|| eyre::eyre!("Session not found"))?;

        if session.user_pub_id != packed_user_id {
            tracing::warn!(
                session_id = %session_id,
                requester_user_id = %user_pub_id,
                session_user_id = ?session.user_pub_id,
                "SessionService::verify_session_access: session belongs to different user"
            );
            bail!("Session does not belong to this user");
        }

        tracing::debug!(
            session_id = %session_id,
            "SessionService::verify_session_access: verified"
        );

        Ok(VerifiedSession {
            session_id,
            app_public_id: AppPublicId::from_packed(session.app_public_id)?,
            user_pub_id: UserPublicId::unpack(session.user_pub_id)
                .map_err(|e| eyre::eyre!("Failed to unpack user_pub_id: {}", e))?,
            row_id: session.id,
        })
    }

    /// List messages for session.
    pub fn list_messages(&self, session_id: SessionId) -> eyre::Result<Vec<ChatMessage>> {
        tracing::debug!(
            session_id = %session_id,
            "SessionService::list_messages: querying"
        );

        let packed_session_id: PackedNanoId = session_id.pack()?;
        let rows = self
            .support_message_table
            .select_by_session_id(packed_session_id)
            .execute()
            .inspect_err(|e| {
                tracing::error!(
                    session_id = %session_id,
                    error = %e,
                    "SessionService::list_messages: query failed"
                );
            })?;

        let messages: Vec<ChatMessage> = rows
            .into_iter()
            .map(|r| ChatMessage {
                incoming: r.incoming,
                sent_at: r.sent_at,
                content: r.content,
            })
            .collect();

        tracing::debug!(
            session_id = %session_id,
            count = messages.len(),
            "SessionService::list_messages: completed"
        );

        Ok(messages)
    }

    /// List sessions for user, optionally filtered by app.
    pub fn list_sessions(
        &self,
        user_pub_id: UserPublicId,
        app_filter: Option<AppPublicId>,
    ) -> eyre::Result<Vec<SessionInfo>> {
        tracing::debug!(
            user_pub_id = %user_pub_id,
            app_filter = ?app_filter,
            "SessionService::list_sessions: querying"
        );

        let packed_user_id: PackedNanoId = user_pub_id.pack()?;

        let rows = self.chat_session_table.select_all().execute().inspect_err(|e| {
            tracing::error!(
                user_pub_id = %user_pub_id,
                error = %e,
                "SessionService::list_sessions: query failed"
            );
        })?;

        let sessions: Vec<SessionInfo> = rows
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
            .map(|r| {
                Ok(SessionInfo {
                    session_id: SessionId::from_packed(r.session_id)?,
                    app_public_id: AppPublicId::from_packed(r.app_public_id)?,
                    user_pub_id: UserPublicId::unpack(r.user_pub_id)
                        .map_err(|e| eyre::eyre!("Failed to unpack user_pub_id: {}", e))?,
                    created_at: r.created_at,
                    closed_at: r.closed_at,
                })
            })
            .collect::<eyre::Result<Vec<_>>>()?;

        tracing::debug!(
            user_pub_id = %user_pub_id,
            count = sessions.len(),
            "SessionService::list_sessions: completed"
        );

        Ok(sessions)
    }
}