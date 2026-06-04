use support_cafe::codegen::model::AppMemberRole;
use support_cafe::id_types::PackedNanoId;
use worktable::migration::Migration;
use worktable::migration_engine;
use worktable::prelude::*;
use worktable::worktable;

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
);

mod v1 {
    use super::{AppMemberRole, PackedNanoId};
    use worktable::prelude::*;
    use worktable::worktable_version;

    worktable_version!(
        name: AppMember,
        version: 1,
        columns: {
            id: u64 primary_key autoincrement,
            app_public_id: PackedNanoId,
            user_pub_id: PackedNanoId,
            membership_key: String,
            role: AppMemberRole,
            created_at: i64,
        },
        indexes: {
            app_public_id_idx: app_public_id,
            user_pub_id_idx: user_pub_id,
            membership_key_idx: membership_key unique,
        },
    );
}

#[derive(Default)]
pub struct Context;

pub struct Migrator;

impl Migration<v1::AppMemberRow, AppMemberRow> for Migrator {
    type Context = Context;

    fn migrate(row: v1::AppMemberRow, _ctx: &Self::Context) -> AppMemberRow {
        AppMemberRow {
            id: row.id,
            app_public_id: row.app_public_id,
            user_pub_id: row.user_pub_id,
            membership_key: row.membership_key,
            role: row.role,
            created_at: row.created_at,
            is_support_enabled: false,
        }
    }
}

migration_engine!(
    migration: Migrator,
    current: AppMemberWorkTable,
    ctx: Context,
    version_tables: {
        1 => v1::AppMemberWorkTable,
    },
);
