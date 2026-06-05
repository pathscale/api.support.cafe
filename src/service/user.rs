use std::sync::Arc;

use async_trait::async_trait;
use honey_id_types::handlers::convenience_utils::user_management::{
    CreateUserInfo, DeleteUserInfo, UserStorage,
};
use honey_id_types::id_entities::UserPublicId;

use crate::codegen::model::{GetMyInfoResponse, UserRole};
use crate::db::schema::app_member::AppMemberWorkTable;
use crate::db::schema::support_info::SupportInfoWorkTable;
use crate::db::schema::user::{UserRow, UserWorkTable};
use crate::db::util::PackedUserPubId;

#[derive(Clone)]
pub struct UserService {
    user_table: Arc<UserWorkTable>,
    app_member_table: Arc<AppMemberWorkTable>,
    support_info_table: Arc<SupportInfoWorkTable>,
}

impl UserService {
    pub fn new(
        user_table: Arc<UserWorkTable>,
        app_member_table: Arc<AppMemberWorkTable>,
        support_info_table: Arc<SupportInfoWorkTable>,
    ) -> Self {
        Self {
            user_table,
            app_member_table,
            support_info_table,
        }
    }

    pub fn get_my_info(&self, user_pub_id: UserPublicId) -> eyre::Result<GetMyInfoResponse> {
        let packed_id = user_pub_id
            .pack()
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        let user = self
            .user_table
            .select_by_pub_id(packed_id)
            .ok_or_else(|| eyre::eyre!("User not found"))?;

        Ok(GetMyInfoResponse {
            pub_id: user.pub_id().into(),
            username: user.username,
            role: user.role,
        })
    }
}

#[async_trait]
impl UserStorage for UserService {
    fn get_api_roles_by_pub_id(&self, user_pub_id: UserPublicId) -> eyre::Result<Vec<u32>> {
        let packed_id = user_pub_id
            .pack()
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        Ok(vec![
            self.user_table
                .select_by_pub_id(packed_id)
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

        if let Some(mut user) = self.user_table.select_by_pub_id(packed_id) {
            user.username = user_info_request.username;
            self.user_table.update(user).await?;
        } else {
            self.user_table.insert(UserRow {
                id: self.user_table.get_next_pk().into(),
                pub_id: packed_id,
                username: user_info_request.username,
                role: UserRole::User,
            })?;
        }

        Ok(())
    }

    async fn delete_user(&self, user_info: DeleteUserInfo) -> eyre::Result<()> {
        let packed_id = PackedUserPubId::pack(&user_info.user_pub_id)
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        self.app_member_table
            .delete_by_user_pub_id(packed_id)
            .await?;
        self.support_info_table
            .delete_by_user_pub_id(packed_id)
            .await?;
        self.user_table.delete_by_pub_id(packed_id).await?;

        Ok(())
    }
}
