use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::id_types::PackedNanoId;

worktable!(
    name: SupportMessage,
    version: 2,
    persist: true,
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

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(SupportMessageWorkTable);
