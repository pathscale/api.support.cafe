use eyre::Result;
use tg_support::{App, config};

fn main() -> Result<()> {
    let config = config::load()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime.threads)
        .enable_all()
        .build()?;

    rt.block_on(async {
        let app = App::init(config).await?;
        app.run().await
    })
}
