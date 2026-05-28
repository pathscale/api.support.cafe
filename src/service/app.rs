use std::sync::Arc;

use chrono::Utc;
use psc_nanoid::Nanoid;
use psc_nanoid::alphabet::Base62Alphabet;
use worktable::prelude::SelectQueryExecutor;

use crate::db::schema::app_config::{
    AppConfigRow, AppConfigWorkTable, ActiveByPubIdQuery, AppNameByPubIdQuery,
    TgBotTokenByPubIdQuery,
};
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
}

impl AppService {
    pub fn new(app_config_table: Arc<AppConfigWorkTable>) -> Self {
        Self { app_config_table }
    }

    pub fn create_app(
        &self,
        tg_bot_token: String,
        app_name: Option<String>,
    ) -> eyre::Result<CreateAppResponse> {
        let created_at = Utc::now().timestamp_millis();
        let app_public_id_nanoid = Nanoid::<16, Base62Alphabet>::new();
        let app_public_id: AppPublicId = app_public_id_nanoid.into();
        let packed_pub_id = app_public_id.pack()?;

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

        Ok(CreateAppResponse { app_public_id, created_at })
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
                    TgBotTokenByPubIdQuery { tg_bot_token: token.clone() },
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
                    AppNameByPubIdQuery { app_name: Some(name.clone()) },
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
        self.app_config_table.select_by_public_id(packed_pub_id).is_some()
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
}