mod loader;

use std::path::PathBuf;

use endpoint_libs::libs::ws::WsServerConfig;
use honey_id_types::HoneyIdConfig;
use serde::Deserialize;
use smart_default::SmartDefault;

pub use loader::load;

#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(default)]
pub struct Config {
    pub runtime: RuntimeConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub service: ServiceConfig,
    pub honey_id: HoneyIdConfig,
}

#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuntimeConfig {
    #[default = 4]
    pub threads: usize,
    #[default = 1.0]
    pub tasks_ratio: f64,
}

impl RuntimeConfig {
    pub fn working_threads(&self) -> usize {
        (self.threads as f64 * self.tasks_ratio).floor() as usize
    }
}

#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
    #[default("tg_support".to_string())]
    pub name: String,
    #[default("0.0.0.0:443".to_string())]
    pub address: String,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
    #[default = false]
    pub insecure: bool,
    pub pub_certs: Option<Vec<PathBuf>>,
    pub priv_key: Option<PathBuf>,
}

impl From<ServerConfig> for WsServerConfig {
    fn from(c: ServerConfig) -> Self {
        WsServerConfig {
            name: c.name,
            address: c.address,
            pub_certs: c.cert.map(|p| vec![p]).or(c.pub_certs),
            priv_key: c.key.or(c.priv_key),
            insecure: c.insecure,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DatabaseConfig {
    #[default(PathBuf::from("/var/lib/tg_support/data"))]
    pub path: PathBuf,
}

#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServiceConfig {
    #[default("default-platform-key".to_string())]
    pub platform_api_key: String,
}
