use config::{ConfigError, Map, Source, Value, ValueKind};
use eyre::{Result, bail};
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

        let secrets = Self::fetch_secrets_blocking(
            SecretString::new(service_token.into()),
            project,
            config_name,
        )
        .map_err(|e| ConfigError::Message(e.to_string()))?;

        Ok(Some(Self { secrets }))
    }

    fn fetch_secrets_blocking(
        token: SecretString,
        project: String,
        config: String,
    ) -> Result<HashMap<String, String>, ConfigError> {
        let provider = DopplerProvider {
            service_token: token,
            project,
            config,
        };
        provider
            .fetch_all_secrets()
            .map_err(|e| ConfigError::Message(e.to_string()))
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
    service_token: SecretString,
    project: String,
    config: String,
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
    /// Runs before any runtime exists, so it blocks this thread on the
    /// request; the process's [`nago_http::Client`] drives the socket on its
    /// own reactor thread.
    fn fetch_all_secrets(&self) -> Result<HashMap<String, String>> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("project", &self.project)
            .append_pair("config", &self.config)
            .finish();
        let url = format!("https://api.doppler.com/v3/configs/config/secrets?{query}");
        let authorization = format!("Bearer {}", self.service_token.expose_secret());

        let client = nago_http::Client::global()?;
        let response = nagoya::block_on(client.get(
            &url,
            &[
                ("Authorization", authorization.as_str()),
                ("Accept", "application/json"),
            ],
        ))?;
        if !response.is_success() {
            bail!("Doppler answered HTTP {}", response.status);
        }
        let body: DopplerAllSecretsResponse = serde_json::from_slice(&response.body)?;
        Ok(body.secrets.into_iter().map(|(k, v)| (k, v.raw)).collect())
    }
}
