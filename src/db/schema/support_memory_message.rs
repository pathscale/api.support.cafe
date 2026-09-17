use worktable::prelude::*;
use worktable::worktable;

use crate::db::schema::support_message::SupportMessageRow;
use crate::id_types::PackedNanoId;

worktable!(
    name: SupportMemoryMessage,
    persist: false,
    partition_by: app_slot: u32,
    partition_max_size: u64,
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
        message_id_idx: message_id unique using worktables_index,
        session_id_idx: session_id using worktables_index,
        // No `app_public_id` index. A partition holds exactly one app, so an
        // index on it would be an index over a column that never varies: every
        // lookup returns the whole partition and every insert pays to maintain
        // it. `select_all` on the partition is the same answer for no upkeep.
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
