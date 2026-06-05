use support_cafe::id_types::PackedNanoId;
use worktable::migration::Migration;
use worktable::migration_engine;
use worktable::prelude::*;
use worktable::worktable;

worktable!(
    name: AppConfig,
    version: 2,
    persist: true,
    columns: {
        id: u64 primary_key autoincrement,
        public_id: PackedNanoId,
        tg_bot_token: String,
        app_name: String optional,
        active: bool,
        message_persistence_enabled: bool,
        created_at: i64,
    },
    indexes: {
        public_id_idx: public_id unique,
    },
);

mod v1 {
    use super::PackedNanoId;
    use worktable::prelude::*;
    use worktable::worktable_version;

    worktable_version!(
        name: AppConfig,
        version: 1,
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
    );
}

#[derive(Default)]
pub struct Context;

pub struct Migrator;

impl Migration<v1::AppConfigRow, AppConfigRow> for Migrator {
    type Context = Context;

    fn migrate(row: v1::AppConfigRow, _ctx: &Self::Context) -> AppConfigRow {
        AppConfigRow {
            id: row.id,
            public_id: row.public_id,
            tg_bot_token: row.tg_bot_token,
            app_name: row.app_name,
            active: row.active,
            message_persistence_enabled: true,
            created_at: row.created_at,
        }
    }
}

migration_engine!(
    migration: Migrator,
    current: AppConfigWorkTable,
    ctx: Context,
    version_tables: {
        1 => v1::AppConfigWorkTable,
    },
);
