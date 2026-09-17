use std::fmt;
use std::sync::Arc;

use nagoya::sync::RwLock;
use worktable::PersistedWorkTable;
use worktable::persistence::{PersistenceEngine, PersistenceResult};
use worktable::prelude::DiskConfig;

#[cfg(feature = "s3-sync")]
use worktable::prelude::{S3Config as WtS3Config, S3DiskConfig};

#[cfg(not(feature = "s3-sync"))]
use crate::db::schema::support_message::SupportMessagePersistenceEngine;
#[cfg(feature = "s3-sync")]
use crate::db::schema::support_message::SupportMessageS3SyncPersistenceEngine;
use crate::db::schema::support_message::{SupportMessagePartitions, SupportMessageWorkTable};

/// The on-disk name of one app's message partition.
///
/// Partitioning turns the single `support_message` directory into one per app,
/// so the slot is now half of a file path. `worktable_rebuild` has to name the
/// same directories to copy them, which is why this is a function rather than a
/// `format!` at the one site that creates them.
pub fn partition_table_name(slot: u32) -> String {
    format!("{}_p{slot}", SupportMessageWorkTable::name_snake_case())
}

/// Builds the persistence engine for one message partition.
///
/// A partitioned table is not handed a whole-table engine: each partition is a
/// table in its own right with its own files, so the configuration that used to
/// be resolved once in `Tables::new` has to survive and be replayable per slot.
/// That is all this is, the `DiskConfig`/`S3DiskConfig` construction lifted out
/// of `Tables::new` and kept.
#[cfg(not(feature = "s3-sync"))]
#[derive(Debug, Clone)]
pub struct MessagePartitionFactory {
    db_path: String,
}

#[cfg(not(feature = "s3-sync"))]
impl MessagePartitionFactory {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    async fn load(&self, slot: u32) -> eyre::Result<SupportMessageWorkTable> {
        let cfg = DiskConfig::new_with_table_name(
            self.db_path.clone(),
            partition_table_name(slot),
            SupportMessageWorkTable::version(),
        );
        let engine = SupportMessagePersistenceEngine::new(cfg).await?;
        SupportMessageWorkTable::load(engine).await
    }
}

/// Builds the persistence engine for one message partition. See the disk-only
/// twin above; the difference is the S3 mirror, not the shape.
#[cfg(feature = "s3-sync")]
#[derive(Debug, Clone)]
pub struct MessagePartitionFactory {
    db_path: String,
    s3: WtS3Config,
}

#[cfg(feature = "s3-sync")]
impl MessagePartitionFactory {
    pub fn new(db_path: String, s3: WtS3Config) -> Self {
        Self { db_path, s3 }
    }

    async fn load(&self, slot: u32) -> eyre::Result<SupportMessageWorkTable> {
        let cfg = S3DiskConfig {
            disk: DiskConfig::new_with_table_name(
                self.db_path.clone(),
                partition_table_name(slot),
                SupportMessageWorkTable::version(),
            ),
            s3: self.s3.clone(),
        };
        let engine = SupportMessageS3SyncPersistenceEngine::new(cfg).await?;
        SupportMessageWorkTable::load(engine).await
    }
}

/// The persisted message partitions, plus the only thing the generated router
/// cannot do for them: create one.
///
/// WorkTable generates `partition_or_create` for an unpersisted table and
/// deliberately does not for a persisted one, because creating a persisted
/// partition needs a `DatabaseManager` rather than a `Default`. What it offers
/// instead is `partition_or_insert_with(slot, || table)`, and that closure is
/// synchronous and infallible while opening a partition is neither: it is
/// `PersistenceEngine::new(...).await?` followed by `load(...).await?`.
///
/// So the table is built outside the closure and the closure only moves it in.
/// That leaves one hazard the router does not cover. It discards the loser's
/// table on a race, which is fine for a `Default` and wrong here: both racers
/// would already have opened the same files before either lost. Creation is
/// therefore serialised, double-checked on both sides of the lock. The lock is
/// held while a partition is opened and never across a write, which is the
/// distinction from the `app_locks` map this replaced.
pub struct PersistedMessages {
    partitions: SupportMessagePartitions,
    factory: MessagePartitionFactory,
    creating: RwLock<()>,
}

impl PersistedMessages {
    pub fn new(factory: MessagePartitionFactory) -> Self {
        Self {
            partitions: SupportMessagePartitions::new(),
            factory,
            creating: RwLock::new(()),
        }
    }

    /// The partition for `slot`, opening it if this process has not yet.
    ///
    /// Opening is idempotent against what is on disk: a slot that already has
    /// files is loaded from them, a slot that does not starts empty. That is
    /// what makes this both the boot path and the first-message path.
    pub async fn get_or_create(&self, slot: u32) -> eyre::Result<Arc<SupportMessageWorkTable>> {
        if let Some(table) = self.partitions.partition(slot) {
            return Ok(table);
        }

        let _guard = self.creating.write().await;
        // Another caller may have opened it while this one waited.
        if let Some(table) = self.partitions.partition(slot) {
            return Ok(table);
        }

        let table = self.factory.load(slot).await?;
        Ok(self.partitions.partition_or_insert_with(slot, || table)?)
    }

    /// The partition for `slot` if it is already open, without opening one.
    pub fn partition(&self, slot: u32) -> Option<Arc<SupportMessageWorkTable>> {
        self.partitions.partition(slot)
    }

    /// Every open partition with its slot.
    pub fn iter(&self) -> Vec<(u32, Arc<SupportMessageWorkTable>)> {
        self.partitions.iter()
    }

    /// Drain every partition's write queue.
    ///
    /// A partitioned table has no whole-table queue to wait on; each partition
    /// owns one. A partition skipped here is that app's writes lost on
    /// shutdown, so this walks all of them.
    pub async fn wait_for_ops(&self) -> PersistenceResult {
        for (_, table) in self.partitions.iter() {
            table.wait_for_ops().await?;
        }
        Ok(())
    }
}

impl fmt::Debug for PersistedMessages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersistedMessages")
            .field("open_partitions", &self.partitions.len())
            .finish()
    }
}
