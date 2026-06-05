use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::id_types::PackedNanoId;

worktable!(
    name: SupportMessage,
    version: 2,
    persist: true,
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

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(SupportMessageWorkTable);
