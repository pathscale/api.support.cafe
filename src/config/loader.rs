use std::path::PathBuf;

use clap::Parser;
use config::{Config as CfgBuilder, Environment, File};
use eyre::Result;

use super::doppler_source::DopplerSource;
use super::Config;

#[derive(Parser)]
struct CliArgs {
    #[arg(short, long, env = "CAFE_CONFIG")]
    config: Option<PathBuf>,
}

pub fn load() -> Result<Config> {
    let cli = CliArgs::parse();

    let mut builder = CfgBuilder::builder();

    if let Some(path) = &cli.config {
        builder = builder.add_source(File::from(path.clone()));
    }

    if let Some(doppler) = DopplerSource::try_new()? {
        builder = builder.add_source(doppler);
    }

    builder = builder.add_source(
        Environment::with_prefix("CAFE")
            .separator("__")
            .prefix_separator("__")
            .try_parsing(true),
    );

    let config: Config = builder.build()?.try_deserialize()?;

    Ok(config)
}
