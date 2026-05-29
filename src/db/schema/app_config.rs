use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::codegen::model::AppInfo;
use crate::id_types::PackedNanoId;

worktable!(
    name: AppConfig,
    version: 1,
    persist: true,
    columns: {
        id: u64 primary_key autoincrement,
        public_id: PackedNanoId,
        tg_bot_token: String,
        app_name: String optional,
        active: bool,
        created_at: i64,
    },
    indexes: {
        public_id_idx: public_id unique,
    },
    queries: {
        update: {
            TgBotTokenById(tg_bot_token) by id,
            TgBotTokenByPubId(tg_bot_token) by public_id,
            AppNameById(app_name) by id,
            AppNameByPubId(app_name) by public_id,
            ActiveById(active) by id,
            ActiveByPubId(active) by public_id,
        },
        delete: {
            ByPublicId() by public_id,
        }
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(AppConfigWorkTable);

impl From<AppConfigRow> for AppInfo {
    fn from(row: AppConfigRow) -> Self {
        AppInfo {
            public_id: row.public_id.unpack().expect("valid packed nanoid"),
            app_name: row.app_name,
            active: row.active,
            created_at: row.created_at,
        }
    }
}
