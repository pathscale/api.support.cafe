use config::{ConfigError, Map, Source, Value, ValueKind};
use eyre::Result;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct DopplerSource {
    secrets: HashMap<String, String>,
}

impl DopplerSource {
    pub fn try_new() -> Result<Option<Self>, ConfigError> {
        let enabled = std::env::var("CAFE_SECRETS_ENABLED")
            .ok()
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if !enabled {
            return Ok(None);
        }

        let service_token = std::env::var("CAFE_SECRETS_DOPPLER_SERVICE_TOKEN")
            .map_err(|e| ConfigError::Message(e.to_string()))?;
        let project = std::env::var("CAFE_SECRETS_DOPPLER_PROJECT")
            .map_err(|e| ConfigError::Message(e.to_string()))?;
        let config_name = std::env::var("CAFE_SECRETS_DOPPLER_CONFIG")
            .map_err(|e| ConfigError::Message(e.to_string()))?;

        let secrets =
            Self::fetch_secrets_blocking(SecretString::new(service_token.into()), project, config_name)
                .map_err(|e| ConfigError::Message(e.to_string()))?;

        Ok(Some(Self { secrets }))
    }

    fn fetch_secrets_blocking(
        token: SecretString,
        project: String,
        config: String,
    ) -> Result<HashMap<String, String>, ConfigError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ConfigError::Message(e.to_string()))?;

        rt.block_on(async {
            let provider = DopplerProvider::new(token, project, config);
            provider.fetch_all_secrets().await
        })
        .map_err(|e: eyre::Report| ConfigError::Message(e.to_string()))
    }
}

impl Source for DopplerSource {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<Map<String, Value>, ConfigError> {
        let mut map = Map::new();
        for (key, value) in &self.secrets {
            // Doppler keys use CAFE__ format: CAFE__SERVICE__PLATFORM_API_KEY -> service.platform_api_key
            let path = key.replace("CAFE__", "").replace("__", ".").to_lowercase();
            map.insert(path, Value::new(None, ValueKind::String(value.clone())));
        }
        Ok(map)
    }
}

struct DopplerProvider {
    client: Client,
    service_token: SecretString,
    project: String,
    config: String,
}

impl DopplerProvider {
    fn new(service_token: SecretString, project: String, config: String) -> Self {
        Self {
            client: Client::new(),
            service_token,
            project,
            config,
        }
    }
}

#[derive(Deserialize)]
struct DopplerSecretValue {
    raw: String,
}

#[derive(Deserialize)]
struct DopplerAllSecretsResponse {
    secrets: HashMap<String, DopplerSecretValue>,
}

impl DopplerProvider {
    async fn fetch_all_secrets(&self) -> Result<HashMap<String, String>> {
        let url = format!(
            "https://api.doppler.com/v3/configs/config/secrets?project={}&config={}",
            self.project, self.config
        );
        let body: DopplerAllSecretsResponse = self
            .client
            .get(url)
            .bearer_auth(self.service_token.expose_secret())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(body.secrets.into_iter().map(|(k, v)| (k, v.raw)).collect())
    }
}