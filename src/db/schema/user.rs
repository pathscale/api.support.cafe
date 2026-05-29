use async_trait::async_trait;
use honey_id_types::handlers::convenience_utils::user_management::{
    CreateUserInfo, DeleteUserInfo, UserStorage,
};
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

#[async_trait]
impl UserStorage for UserWorkTable {
    fn get_api_roles_by_pub_id(&self, user_pub_id: UserPublicId) -> eyre::Result<Vec<u32>> {
        let packed_id = user_pub_id
            .pack()
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;
        Ok(vec![
            self.select_by_pub_id(packed_id)
                .map(|u| u.role as u32)
                .unwrap_or(UserRole::Public as u32),
        ])
    }

    fn get_public_roles(&self) -> &[u32] {
        &[UserRole::Public as u32]
    }

    fn get_honey_auth_role(&self) -> u32 {
        UserRole::HoneyAuth as u32
    }

    async fn create_or_update_user(&self, user_info_request: CreateUserInfo) -> eyre::Result<()> {
        let packed_id = PackedUserPubId::pack(&user_info_request.user_pub_id)
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        if let Some(mut user) = self.select_by_pub_id(packed_id) {
            user.username = user_info_request.username;
            self.update(user).await?;
            Ok(())
        } else {
            let packed_pub_id = PackedUserPubId::pack(&user_info_request.user_pub_id)
                .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

            self.insert(UserRow {
                id: self.get_next_pk().into(),
                pub_id: packed_pub_id,
                username: user_info_request.username.clone(),
                role: UserRole::User,
            })?;
            Ok(())
        }
    }

    async fn delete_user(&self, user_info: DeleteUserInfo) -> eyre::Result<()> {
        let packed_id = PackedUserPubId::pack(&user_info.user_pub_id)
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        self.delete_by_pub_id(packed_id).await?;
        Ok(())
    }
}
