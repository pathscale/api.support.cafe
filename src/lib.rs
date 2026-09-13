use std::sync::OnceLock;

pub mod app;
pub mod codegen;
pub mod config;
pub mod db;
pub mod handlers;
pub mod id_types;
pub mod service;

/// Application-owned maintenance and routing work runs away from the Tokio
/// threads that drive sockets, signals, and the endpoint server.
pub(crate) fn work_runtime() -> &'static nagoya::runtime::Runtime {
    static RUNTIME: OnceLock<nagoya::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        let workers = std::thread::available_parallelism()
            .map_or(2, core::num::NonZeroUsize::get)
            .min(usize::BITS as usize);
        nagoya::runtime::Runtime::with_tuning(
            workers,
            nagoya::Tuning::default(),
            "support-cafe-work",
        )
    })
}

#[cfg(feature = "acme")]
pub mod acme;
