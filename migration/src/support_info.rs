use support_cafe::db::util::PackedUserPubId;
use worktable::migration::Migration;
use worktable::migration_engine;
use worktable::prelude::*;
use worktable::worktable;

worktable!(
    name: SupportInfo,
    version: 2,
    persist: true,
    columns: {
        user_pub_id: PackedUserPubId primary_key,
        tg_handle: String,
        chat_id: i64 optional,
    },
    indexes: {
        tg_handle_idx: tg_handle unique,
    },
);

mod v1 {
    use super::PackedUserPubId;
    use worktable::prelude::*;
    use worktable::worktable_version;

    worktable_version!(
        name: SupportInfo,
        version: 1,
        columns: {
            user_pub_id: PackedUserPubId primary_key,
            tg_handle: String,
        },
    );
}

#[derive(Default)]
pub struct Context;

pub struct Migrator;

impl Migration<v1::SupportInfoRow, SupportInfoRow> for Migrator {
    type Context = Context;

    fn migrate(row: v1::SupportInfoRow, _ctx: &Self::Context) -> SupportInfoRow {
        SupportInfoRow {
            user_pub_id: row.user_pub_id,
            tg_handle: row.tg_handle,
            chat_id: None,
        }
    }
}

migration_engine!(
    migration: Migrator,
    current: SupportInfoWorkTable,
    ctx: Context,
    version_tables: {
        1 => v1::SupportInfoWorkTable,
    },
);
