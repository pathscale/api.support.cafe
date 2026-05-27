use std::path::PathBuf;

use clap::Parser;
use config::{Config as CfgBuilder, Environment, File};
use eyre::Result;

use super::Config;

#[derive(Parser)]
struct CliArgs {
    #[arg(short, long, env = "TG_SUPPORT_CONFIG")]
    config: Option<PathBuf>,
}

pub fn load() -> Result<Config> {
    let cli = CliArgs::parse();

    let mut builder = CfgBuilder::builder();

    if let Some(path) = &cli.config {
        builder = builder.add_source(File::from(path.clone()));
    }
    builder = builder.add_source(
        Environment::with_prefix("TG_SUPPORT")
            .separator("__")
            .prefix_separator("__")
            .try_parsing(true),
    );

    let config: Config = builder.build()?.try_deserialize()?;

    Ok(config)
}
