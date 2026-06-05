use async_trait::async_trait;
use worktable::prelude::*;
use worktable::worktable;

use crate::db::schema::support_message::SupportMessageRow;
use crate::db::util::PurgeableTable;
use crate::id_types::PackedNanoId;

worktable!(
    name: SupportMemoryMessage,
    persist: false,
    columns: {
        id: i64 primary_key autoincrement,
        message_id: PackedNanoId,
        session_id: PackedNanoId,
        app_public_id: PackedNanoId,
        incoming: bool,
        sent_by: String,
        sent_at: i64,
        content: String,
        tg_chat_id: i64 optional,
    },
    indexes: {
        message_id_idx: message_id unique,
        session_id_idx: session_id,
        app_public_id_idx: app_public_id,
        sent_at_idx: sent_at,
    }
);

impl From<SupportMessageRow> for SupportMemoryMessageRow {
    fn from(row: SupportMessageRow) -> Self {
        Self {
            id: row.id,
            message_id: row.message_id,
            session_id: row.session_id,
            app_public_id: row.app_public_id,
            incoming: row.incoming,
            sent_by: row.sent_by,
            sent_at: row.sent_at,
            content: row.content,
            tg_chat_id: row.tg_chat_id,
        }
    }
}

impl From<SupportMemoryMessageRow> for SupportMessageRow {
    fn from(row: SupportMemoryMessageRow) -> Self {
        Self {
            id: row.id,
            message_id: row.message_id,
            session_id: row.session_id,
            app_public_id: row.app_public_id,
            incoming: row.incoming,
            sent_by: row.sent_by,
            sent_at: row.sent_at,
            content: row.content,
            tg_chat_id: row.tg_chat_id,
        }
    }
}

#[async_trait]
impl PurgeableTable for SupportMemoryMessageWorkTable {
    async fn purge(&self, purge_all_before_ms: i64) -> eyre::Result<()> {
        let rows = self
            .select_by_sent_at_range(..purge_all_before_ms)
            .execute()?;

        for row in rows {
            self.delete(row.id).await?;
        }

        Ok(())
    }
}
