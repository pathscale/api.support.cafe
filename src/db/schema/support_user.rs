use worktable::prelude::*;
use worktable::worktable;

use crate::id_types::PackedNanoId;

worktable!(
    name: SupportUser,
    version: 1,
    persist: true,
    columns: {
        id: u64 primary_key autoincrement,
        app_public_id: PackedNanoId,
        tg_handle: String,
        chat_id: i64 optional,
        is_active: bool,
    },
    indexes: {
        app_pub_id_idx: app_public_id,
        tg_handle_idx: tg_handle,
    },
    queries: {
        update: {
            ChatIdByHandle(chat_id) by tg_handle,
            IsActiveById(is_active) by id,
        },
    }
);
