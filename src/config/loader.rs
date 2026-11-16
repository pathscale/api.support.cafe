use std::path::PathBuf;

use clap::{Args, Parser};
use config::{Config as CfgBuilder, Environment, File};
use eyre::Result;

use super::Config;
use super::doppler_source::DopplerSource;

#[derive(Parser)]
struct CliArgs {
    #[command(flatten)]
    config: ConfigArgs,
}

#[derive(Args)]
struct ConfigArgs {
    #[arg(short, long, env = "CAFE_CONFIG")]
    config: Option<PathBuf>,
}

#[derive(Parser)]
struct ServiceArgs {
    #[command(flatten)]
    config: ConfigArgs,
    /// Reserve pre-credential session owners offline. Stop the service first.
    #[arg(long)]
    migrate_visitor_credentials: bool,
}

pub fn load() -> Result<Config> {
    load_config(CliArgs::parse().config)
}

pub fn load_service() -> Result<(Config, bool)> {
    let cli = ServiceArgs::parse();
    Ok((load_config(cli.config)?, cli.migrate_visitor_credentials))
}

fn load_config(args: ConfigArgs) -> Result<Config> {
    let mut builder = CfgBuilder::builder();

    if let Some(path) = args.config {
        builder = builder.add_source(File::from(path));
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
