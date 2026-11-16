use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::codegen::model::AppMemberRole;
use crate::id_types::PackedNanoId;

worktable!(
    name: TelegramLink,
    version: 1,
    persist: true,
    columns: {
        member_id: u64 primary_key,
        app_public_id: PackedNanoId,
        user_pub_id: PackedNanoId,
        member_created_at: i64,
        member_role: AppMemberRole,
        bot_token_digest: String,
        proof_id: PackedNanoId,
        handle: String,
        chat_id: i64,
    },
    indexes: {
        app_public_id_idx: app_public_id using worktables_index,
    },
    queries: {
        delete: {
            ByAppPublicId() by app_public_id,
        }
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(TelegramLinkWorkTable);
