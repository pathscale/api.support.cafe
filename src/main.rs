use app::App;
use eyre::Result;

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
    #[cfg(feature = "acme")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

    let mut config = config::load()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime.threads)
        .enable_all()
        .build()?;

    rt.block_on(async {
        #[cfg(feature = "acme")]
        let _acme_guard = acme::init_acme(&mut config).await?;

        let app = App::init(config).await?;
        app.run().await
    })
}
