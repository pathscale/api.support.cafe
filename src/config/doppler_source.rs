use config::{ConfigError, Map, Source, Value, ValueKind};
use eyre::{Result, bail, eyre};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::collections::HashMap;

use nagoya::reactor::{Reactor, block_on_with};

use crate::https;

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
    /// Runs before any runtime exists, so it brings its own: a local reactor
    /// driven on this thread for one request, and the lookup done first, while
    /// no reactor is running for it to stall.
    fn fetch_all_secrets(&self) -> Result<HashMap<String, String>> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("project", &self.project)
            .append_pair("config", &self.config)
            .finish();
        let path = format!("/v3/configs/config/secrets?{query}");
        let authorization = format!("Bearer {}", self.service_token.expose_secret());

        let target = https::Target::resolve("api.doppler.com")?;
        let reactor = Reactor::local().map_err(|e| eyre!("reactor setup failed: {e:?}"))?;
        let response = block_on_with(
            &reactor,
            https::send(
                &target,
                &reactor.handle(),
                https::Request {
                    method: "GET",
                    path: &path,
                    headers: &[
                        ("Authorization", &authorization),
                        ("Accept", "application/json"),
                    ],
                    body: None,
                },
            ),
        )?;
        if !response.is_success() {
            bail!("Doppler answered HTTP {}", response.status);
        }
        let body: DopplerAllSecretsResponse = serde_json::from_slice(&response.body)?;
        Ok(body.secrets.into_iter().map(|(k, v)| (k, v.raw)).collect())
    }
}
