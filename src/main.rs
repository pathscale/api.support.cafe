use app::App;
use endpoint_libs::libs::log::{FileLoggingConfig, LoggingConfig, setup_logging};
use eyre::Result;
use service::log::LogService;

mod app;
pub mod codegen;
pub mod config;
pub mod db;
pub mod handlers;
pub mod id_types;
pub mod service;

#[cfg(feature = "acme")]
pub mod acme;

fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

    #[allow(unused_mut)]
    let mut config = config::load()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime.threads)
        .enable_all()
        .build()?;

    rt.block_on(async {
        let log_setup = setup_logging(LoggingConfig {
            level: config.log.level,
            file_config: Some(FileLoggingConfig {
                path: config.log.folder.clone(),
                file_prefix: None,
                rotation: None,
            }),
            otel_config: config.log.otel.clone().into_otel_config(),
        })?;

        let _log_guards = log_setup.log_guards;
        let _otel_guards = log_setup.otel_guards;
        let log_service =
            std::sync::Arc::new(LogService::new(log_setup.reload_handle, config.log.level));

        #[cfg(feature = "acme")]
        let _acme_guard = acme::init_acme(&mut config).await?;

        let app = App::init(config, log_service).await?;
        app.run().await
    })
}
