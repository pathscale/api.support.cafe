use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::id_types::PackedNanoId;

worktable!(
    name: SupportMessage,
    version: 1,
    persist: true,
    columns: {
        id: i64 primary_key autoincrement,
        session_id: PackedNanoId,
        app_public_id: PackedNanoId,
        incoming: bool,
        sent_by: String,
        sent_at: i64,
        content: String,
        tg_chat_id: i64 optional,
    },
    indexes: {
        session_id_idx: session_id,
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(SupportMessageWorkTable);
