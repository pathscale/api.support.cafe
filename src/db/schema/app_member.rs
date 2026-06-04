use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::codegen::model::{AppMember, AppMemberRole};
use crate::id_types::PackedNanoId;

worktable!(
    name: AppMember,
    version: 2,
    persist: true,
    columns: {
        id: u64 primary_key autoincrement,
        app_public_id: PackedNanoId,
        user_pub_id: PackedNanoId,
        membership_key: String,
        role: AppMemberRole,
        created_at: i64,
        is_support_enabled: bool,
    },
    indexes: {
        app_public_id_idx: app_public_id,
        user_pub_id_idx: user_pub_id,
        membership_key_idx: membership_key unique,
    },
    queries: {
        update: {
            RoleByMembershipKey(role) by membership_key,
            IsSupportEnabledByMembershipKey(is_support_enabled) by membership_key,
        },
        delete: {
            ByMembershipKey() by membership_key,
            ByAppPublicId() by app_public_id,
        }
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(AppMemberWorkTable);

impl From<AppMemberRow> for AppMember {
    fn from(row: AppMemberRow) -> Self {
        Self {
            app_public_id: row.app_public_id.unpack().expect("valid packed nanoid"),
            user_pub_id: row.user_pub_id.unpack().expect("valid packed nanoid"),
            role: row.role,
            created_at: row.created_at,
            is_support_enabled: row.is_support_enabled,
            tg_handle: None,
        }
    }
}

pub fn membership_key(app_public_id: PackedNanoId, user_pub_id: PackedNanoId) -> String {
    format!("{app_public_id:?}:{user_pub_id:?}")
}
