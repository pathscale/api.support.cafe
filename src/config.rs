mod doppler_source;
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
    #[cfg(feature = "s3-sync")]
    pub s3: S3Config,
    #[cfg(feature = "acme")]
    pub acme: AcmeConfig,
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

#[cfg(feature = "s3-sync")]
#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct S3Config {
    #[default("tg-support-db".to_string())]
    pub bucket_name: String,
    #[default("https://t3.storage.dev".to_string())]
    pub endpoint: String,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    #[default("db".to_string())]
    pub prefix: String,
    #[default = 300]
    pub sync_frequency_secs: u64,
}

#[cfg(feature = "s3-sync")]
impl S3Config {
    pub fn is_configured(&self) -> bool {
        self.access_key.is_some() && self.secret_key.is_some()
    }
}

/// ACME certificate provisioning configuration.
#[cfg(feature = "acme")]
#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AcmeConfig {
    #[default("certs@pathscale.com".to_string())]
    pub email: String,
    #[default("api.support.cafe".to_string())]
    pub domains: String,
    #[default(true)]
    pub production: bool,
    pub bunny_api_key: Option<String>,
    #[cfg(feature = "cert-s3-sync")]
    pub cert_s3: CertS3Config,
}

#[cfg(feature = "acme")]
impl AcmeConfig {
    pub fn is_enabled(&self) -> bool {
        self.bunny_api_key.is_some()
    }

    pub fn domains_vec(&self) -> Vec<String> {
        self.domains
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// S3 cert sync configuration for ACME.
#[cfg(feature = "cert-s3-sync")]
#[derive(Clone, Debug, SmartDefault, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CertS3Config {
    #[default("tg-support-db".to_string())]
    pub bucket_name: String,
    #[default("https://t3.storage.dev".to_string())]
    pub endpoint: String,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    #[default("certs".to_string())]
    pub prefix: String,
    pub region: Option<String>,
}

#[cfg(feature = "cert-s3-sync")]
impl CertS3Config {
    pub fn is_configured(&self) -> bool {
        self.access_key.is_some() && self.secret_key.is_some()
    }
}
