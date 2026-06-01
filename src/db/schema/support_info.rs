use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::db::util::PackedUserPubId;

worktable!(
    name: SupportInfo,
    version: 1,
    persist: true,
    columns: {
        user_pub_id: PackedUserPubId primary_key,
        tg_handle: String,
    },
    queries: {
        update: {
            TgHandleByUserPubId(tg_handle) by user_pub_id,
        },
        delete: {
            ByUserPubId() by user_pub_id,
        }
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(SupportInfoWorkTable);
