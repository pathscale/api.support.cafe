use honey_id_types::id_entities::UserPublicId;
use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::codegen::model::{UserInfo, UserRole};
use crate::db::util::PackedUserPubId;

worktable!(
    name: User,
    persist: true,
    columns: {
        id: u64 primary_key autoincrement,
        pub_id: PackedUserPubId,
        username: String,
        role: UserRole,
    },
    indexes: {
        pub_id_idx: pub_id unique,
    },
    queries: {
        update: {
            RoleById(role) by id,
            RoleByPubId(role) by pub_id,
        },
        delete: {
            ByPubId() by pub_id,
        }
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(UserWorkTable);

impl UserRow {
    pub fn pub_id(&self) -> UserPublicId {
        UserPublicId::unpack(self.pub_id).expect("Invalid packed nanoid in database")
    }
}

impl From<UserRow> for UserInfo {
    fn from(row: UserRow) -> Self {
        UserInfo {
            id: row.id as i64,
            pub_id: row.pub_id().into(),
            username: row.username,
            role: row.role,
        }
    }
}
