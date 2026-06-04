use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::db::util::PackedUserPubId;

worktable!(
    name: SupportInfo,
    version: 2,
    persist: true,
    columns: {
        user_pub_id: PackedUserPubId primary_key,
        tg_handle: String,
        chat_id: i64 optional,
    },
    indexes: {
        tg_handle_idx: tg_handle unique,
    },
    queries: {
        update: {
            TgHandleByUserPubId(tg_handle) by user_pub_id,
            ChatIdByUserPubId(chat_id) by user_pub_id,
            ChatIdByTgHandle(chat_id) by tg_handle,
        },
        delete: {
            ByUserPubId() by user_pub_id,
        }
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(SupportInfoWorkTable);
