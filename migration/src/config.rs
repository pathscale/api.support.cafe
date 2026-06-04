use std::path::PathBuf;

use clap::Parser;
use eyre::{Result, WrapErr};

#[derive(Clone, Debug, serde::Deserialize, smart_default::SmartDefault)]
#[serde(default)]
pub struct MigrateConfig {
    pub s3: support_cafe::config::S3Config,
    pub migration: MigrationSettings,
}

#[derive(Clone, Debug, serde::Deserialize, smart_default::SmartDefault)]
#[serde(deny_unknown_fields, default)]
pub struct MigrationSettings {
    #[default(PathBuf::from("/tmp/support_cafe_migration"))]
    pub output_path: PathBuf,
}

#[derive(Parser)]
struct CliArgs {
    #[arg(short, long, env = "SUPPORT_CAFE_MIGRATE_CONFIG")]
    config: Option<PathBuf>,
}

pub fn load() -> Result<MigrateConfig> {
    use config::{Config as CfgBuilder, Environment, File};

    let cli = CliArgs::parse();
    let mut builder = CfgBuilder::builder();

    if let Some(p) = &cli.config {
        builder = builder.add_source(File::from(p.clone()));
    }

    builder = builder.add_source(
        Environment::with_prefix("SUPPORT_CAFE_MIGRATE")
            .separator("__")
            .prefix_separator("__")
            .try_parsing(true),
    );

    let cfg = builder.build().wrap_err("Failed to build config")?;
    cfg.try_deserialize()
        .wrap_err("Failed to deserialize config")
}
