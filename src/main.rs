use eyre::Result;
use tg_support::{App, config};

fn main() -> Result<()> {
    #[cfg(feature = "acme")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

    let config = config::load()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime.threads)
        .enable_all()
        .build()?;

    rt.block_on(async {
        #[allow(unused_mut)]
        let mut config = std::sync::Arc::new(config);

        #[cfg(feature = "acme")]
        let _acme_guard = tg_support::acme::init_acme(&mut config).await?;

        let app = App::init((*config).clone()).await?;
        app.run().await
    })
}
