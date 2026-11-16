use std::sync::Arc;

use async_trait::async_trait;
use honey_id_types::handlers::convenience_utils::user_management::{
    CreateUserInfo, DeleteUserInfo, UserStorage,
};
use honey_id_types::id_entities::UserPublicId;

use crate::codegen::model::{GetMyInfoResponse, UserRole};
use crate::db::schema::user::UserRow;
use crate::db::schema::visitor_credential::reservation;
use crate::db::tables::Tables;
use crate::db::util::PackedUserPubId;

#[derive(Clone)]
pub struct UserService {
    tables: Arc<Tables>,
}

impl UserService {
    pub fn new(tables: Arc<Tables>) -> Self {
        Self { tables }
    }

    pub fn get_my_info(&self, user_pub_id: UserPublicId) -> eyre::Result<GetMyInfoResponse> {
        let packed_id = user_pub_id
            .pack()
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        let user = self
            .tables
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
            self.tables
                .user_table
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

        let _allocation = self.tables.visitor_identity_allocation.lock().await;
        if let Some(mut user) = self.tables.user_table.select_by_pub_id(packed_id) {
            user.username = user_info_request.username;
            self.tables.user_table.replace(user).await?;
        } else {
            eyre::ensure!(
                self.tables
                    .visitor_credential_table
                    .select(packed_id)
                    .is_none(),
                "Public ID is reserved"
            );
            self.tables
                .user_table
                .insert(UserRow {
                    id: self.tables.user_table.get_next_pk().into(),
                    pub_id: packed_id,
                    username: user_info_request.username,
                    role: UserRole::User,
                })
                .await?;
            self.tables.user_table.wait_for_ops().await?;
        }

        Ok(())
    }

    async fn delete_user(&self, user_info: DeleteUserInfo) -> eyre::Result<()> {
        let packed_id = PackedUserPubId::pack(&user_info.user_pub_id)
            .map_err(|e| eyre::eyre!("Failed to pack user_pub_id: {:?}", e))?;

        let _allocation = self.tables.visitor_identity_allocation.lock().await;
        if self.tables.user_table.select_by_pub_id(packed_id).is_some() {
            if self
                .tables
                .visitor_credential_table
                .select(packed_id)
                .is_none()
            {
                self.tables
                    .visitor_credential_table
                    .insert(reservation(packed_id))
                    .await?;
            }
            // Existing sessions and still-connected clients can retain this ID.
            self.tables.wait_for_visitor_ops().await?;
        }
        self.tables
            .app_member_table
            .delete_by_user_pub_id(packed_id)
            .await?;
        self.tables
            .support_info_table
            .delete_by_user_pub_id(packed_id)
            .await?;
        self.tables.user_table.delete_by_pub_id(packed_id).await?;

        Ok(())
    }
}
