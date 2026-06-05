use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::Mutex as StdMutex;
use tokio::sync::Mutex as AsyncMutex;
use tracing::warn;
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::ChatMessage;
use crate::db::schema::app_config::{AppConfigWorkTable, MessagePersistenceEnabledByPubIdQuery};
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_memory_message::{
    SupportMemoryMessageRow, SupportMemoryMessageWorkTable,
};
use crate::db::schema::support_message::{SupportMessageRow, SupportMessageWorkTable};
use crate::id_types::{AppPublicId, PackedNanoId, SessionId};

pub struct MessageStore {
    app_config_table: Arc<AppConfigWorkTable>,
    chat_session_table: Arc<ChatSessionWorkTable>,
    persisted_table: Arc<SupportMessageWorkTable>,
    memory_table: Arc<SupportMemoryMessageWorkTable>,
    app_locks: StdMutex<HashMap<PackedNanoId, Arc<AsyncMutex<()>>>>,
}

impl MessageStore {
    pub fn new(
        app_config_table: Arc<AppConfigWorkTable>,
        chat_session_table: Arc<ChatSessionWorkTable>,
        persisted_table: Arc<SupportMessageWorkTable>,
        memory_table: Arc<SupportMemoryMessageWorkTable>,
    ) -> Self {
        Self {
            app_config_table,
            chat_session_table,
            persisted_table,
            memory_table,
            app_locks: StdMutex::new(HashMap::new()),
        }
    }

    pub async fn store_message(&self, mut msg: SupportMessageRow) -> eyre::Result<()> {
        let app_lock = self.app_lock(msg.app_public_id);
        let _guard = app_lock.lock().await;

        if self.persistence_enabled(msg.app_public_id)? {
            msg.id = self.persisted_table.get_next_pk().into();
            self.persisted_table.insert(msg)?;
        } else {
            let mut memory_row: SupportMemoryMessageRow = msg.into();
            memory_row.id = self.memory_table.get_next_pk().into();
            self.memory_table.insert(memory_row)?;
        }

        Ok(())
    }

    pub async fn list_messages(&self, session_id: SessionId) -> eyre::Result<Vec<ChatMessage>> {
        let packed_session_id = session_id.pack()?;
        let session = self
            .chat_session_table
            .select_by_session_id(packed_session_id)
            .ok_or_else(|| eyre::eyre!("Session not found"))?;

        let app_lock = self.app_lock(session.app_public_id);
        let _guard = app_lock.lock().await;

        let mut messages: Vec<ChatMessage> = if self.persistence_enabled(session.app_public_id)? {
            self.persisted_table
                .select_by_session_id(packed_session_id)
                .execute()?
                .into_iter()
                .map(chat_message_from_persisted)
                .collect()
        } else {
            self.memory_table
                .select_by_session_id(packed_session_id)
                .execute()?
                .into_iter()
                .map(chat_message_from_memory)
                .collect()
        };

        messages.sort_by_key(|m| m.sent_at);
        Ok(messages)
    }

    pub async fn set_app_persistence(
        &self,
        app_public_id: AppPublicId,
        enabled: bool,
    ) -> eyre::Result<()> {
        let packed_app = app_public_id.pack()?;
        let app_lock = self.app_lock(packed_app);
        let _guard = app_lock.lock().await;
        let current = self.persistence_enabled(packed_app)?;

        if current == enabled {
            return Ok(());
        }

        if enabled {
            self.move_memory_to_persisted(packed_app).await?;
        } else {
            self.move_persisted_to_memory(packed_app).await?;
        }

        Ok(())
    }

    pub async fn purge_memory_before(&self, purge_all_before_ms: i64) -> eyre::Result<()> {
        let rows = self
            .memory_table
            .select_by_sent_at_range(..purge_all_before_ms)
            .execute()?;

        for row in rows {
            let app_lock = self.app_lock(row.app_public_id);
            let _guard = app_lock.lock().await;

            if self
                .memory_table
                .select_by_message_id(row.message_id)
                .is_some()
            {
                self.memory_table.delete(row.id).await?;
            }
        }

        Ok(())
    }

    pub fn spawn_purge_task(
        self: Arc<Self>,
        interval: Duration,
        retention: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;
                let cutoff = Utc::now().timestamp_millis() - retention.as_millis() as i64;
                if let Err(e) = self.purge_memory_before(cutoff).await {
                    warn!(error = %e, "failed to purge memory support messages");
                }
            }
        })
    }

    fn persistence_enabled(&self, app_public_id: PackedNanoId) -> eyre::Result<bool> {
        self.app_config_table
            .select_by_public_id(app_public_id)
            .map(|r| r.message_persistence_enabled)
            .ok_or_else(|| eyre::eyre!("App not found"))
    }

    fn app_lock(&self, app_public_id: PackedNanoId) -> Arc<AsyncMutex<()>> {
        let mut locks = self.app_locks.lock();
        locks
            .entry(app_public_id)
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    async fn move_persisted_to_memory(&self, app_public_id: PackedNanoId) -> eyre::Result<()> {
        let rows = self
            .persisted_table
            .select_by_app_public_id(app_public_id)
            .execute()?;

        for row in &rows {
            if self
                .memory_table
                .select_by_message_id(row.message_id)
                .is_none()
            {
                let mut memory_row: SupportMemoryMessageRow = row.clone().into();
                memory_row.id = self.memory_table.get_next_pk().into();
                self.memory_table.insert(memory_row)?;
            }
        }

        if rows.iter().any(|row| {
            self.memory_table
                .select_by_message_id(row.message_id)
                .is_none()
        }) {
            return Err(eyre::eyre!("message persistence transition copy failed"));
        }

        self.app_config_table
            .update_message_persistence_enabled_by_pub_id(
                MessagePersistenceEnabledByPubIdQuery {
                    message_persistence_enabled: false,
                },
                app_public_id,
            )
            .await?;

        for row in rows {
            if let Err(e) = self.persisted_table.delete(row.id).await {
                warn!(error = %e, "failed to clean persisted message after disabling persistence");
            }
        }

        Ok(())
    }

    async fn move_memory_to_persisted(&self, app_public_id: PackedNanoId) -> eyre::Result<()> {
        let rows = self
            .memory_table
            .select_by_app_public_id(app_public_id)
            .execute()?;

        for row in &rows {
            if self
                .persisted_table
                .select_by_message_id(row.message_id)
                .is_none()
            {
                let mut persisted_row: SupportMessageRow = row.clone().into();
                persisted_row.id = self.persisted_table.get_next_pk().into();
                self.persisted_table.insert(persisted_row)?;
            }
        }

        if rows.iter().any(|row| {
            self.persisted_table
                .select_by_message_id(row.message_id)
                .is_none()
        }) {
            return Err(eyre::eyre!("message persistence transition copy failed"));
        }

        self.app_config_table
            .update_message_persistence_enabled_by_pub_id(
                MessagePersistenceEnabledByPubIdQuery {
                    message_persistence_enabled: true,
                },
                app_public_id,
            )
            .await?;

        for row in rows {
            if let Err(e) = self.memory_table.delete(row.id).await {
                warn!(error = %e, "failed to clean memory message after enabling persistence");
            }
        }

        Ok(())
    }
}

fn chat_message_from_persisted(row: SupportMessageRow) -> ChatMessage {
    ChatMessage {
        session_id: row.session_id.unpack().expect("valid packed nanoid"),
        incoming: row.incoming,
        sent_by: row.sent_by,
        sent_at: row.sent_at,
        content: row.content,
    }
}

fn chat_message_from_memory(row: SupportMemoryMessageRow) -> ChatMessage {
    ChatMessage {
        session_id: row.session_id.unpack().expect("valid packed nanoid"),
        incoming: row.incoming,
        sent_by: row.sent_by,
        sent_at: row.sent_at,
        content: row.content,
    }
}
