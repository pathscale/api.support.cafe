use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::id_types::PackedNanoId;

// The integer an app's messages are partitioned by.
//
// `partition_by` routes by array index, so its key must be an unsigned integer;
// WorkTable refuses a name outright and says why: "Names belong in a separate
// registry table looked up once, not in the routing key". This is that table.
// A `PackedNanoId` comes in, a `u32` slot goes out, and the slot is what every
// partitioned table is keyed on from there.
//
// It replaces `MessageStore::app_locks`, a `HashMap<PackedNanoId, Mutex<()>>`
// that served the same purpose by holding a per-app lock across the writes
// instead of giving each app its own partition to write into.
//
// Persisted, and it has to be. The slot is half of an on-disk name: a
// persisted partition lives in a directory called `support_message_p<slot>`.
// An in-memory registry would mint slots from zero on every boot, hand app B
// the slot app A had last run, and serve one app another's messages. This
// table is what makes a slot mean the same app across a restart, and it is
// loaded before any partition so the mapping is known before anything is
// routed by it.
worktable!(
    name: AppSlot,
    version: 1,
    persist: true,
    columns: {
        slot: u32 primary_key autoincrement,
        app_public_id: PackedNanoId,
    },
    indexes: {
        app_public_id_idx: app_public_id unique using worktables_index,
    }
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(AppSlotWorkTable);
