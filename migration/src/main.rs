use std::path::Path;

use eyre::{Result, WrapErr};
use tempfile::TempDir;
use tracing::{info, warn};
use worktable::persistence::PersistenceEngine;
use worktable::prelude::{DiskConfig, S3DiskConfig};

use support_cafe::db::schema::app_config::{AppConfigS3SyncPersistenceEngine, AppConfigWorkTable};
use support_cafe::db::schema::app_member::{AppMemberS3SyncPersistenceEngine, AppMemberWorkTable};
use support_cafe::db::schema::support_info::{
    SupportInfoS3SyncPersistenceEngine, SupportInfoWorkTable,
};
use support_cafe::db::schema::support_message::{
    SupportMessageS3SyncPersistenceEngine, SupportMessageWorkTable,
};

mod app_config;
mod app_member;
mod config;
mod support_info;
mod support_message;

use config::MigrateConfig;

async fn run(config: MigrateConfig) -> Result<()> {
    let target_dir = config.migration.output_path.clone();

    std::fs::create_dir_all(&target_dir)?;
    info!(target = %target_dir.display(), "Using target directory");

    let staging = TempDir::new_in(&target_dir)
        .wrap_err("Failed to create staging directory for S3 download")?;
    let source_dir = staging.path().to_path_buf();

    let wt_s3 = worktable::prelude::S3Config {
        bucket_name: config.s3.bucket_name.clone(),
        endpoint: config.s3.endpoint.clone(),
        access_key: config.s3.access_key.clone().unwrap(),
        secret_key: config.s3.secret_key.clone().unwrap(),
        region: None,
        prefix: Some(config.s3.prefix.clone()),
    };

    let db_path = source_dir.to_string_lossy().to_string();

    info!("Downloading AppConfig from S3...");
    {
        let cfg = S3DiskConfig {
            disk: DiskConfig::new_with_table_name(
                db_path.clone(),
                AppConfigWorkTable::name_snake_case(),
                AppConfigWorkTable::version(),
            ),
            s3: wt_s3.clone(),
        };
        AppConfigS3SyncPersistenceEngine::new(cfg)
            .await
            .wrap_err("Failed to download AppConfig from S3")?;
    }

    info!("Downloading AppMember from S3...");
    {
        let cfg = S3DiskConfig {
            disk: DiskConfig::new_with_table_name(
                db_path.clone(),
                AppMemberWorkTable::name_snake_case(),
                AppMemberWorkTable::version(),
            ),
            s3: wt_s3.clone(),
        };
        AppMemberS3SyncPersistenceEngine::new(cfg)
            .await
            .wrap_err("Failed to download AppMember from S3")?;
    }

    info!("Downloading SupportInfo from S3...");
    {
        let cfg = S3DiskConfig {
            disk: DiskConfig::new_with_table_name(
                db_path.clone(),
                SupportInfoWorkTable::name_snake_case(),
                SupportInfoWorkTable::version(),
            ),
            s3: wt_s3.clone(),
        };
        SupportInfoS3SyncPersistenceEngine::new(cfg)
            .await
            .wrap_err("Failed to download SupportInfo from S3")?;
    }

    info!("Downloading SupportMessage from S3...");
    {
        let cfg = S3DiskConfig {
            disk: DiskConfig::new_with_table_name(
                db_path,
                SupportMessageWorkTable::name_snake_case(),
                SupportMessageWorkTable::version(),
            ),
            s3: wt_s3,
        };
        SupportMessageS3SyncPersistenceEngine::new(cfg)
            .await
            .wrap_err("Failed to download SupportMessage from S3")?;
    }

    info!(source = %source_dir.display(), "Downloaded data from S3");

    migrate_app_config(&source_dir, &target_dir).await?;
    migrate_app_member(&source_dir, &target_dir).await?;
    migrate_support_info(&source_dir, &target_dir).await?;
    migrate_support_message(&source_dir, &target_dir).await?;

    info!("Migration complete");
    Ok(())
}

async fn migrate_app_config(source_dir: &Path, target_dir: &Path) -> Result<()> {
    let table_name = AppConfigWorkTable::name_snake_case();
    let table_dir = source_dir.join(table_name);

    if !table_dir.exists() {
        warn!("No AppConfig data found, skipping");
        return Ok(());
    }

    info!("Migrating AppConfig table...");
    match app_config::MigratorEngine::migrate(
        &source_dir.to_string_lossy(),
        &target_dir.to_string_lossy(),
        &app_config::Context,
    )
    .await
    {
        Ok(report) => info!(source_version = report.source_version, "AppConfig migrated"),
        Err(e) if e.to_string().contains("Unsupported version: 2") => {
            info!("AppConfig already at v2, skipping");
            copy_dir_recursive(&table_dir, &target_dir.join(table_name))?;
        }
        Err(e) => return Err(e).wrap_err("AppConfig migration failed"),
    }

    Ok(())
}

async fn migrate_app_member(source_dir: &Path, target_dir: &Path) -> Result<()> {
    let table_name = AppMemberWorkTable::name_snake_case();
    let table_dir = source_dir.join(table_name);

    if !table_dir.exists() {
        warn!("No AppMember data found, skipping");
        return Ok(());
    }

    info!("Migrating AppMember table...");
    match app_member::MigratorEngine::migrate(
        &source_dir.to_string_lossy(),
        &target_dir.to_string_lossy(),
        &app_member::Context,
    )
    .await
    {
        Ok(report) => info!(source_version = report.source_version, "AppMember migrated"),
        Err(e) if e.to_string().contains("Unsupported version: 2") => {
            info!("AppMember already at v2, skipping");
            copy_dir_recursive(&table_dir, &target_dir.join(table_name))?;
        }
        Err(e) => return Err(e).wrap_err("AppMember migration failed"),
    }

    Ok(())
}

async fn migrate_support_message(source_dir: &Path, target_dir: &Path) -> Result<()> {
    let table_name = SupportMessageWorkTable::name_snake_case();
    let table_dir = source_dir.join(table_name);

    if !table_dir.exists() {
        warn!("No SupportMessage data found, skipping");
        return Ok(());
    }

    info!("Migrating SupportMessage table...");
    match support_message::MigratorEngine::migrate(
        &source_dir.to_string_lossy(),
        &target_dir.to_string_lossy(),
        &support_message::Context,
    )
    .await
    {
        Ok(report) => info!(
            source_version = report.source_version,
            "SupportMessage migrated"
        ),
        Err(e) if e.to_string().contains("Unsupported version: 2") => {
            info!("SupportMessage already at v2, skipping");
            copy_dir_recursive(&table_dir, &target_dir.join(table_name))?;
        }
        Err(e) => return Err(e).wrap_err("SupportMessage migration failed"),
    }

    Ok(())
}

async fn migrate_support_info(source_dir: &Path, target_dir: &Path) -> Result<()> {
    let table_name = SupportInfoWorkTable::name_snake_case();
    let table_dir = source_dir.join(table_name);

    if !table_dir.exists() {
        warn!("No SupportInfo data found, skipping");
        return Ok(());
    }

    info!("Migrating SupportInfo table...");
    match support_info::MigratorEngine::migrate(
        &source_dir.to_string_lossy(),
        &target_dir.to_string_lossy(),
        &support_info::Context,
    )
    .await
    {
        Ok(report) => info!(
            source_version = report.source_version,
            "SupportInfo migrated"
        ),
        Err(e) if e.to_string().contains("Unsupported version: 2") => {
            info!("SupportInfo already at v2, skipping");
            copy_dir_recursive(&table_dir, &target_dir.join(table_name))?;
        }
        Err(e) => return Err(e).wrap_err("SupportInfo migration failed"),
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

    let cfg = config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    nagoya::block_on(run(cfg))
}
