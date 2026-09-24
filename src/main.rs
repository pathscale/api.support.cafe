use endpoint_libs::libs::log::{FileLoggingConfig, LoggingConfig, setup_logging};
use eyre::Result;
use support_cafe::app::App;
use support_cafe::config;
use support_cafe::service::log::LogService;

fn main() -> Result<()> {
    nago_rustls::rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

    #[allow(unused_mut)]
    let mut config = config::load()?;

    // No runtime of its own to build. The server drives its own reactor inside
    // `listen`, outbound HTTP goes through nago_http clients that run their own
    // reactor threads, and everything awaited out here is woken by those, so
    // parking this thread is enough.
    nagoya::block_on(async {
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
        // No OTel guard to hold: endpoint-libs 3 removed OTLP export, so
        // `LogSetupReturn` no longer carries one. `otel_config.enabled` is still
        // accepted and warns at startup, which is why the config below is left
        // as it is rather than deleted; exporting again is a decision about
        // where traces go, not about this binding.
        let log_service =
            std::sync::Arc::new(LogService::new(log_setup.reload_handle, config.log.level));

        #[cfg(feature = "acme")]
        let _acme_guard = support_cafe::acme::init_acme(&mut config).await?;

        let app = App::init(config, log_service).await?;
        app.run().await
    })
}
