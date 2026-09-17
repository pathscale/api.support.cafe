use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::warn;
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::ChatMessage;
use crate::db::message_partitions::PersistedMessages;
use crate::db::schema::app_config::{AppConfigColumns, AppConfigWorkTable};
use crate::db::schema::app_slot::{AppSlotRow, AppSlotWorkTable};
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_memory_message::{
    SupportMemoryMessagePartitions, SupportMemoryMessageRow,
};
use crate::db::schema::support_message::SupportMessageRow;
use crate::id_types::{AppPublicId, PackedNanoId, SessionId};

pub struct MessageStore {
    app_config_table: Arc<AppConfigWorkTable>,
    chat_session_table: Arc<ChatSessionWorkTable>,
    persisted_table: Arc<PersistedMessages>,
    memory_table: Arc<SupportMemoryMessagePartitions>,
    app_slot_table: Arc<AppSlotWorkTable>,
}

impl MessageStore {
    pub fn new(
        app_config_table: Arc<AppConfigWorkTable>,
        chat_session_table: Arc<ChatSessionWorkTable>,
        persisted_table: Arc<PersistedMessages>,
        memory_table: Arc<SupportMemoryMessagePartitions>,
        app_slot_table: Arc<AppSlotWorkTable>,
    ) -> Self {
        Self {
            app_config_table,
            chat_session_table,
            persisted_table,
            memory_table,
            app_slot_table,
        }
    }

    pub async fn store_message(&self, mut msg: SupportMessageRow) -> eyre::Result<()> {
        let slot = self.app_slot(msg.app_public_id).await?;

        if self.persistence_enabled(msg.app_public_id)? {
            let table = self.persisted_table.get_or_create(slot).await?;
            msg.id = table.get_next_pk().into();
            table.insert(msg).await?;
        } else {
            let table = self.memory_table.partition_or_create(slot)?;
            let mut memory_row: SupportMemoryMessageRow = msg.into();
            memory_row.id = table.get_next_pk().into();
            table.insert(memory_row).await?;
        }

        Ok(())
    }

    pub async fn list_messages(&self, session_id: SessionId) -> eyre::Result<Vec<ChatMessage>> {
        let packed_session_id = session_id.pack()?;
        let session = self
            .chat_session_table
            .select_by_session_id(packed_session_id)
            .ok_or_else(|| eyre::eyre!("Session not found"))?;

        // Nor does reading mint a slot. A slot is minted by the first write for
        // an app; an app with none has no messages to list.
        let Some(slot) = self.existing_app_slot(session.app_public_id) else {
            return Ok(Vec::new());
        };

        // Reading does not open a partition. An app that has never stored a
        // message has no partition, and an absent one is an empty result rather
        // than a reason to create files.
        let mut messages: Vec<ChatMessage> = if self.persistence_enabled(session.app_public_id)? {
            match self.persisted_table.partition(slot) {
                Some(table) => table
                    .select_by_session_id(packed_session_id)
                    .execute()?
                    .into_iter()
                    .map(chat_message_from_persisted)
                    .collect(),
                None => Vec::new(),
            }
        } else {
            match self.memory_table.partition(slot) {
                Some(table) => table
                    .select_by_session_id(packed_session_id)
                    .execute()?
                    .into_iter()
                    .map(chat_message_from_memory)
                    .collect(),
                None => Vec::new(),
            }
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
        // One partition at a time, because a range query belongs to a
        // partition rather than to the table: each app's messages are their own
        // index now.
        //
        // This used to take the per-app lock once per row, which is the shape
        // that made purging a whole retention window a lock-acquire loop. An
        // app's partition is its own, so there is nothing to take.
        for slot in self.memory_table.keys() {
            let Some(table) = self.memory_table.partition(slot) else {
                // Removed between listing the keys and reading it. Nothing to
                // purge in a partition that is gone.
                continue;
            };

            let rows = table
                .select_by_sent_at_range(..purge_all_before_ms)
                .execute()?;

            for row in rows {
                if table.select_by_message_id(row.message_id).is_some() {
                    table.delete(row.id).await?;
                }
            }
        }

        Ok(())
    }

    pub fn spawn_purge_task(
        self: Arc<Self>,
        interval: Duration,
        retention: Duration,
    ) -> nagoya::JoinHandle<()> {
        crate::work_runtime().spawn(async move {
            loop {
                let cutoff = Utc::now().timestamp_millis() - retention.as_millis() as i64;
                if let Err(e) = self.purge_memory_before(cutoff).await {
                    warn!(error = %e, "failed to purge memory support messages");
                }
                nagoya::sleep(interval).await;
            }
        })
    }

    fn persistence_enabled(&self, app_public_id: PackedNanoId) -> eyre::Result<bool> {
        self.app_config_table
            .select_by_public_id(app_public_id)
            .map(|r| r.message_persistence_enabled)
            .ok_or_else(|| eyre::eyre!("App not found"))
    }

    /// The partition an app's messages live in, minted on first use.
    ///
    /// This replaces `app_lock`, which was the same `entry().or_insert_with()`
    /// over a `HashMap` but handed back a mutex to hold across the writes. The
    /// tables are partitioned by this slot now, so an app writing to its own
    /// partition is not contending with any other app and there is nothing to
    /// serialise.
    async fn app_slot(&self, app_public_id: PackedNanoId) -> eyre::Result<u32> {
        if let Some(slot) = self.existing_app_slot(app_public_id) {
            return Ok(slot);
        }
        let slot = self.app_slot_table.get_next_pk().into();
        match self
            .app_slot_table
            .insert(AppSlotRow {
                slot,
                app_public_id,
            })
            .await
        {
            Ok(_) => Ok(slot),
            // Another caller minted it between the select and the insert. The
            // unique index on `app_public_id` is what makes that a rejected
            // insert rather than two slots for one app, so re-read rather than
            // guarding the whole thing with a lock.
            Err(_) => self
                .app_slot_table
                .select_by_app_public_id(app_public_id)
                .map(|row| row.slot)
                .ok_or_else(|| eyre::eyre!("app slot vanished after a losing insert")),
        }
    }

    /// The app's slot if one has been minted, without minting one.
    fn existing_app_slot(&self, app_public_id: PackedNanoId) -> Option<u32> {
        self.app_slot_table
            .select_by_app_public_id(app_public_id)
            .map(|row| row.slot)
    }

    /// Move an app's messages out of its persisted partition into its memory
    /// one, then flip the flag.
    ///
    /// `select_all` rather than a lookup by app id: a partition holds exactly
    /// one app, so every row in it is this app's, and the index that used to
    /// answer this question was an index over a column that never varied.
    async fn move_persisted_to_memory(&self, app_public_id: PackedNanoId) -> eyre::Result<()> {
        let slot = self.app_slot(app_public_id).await?;
        let Some(source) = self.persisted_table.partition(slot) else {
            // Nothing was ever persisted for this app.
            return self.set_persistence_flag(app_public_id, false).await;
        };
        let target = self.memory_table.partition_or_create(slot)?;

        let rows = source.select_all().execute()?;

        for row in &rows {
            if target.select_by_message_id(row.message_id).is_none() {
                let mut memory_row: SupportMemoryMessageRow = row.clone().into();
                memory_row.id = target.get_next_pk().into();
                target.insert(memory_row).await?;
            }
        }

        if rows
            .iter()
            .any(|row| target.select_by_message_id(row.message_id).is_none())
        {
            return Err(eyre::eyre!("message persistence transition copy failed"));
        }

        self.set_persistence_flag(app_public_id, false).await?;

        for row in rows {
            if let Err(e) = source.delete(row.id).await {
                warn!(error = %e, "failed to clean persisted message after disabling persistence");
            }
        }

        Ok(())
    }

    /// The mirror of `move_persisted_to_memory`.
    async fn move_memory_to_persisted(&self, app_public_id: PackedNanoId) -> eyre::Result<()> {
        let slot = self.app_slot(app_public_id).await?;
        let Some(source) = self.memory_table.partition(slot) else {
            return self.set_persistence_flag(app_public_id, true).await;
        };
        let target = self.persisted_table.get_or_create(slot).await?;

        let rows = source.select_all().execute()?;

        for row in &rows {
            if target.select_by_message_id(row.message_id).is_none() {
                let mut persisted_row: SupportMessageRow = row.clone().into();
                persisted_row.id = target.get_next_pk().into();
                target.insert(persisted_row).await?;
            }
        }

        if rows
            .iter()
            .any(|row| target.select_by_message_id(row.message_id).is_none())
        {
            return Err(eyre::eyre!("message persistence transition copy failed"));
        }

        self.set_persistence_flag(app_public_id, true).await?;

        for row in rows {
            if let Err(e) = source.delete(row.id).await {
                warn!(error = %e, "failed to clean memory message after enabling persistence");
            }
        }

        Ok(())
    }

    async fn set_persistence_flag(
        &self,
        app_public_id: PackedNanoId,
        enabled: bool,
    ) -> eyre::Result<()> {
        self.app_config_table
            .update_by_public_id(
                app_public_id,
                AppConfigColumns::MESSAGE_PERSISTENCE_ENABLED,
                enabled,
            )
            .await?;
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
