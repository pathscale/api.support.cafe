use chrono::Utc;
use eyre::bail;
use honey_id_types::id_entities::UserPublicId;
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{AppMember, AppMemberRole, UserRole};
use crate::db::schema::app_member::{
    AppMemberRow, IsSupportEnabledByMembershipKeyQuery, RoleByMembershipKeyQuery, membership_key,
};
use crate::db::schema::user::RoleByPubIdQuery;
use crate::id_types::AppPublicId;
use crate::service::app::AppService;

impl AppService {
    pub async fn add_member(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
        role: AppMemberRole,
    ) -> eyre::Result<AppMemberRow> {
        self.ensure_app_exists(app_public_id)?;
        self.ensure_user_exists(user_pub_id)?;

        let packed_app = app_public_id.pack()?;
        let packed_user = user_pub_id.pack()?;
        let key = membership_key(packed_app, packed_user);

        if self
            .app_member_table
            .select_by_membership_key(key.clone())
            .is_some()
        {
            bail!("User is already a member of this app");
        }

        let row = AppMemberRow {
            id: self.app_member_table.get_next_pk().into(),
            app_public_id: packed_app,
            user_pub_id: packed_user,
            membership_key: key,
            role,
            created_at: Utc::now().timestamp_millis(),
            is_support_enabled: false,
        };
        self.app_member_table.insert(row.clone())?;
        self.recompute_user_role_from_memberships(user_pub_id)
            .await?;
        Ok(row)
    }

    pub async fn remove_member(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
        expected_role: AppMemberRole,
    ) -> eyre::Result<()> {
        let member = self
            .get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        if member.role != expected_role {
            bail!("App member role mismatch");
        }
        if member.role == AppMemberRole::Owner {
            bail!("Owner cannot be removed through this endpoint");
        }

        self.app_member_table
            .delete_by_membership_key(member.membership_key)
            .await?;
        Ok(())
    }

    pub async fn set_member_role(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
        role: AppMemberRole,
    ) -> eyre::Result<()> {
        if role == AppMemberRole::Owner {
            bail!("Owner role cannot be assigned through this endpoint");
        }

        let member = self
            .get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        if member.role == AppMemberRole::Owner {
            bail!("Owner role cannot be changed through this endpoint");
        }

        let packed_app = app_public_id.pack()?;
        let packed_user = user_pub_id.pack()?;
        let key = membership_key(packed_app, packed_user);

        self.app_member_table
            .update_role_by_membership_key(RoleByMembershipKeyQuery { role }, key)
            .await?;
        self.recompute_user_role_from_memberships(user_pub_id)
            .await?;
        Ok(())
    }

    pub fn list_members(&self, app_public_id: AppPublicId) -> eyre::Result<Vec<AppMemberRow>> {
        let packed_app = app_public_id.pack()?;
        Ok(self
            .app_member_table
            .select_by_app_public_id(packed_app)
            .execute()?)
    }

    pub fn list_members_with_support_info(
        &self,
        app_public_id: AppPublicId,
    ) -> eyre::Result<Vec<AppMember>> {
        Ok(self
            .list_members(app_public_id)?
            .into_iter()
            .map(|row| {
                let support_info = self.support_info_table.select(row.user_pub_id);
                AppMember {
                    app_public_id: row.app_public_id.unpack().expect("valid packed nanoid"),
                    user_pub_id: row.user_pub_id.unpack().expect("valid packed nanoid"),
                    role: row.role,
                    created_at: row.created_at,
                    is_support_enabled: row.is_support_enabled,
                    tg_handle: support_info.map(|info| info.tg_handle),
                }
            })
            .collect())
    }

    pub async fn enable_support_user(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        let member = self
            .get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        let packed_user = user_pub_id.pack()?;
        let support_info = self
            .support_info_table
            .select(packed_user)
            .ok_or_else(|| eyre::eyre!("Support user Telegram handle is not set"))?;

        if support_info.tg_handle.trim().is_empty() {
            bail!("Support user Telegram handle is not set");
        }

        self.app_member_table
            .update_is_support_enabled_by_membership_key(
                IsSupportEnabledByMembershipKeyQuery {
                    is_support_enabled: true,
                },
                member.membership_key,
            )
            .await?;

        Ok(())
    }

    pub async fn disable_support_user(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        let member = self
            .get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        self.app_member_table
            .update_is_support_enabled_by_membership_key(
                IsSupportEnabledByMembershipKeyQuery {
                    is_support_enabled: false,
                },
                member.membership_key,
            )
            .await?;

        Ok(())
    }

    pub fn list_apps_for_user(
        &self,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<Vec<crate::db::schema::app_config::AppConfigRow>> {
        let packed_user = user_pub_id.pack()?;
        let rows = self
            .app_member_table
            .select_by_user_pub_id(packed_user)
            .execute()?;

        Ok(rows
            .into_iter()
            .filter_map(|r| self.app_config_table.select_by_public_id(r.app_public_id))
            .collect())
    }

    pub fn get_member(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<Option<AppMemberRow>> {
        let packed_app = app_public_id.pack()?;
        let packed_user = user_pub_id.pack()?;
        let key = membership_key(packed_app, packed_user);
        Ok(self.app_member_table.select_by_membership_key(key))
    }

    pub fn ensure_app_owner(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        let member = self
            .get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        if member.role == AppMemberRole::Owner {
            Ok(())
        } else {
            bail!("User is not the app owner")
        }
    }

    pub fn ensure_app_admin_or_owner(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        if self.is_platform_admin(user_pub_id)? {
            return Ok(());
        }

        let member = self
            .get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        if matches!(member.role, AppMemberRole::Owner | AppMemberRole::Admin) {
            Ok(())
        } else {
            bail!("User is not allowed to perform this app action")
        }
    }

    pub fn ensure_app_member(
        &self,
        app_public_id: AppPublicId,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        if self.is_platform_admin(user_pub_id)? {
            return Ok(());
        }

        self.get_member(app_public_id, user_pub_id)?
            .ok_or_else(|| eyre::eyre!("User is not a member of this app"))?;

        Ok(())
    }

    pub fn is_platform_admin(&self, user_pub_id: UserPublicId) -> eyre::Result<bool> {
        let packed_user = user_pub_id.pack()?;
        Ok(self
            .user_table
            .select_by_pub_id(packed_user)
            .map(|u| u.role == UserRole::Admin)
            .unwrap_or(false))
    }

    fn ensure_app_exists(&self, app_public_id: AppPublicId) -> eyre::Result<()> {
        let packed_app = app_public_id.pack()?;
        if self
            .app_config_table
            .select_by_public_id(packed_app)
            .is_none()
        {
            bail!("App not found");
        }
        Ok(())
    }

    fn ensure_user_exists(&self, user_pub_id: UserPublicId) -> eyre::Result<()> {
        let packed_user = user_pub_id.pack()?;
        if self.user_table.select_by_pub_id(packed_user).is_none() {
            bail!("User not found");
        }
        Ok(())
    }

    pub(super) async fn recompute_user_role_from_memberships(
        &self,
        user_pub_id: UserPublicId,
    ) -> eyre::Result<()> {
        let packed_user = user_pub_id.pack()?;
        let Some(user) = self.user_table.select_by_pub_id(packed_user) else {
            bail!("User not found");
        };

        if user.role == UserRole::Admin {
            return Ok(());
        }

        let rows = self
            .app_member_table
            .select_by_user_pub_id(packed_user)
            .execute()?;

        let target_role = if rows
            .iter()
            .any(|r| matches!(r.role, AppMemberRole::Owner | AppMemberRole::Admin))
        {
            UserRole::AppAdmin
        } else if rows.iter().any(|r| r.role == AppMemberRole::Support) {
            UserRole::Support
        } else {
            UserRole::User
        };

        if user.role != target_role {
            self.user_table
                .update_role_by_pub_id(RoleByPubIdQuery { role: target_role }, packed_user)
                .await?;
        }

        Ok(())
    }
}
