use std::sync::Arc;

use chrono::Utc;
use psc_nanoid::Nanoid;
use psc_nanoid::alphabet::Base62Alphabet;
use worktable::prelude::SelectQueryExecutor;

mod member;

use honey_id_types::id_entities::UserPublicId;

use crate::codegen::model::AppMemberRole;
use crate::db::schema::app_config::{
    ActiveByPubIdQuery, AppConfigRow, AppConfigWorkTable, AppNameByPubIdQuery,
    TgBotTokenByPubIdQuery,
};
use crate::db::schema::app_member::AppMemberWorkTable;
use crate::db::schema::app_member::{AppMemberRow, membership_key};
use crate::db::schema::support_info::SupportInfoWorkTable;
use crate::db::schema::user::UserWorkTable;
use crate::id_types::AppPublicId;

#[derive(Debug, Clone)]
pub struct CreateAppResponse {
    pub app_public_id: AppPublicId,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct AppUpdate {
    pub tg_bot_token: Option<String>,
    pub app_name: Option<String>,
    pub active: Option<bool>,
}

pub struct AppService {
    app_config_table: Arc<AppConfigWorkTable>,
    app_member_table: Arc<AppMemberWorkTable>,
    support_info_table: Arc<SupportInfoWorkTable>,
    user_table: Arc<UserWorkTable>,
}

impl AppService {
    pub fn new(
        app_config_table: Arc<AppConfigWorkTable>,
        app_member_table: Arc<AppMemberWorkTable>,
        support_info_table: Arc<SupportInfoWorkTable>,
        user_table: Arc<UserWorkTable>,
    ) -> Self {
        Self {
            app_config_table,
            app_member_table,
            support_info_table,
            user_table,
        }
    }

    pub async fn create_app(
        &self,
        tg_bot_token: String,
        app_name: Option<String>,
        owner_pub_id: UserPublicId,
    ) -> eyre::Result<CreateAppResponse> {
        let created_at = Utc::now().timestamp_millis();
        let app_public_id_nanoid = Nanoid::<16, Base62Alphabet>::new();
        let app_public_id: AppPublicId = app_public_id_nanoid.into();
        let packed_pub_id = app_public_id.pack()?;
        let packed_owner = owner_pub_id.pack()?;

        if self.user_table.select_by_pub_id(packed_owner).is_none() {
            eyre::bail!("User not found");
        }

        tracing::debug!(
            app_public_id = %app_public_id,
            "AppService::create_app: generating new app"
        );

        let row = AppConfigRow {
            id: self.app_config_table.get_next_pk().into(),
            public_id: packed_pub_id,
            tg_bot_token,
            app_name,
            active: true,
            created_at,
        };

        self.app_config_table.insert(row).inspect_err(|e| {
            tracing::error!(
                app_public_id = %app_public_id,
                error = %e,
                "AppService::create_app: insert failed"
            );
        })?;

        let owner_row = AppMemberRow {
            id: self.app_member_table.get_next_pk().into(),
            app_public_id: packed_pub_id,
            user_pub_id: packed_owner,
            membership_key: membership_key(packed_pub_id, packed_owner),
            role: AppMemberRole::Owner,
            created_at,
            is_support_enabled: false,
        };
        self.app_member_table.insert(owner_row).inspect_err(|e| {
            tracing::error!(
                app_public_id = %app_public_id,
                user_pub_id = %owner_pub_id,
                error = %e,
                "AppService::create_app: owner insert failed"
            );
        })?;
        self.recompute_user_role_from_memberships(owner_pub_id)
            .await?;

        Ok(CreateAppResponse {
            app_public_id,
            created_at,
        })
    }

    pub async fn edit_app(
        &self,
        app_public_id: AppPublicId,
        update: AppUpdate,
    ) -> eyre::Result<()> {
        let packed_pub_id = app_public_id.pack()?;

        tracing::debug!(
            app_public_id = %app_public_id,
            "AppService::edit_app: updating app"
        );

        if let Some(token) = &update.tg_bot_token {
            self.app_config_table
                .update_tg_bot_token_by_pub_id(
                    TgBotTokenByPubIdQuery {
                        tg_bot_token: token.clone(),
                    },
                    packed_pub_id,
                )
                .await
                .inspect_err(|e| {
                    tracing::error!(
                        app_public_id = %app_public_id,
                        error = %e,
                        "AppService::edit_app: tg_bot_token update failed"
                    );
                })?;
        }

        if let Some(name) = &update.app_name {
            self.app_config_table
                .update_app_name_by_pub_id(
                    AppNameByPubIdQuery {
                        app_name: Some(name.clone()),
                    },
                    packed_pub_id,
                )
                .await
                .inspect_err(|e| {
                    tracing::error!(
                        app_public_id = %app_public_id,
                        error = %e,
                        "AppService::edit_app: app_name update failed"
                    );
                })?;
        }

        if let Some(active) = update.active {
            self.app_config_table
                .update_active_by_pub_id(ActiveByPubIdQuery { active }, packed_pub_id)
                .await
                .inspect_err(|e| {
                    tracing::error!(
                        app_public_id = %app_public_id,
                        error = %e,
                        "AppService::edit_app: active update failed"
                    );
                })?;
        }

        Ok(())
    }

    pub fn exists(&self, app_public_id: AppPublicId) -> bool {
        let Ok(packed_pub_id) = app_public_id.pack() else {
            return false;
        };
        self.app_config_table
            .select_by_public_id(packed_pub_id)
            .is_some()
    }

    pub fn list_apps(&self) -> eyre::Result<Vec<AppConfigRow>> {
        self.app_config_table
            .select_all()
            .execute()
            .map_err(|e| eyre::eyre!("AppService::list_apps: query failed: {}", e))
            .inspect_err(|e| {
                tracing::error!(error = %e, "AppService::list_apps: query failed");
            })
    }

    pub fn get_app(&self, app_public_id: AppPublicId) -> eyre::Result<Option<AppConfigRow>> {
        let packed_pub_id = app_public_id.pack()?;
        Ok(self.app_config_table.select_by_public_id(packed_pub_id))
    }

    pub async fn delete_app(&self, app_public_id: AppPublicId) -> eyre::Result<()> {
        let packed_pub_id = app_public_id.pack()?;

        tracing::debug!(
            app_public_id = %app_public_id,
            "AppService::delete_app: deleting app"
        );

        self.app_member_table
            .delete_by_app_public_id(packed_pub_id)
            .await
            .map_err(|e| eyre::eyre!("Delete app members error: {e}"))
            .inspect_err(|e| {
                tracing::error!(
                    app_public_id = %app_public_id,
                    error = %e,
                    "AppService::delete_app: app member delete failed"
                );
            })?;

        self.app_config_table
            .delete_by_public_id(packed_pub_id)
            .await
            .map_err(|e| eyre::eyre!("Delete error: {e}"))
            .inspect_err(|e| {
                tracing::error!(
                    app_public_id = %app_public_id,
                    error = %e,
                    "AppService::delete_app: app config delete failed"
                );
            })?;

        Ok(())
    }
}
