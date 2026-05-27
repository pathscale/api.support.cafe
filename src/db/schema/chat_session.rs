use worktable::prelude::*;
use worktable::worktable;

use crate::id_types::PackedNanoId;

worktable!(
    name: ChatSession,
    version: 1,
    persist: true,
    columns: {
        id: u64 primary_key autoincrement,
        session_id: PackedNanoId,
        app_public_id: PackedNanoId,
        user_pub_id: PackedNanoId,
        created_at: i64,
        closed_at: i64 optional,
    },
    indexes: {
        app_pub_id_idx: app_public_id,
        session_id_idx: session_id unique,
    },
    queries: {
        update: {
            ClosedAtById(closed_at) by id,
        },
    }
);
