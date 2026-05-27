#[cfg(feature = "acme")]
use cert_provider::provider::dns01::{BunnyDns, DnsAcmeProvider};
#[cfg(feature = "cert-s3-sync")]
use cert_provider::s3_sync::{S3CertSync, S3Config as CertS3ConfigInternal};
#[cfg(feature = "acme")]
use cert_provider::{BackgroundGuard, CertProvider};
use eyre::{Result, eyre};
#[cfg(feature = "acme")]
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[cfg(feature = "cert-s3-sync")]
use crate::config::AcmeConfig;
#[cfg(feature = "acme")]
use crate::config::Config;

#[cfg(feature = "acme")]
pub async fn init_acme(config: &mut Config) -> Result<Option<BackgroundGuard>> {
    let acme = &config.acme;
    if !acme.is_enabled() {
        info!("ACME cert provisioning not configured — using external cert files");
        return Ok(None);
    }

    let domains = acme.domains_vec();
    if domains.is_empty() {
        warn!("ACME enabled but no domains configured — using external cert files");
        return Ok(None);
    }

    let api_key = acme
        .bunny_api_key
        .clone()
        .ok_or_else(|| eyre!("API__ACME__BUNNY_API_KEY required for ACME DNS-01"))?;
    let email = acme.email.clone();

    let cert_dir = config
        .server
        .cert
        .as_ref()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("/certs"));

    let dns = BunnyDns::new(api_key);
    let mut acme_provider = DnsAcmeProvider::new(email, dns)
        .propagation_secs(180)
        .max_retries(3);

    if acme.production {
        acme_provider = acme_provider.production();
    }

    #[allow(unused_mut)]
    let mut provider: Box<dyn CertProvider> = {
        #[cfg(feature = "cert-s3-sync")]
        {
            init_acme_with_s3(acme_provider, acme).await?
        }
        #[cfg(not(feature = "cert-s3-sync"))]
        {
            Box::new(acme_provider)
        }
    };

    let guard = provider
        .init(cert_dir.clone(), Some(domains))
        .await
        .map_err(|e| eyre!("ACME cert provisioning failed: {e}"))?;

    config.server.cert = Some(cert_dir.join("fullchain.pem"));
    config.server.key = Some(cert_dir.join("privkey.pem"));

    info!(?cert_dir, "ACME certificate ready");
    Ok(Some(guard))
}

#[cfg(feature = "cert-s3-sync")]
async fn init_acme_with_s3(
    acme_provider: DnsAcmeProvider<BunnyDns>,
    acme_config: &AcmeConfig,
) -> Result<Box<dyn CertProvider>> {
    let s3 = &acme_config.cert_s3;
    if !s3.is_configured() {
        warn!("cert-s3-sync enabled but S3 not configured — ACME without S3 sync");
        return Ok(Box::new(acme_provider));
    }

    let primary_domain = acme_config
        .domains
        .split(',')
        .next()
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
        .unwrap_or("default");

    let cert_prefix = format!("{}/{}", s3.prefix.trim_end_matches('/'), primary_domain);

    let s3_config = CertS3ConfigInternal {
        bucket_name: s3.bucket_name.clone(),
        endpoint: s3.endpoint.clone(),
        access_key: s3.access_key.clone().unwrap(),
        secret_key: s3.secret_key.clone().unwrap(),
        region: s3.region.clone(),
        prefix: Some(cert_prefix),
    };

    let sync =
        S3CertSync::new(s3_config).map_err(|e| eyre!("Failed to create S3 cert sync: {e}"))?;
    let sync_arc = Arc::new(sync);
    let acme_provider = acme_provider.with_s3_sync(sync_arc.clone());

    Ok(Box::new(cert_provider::S3CertProvider::from_arc(
        acme_provider,
        sync_arc,
    )))
}
