use std::fs::{self, File, OpenOptions};
use std::path::Path;

use eyre::WrapErr;
use futures::channel::oneshot;
use worktable::prelude::*;
use worktable::worktable;

#[cfg(feature = "s3-sync")]
use worktable::s3_sync_persistence;

use crate::id_types::PackedNanoId;

worktable!(
    name: VisitorCredential,
    version: 1,
    persist: true,
    columns: {
        visitor_id: PackedNanoId primary_key using worktables_index,
        app_public_id: PackedNanoId optional,
        secret_digest: String optional,
    },
);

#[cfg(feature = "s3-sync")]
s3_sync_persistence!(VisitorCredentialWorkTable);

/// A permanently reserved ID also marks completion of the offline backfill.
pub fn migration_marker() -> PackedNanoId {
    PackedNanoId::pack(&"0000000000000000".parse().expect("valid marker ID"))
        .expect("marker ID packs")
}

pub(crate) fn reservation(visitor_id: PackedNanoId) -> VisitorCredentialRow {
    VisitorCredentialRow {
        visitor_id,
        app_public_id: None,
        secret_digest: None,
    }
}

pub(crate) async fn flush_credentials(
    credentials: &VisitorCredentialWorkTable,
    disk_path: Option<&Path>,
) -> eyre::Result<()> {
    credentials.wait_for_ops().await?;
    let Some(path) = disk_path.map(Path::to_path_buf) else {
        // The S3 worker has already committed the snapshot manifest.
        return Ok(());
    };
    // The ordinary disk worker flushes writes. Sync the table's files and
    // directory entries before acknowledging issuance or migration.
    let (send, done) = oneshot::channel();
    let _worker = std::thread::Builder::new()
        .name("visitor-credential-sync".into())
        .spawn(move || {
            let result = (|| -> std::io::Result<()> {
                for entry in fs::read_dir(&path)? {
                    let entry = entry?;
                    if entry.file_type()?.is_file() {
                        OpenOptions::new()
                            .write(true)
                            .open(entry.path())?
                            .sync_all()?;
                    }
                }
                File::open(&path)?.sync_all()?;
                if let Some(parent) = path.parent() {
                    File::open(parent)?.sync_all()?;
                }
                Ok(())
            })();
            let _ = send.send(result);
        })?;
    done.await
        .wrap_err("Visitor credential sync thread stopped")?
        .wrap_err("Could not sync visitor credential table")
}

pub async fn reserve_legacy_owners(
    credentials: &VisitorCredentialWorkTable,
    sessions: impl IntoIterator<Item = PackedNanoId>,
    disk_path: Option<&Path>,
) -> eyre::Result<()> {
    let marker = migration_marker();
    for visitor_id in sessions {
        if visitor_id != marker && credentials.select(visitor_id).is_none() {
            credentials.insert(reservation(visitor_id)).await?;
        }
    }
    flush_credentials(credentials, disk_path).await?;

    credentials.insert(reservation(marker)).await?;
    flush_credentials(credentials, disk_path).await
}
