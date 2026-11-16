use std::collections::HashMap;
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::peer::{Extensions, PeerIdentity};
use endpoint_libs::libs::toolbox::CustomError;
use endpoint_libs::libs::ws::{
    BoxedStream, OnConnect, UpgradeEvent, WebsocketServer, WsServerConfig, WsUpgrader,
};
use parking_lot::Mutex;

use crate::codegen::model::EnumErrorCode;
use crate::handlers::app::auth::upgrade_visitor_handshake;
use crate::id_types::AppPublicId;

const MAX_CONNECTIONS_PER_APP: usize = 64;

pub(crate) fn install(server: &mut WebsocketServer) {
    server.set_upgrader(Arc::new(VisitorUpgrader));
    server.add_on_connect_hook(AppAdmission(Arc::new(Mutex::new(HashMap::new()))));
}

struct Slot<K: Eq + Hash> {
    counts: Arc<Mutex<HashMap<K, usize>>>,
    key: K,
}

impl<K: Copy + Eq + Hash> Slot<K> {
    fn acquire(counts: &Arc<Mutex<HashMap<K, usize>>>, key: K, limit: usize) -> Option<Self> {
        let mut counts_guard = counts.lock();
        if counts_guard.get(&key).copied().unwrap_or_default() >= limit {
            return None;
        }
        *counts_guard.entry(key).or_default() += 1;
        Some(Self {
            counts: counts.clone(),
            key,
        })
    }
}

impl<K: Eq + Hash> Drop for Slot<K> {
    fn drop(&mut self) {
        let mut counts = self.counts.lock();
        if let Some(count) = counts.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&self.key);
            }
        }
    }
}

/// Runs the visitor handshake only. Connections are not counted per transport address: behind
/// Fly every client arrives from the proxy address, so a per-peer cap would throttle all users
/// together. The per-app budget below still applies.
struct VisitorUpgrader;

#[async_trait(?Send)]
impl WsUpgrader for VisitorUpgrader {
    async fn upgrade_stream(
        &self,
        stream: BoxedStream,
        addr: SocketAddr,
        config: &WsServerConfig,
    ) -> eyre::Result<Option<UpgradeEvent>> {
        upgrade_visitor_handshake(stream, addr, config).await
    }
}

struct AppAdmission(Arc<Mutex<HashMap<AppPublicId, usize>>>);

#[async_trait(?Send)]
impl OnConnect for AppAdmission {
    async fn on_connect(
        &self,
        _peer: &PeerIdentity,
        ext: &mut Extensions,
    ) -> Result<(), CustomError> {
        ext.insert(Arc::new(AppAdmissionScope {
            counts: self.0.clone(),
            slot: Mutex::new(None),
        }));
        Ok(())
    }
}

// endpoint-libs runs OnConnect before subprotocol auth. The slot belongs to the
// connection extensions, so refusal, cancellation and normal close all release
// it when the last connection context is dropped.
pub(crate) struct AppAdmissionScope {
    counts: Arc<Mutex<HashMap<AppPublicId, usize>>>,
    slot: Mutex<Option<Slot<AppPublicId>>>,
}

impl AppAdmissionScope {
    pub(crate) fn claim(ext: &Extensions, app: AppPublicId) -> Result<(), CustomError> {
        let scope = ext.get::<Arc<Self>>().ok_or_else(|| {
            CustomError::new(EnumErrorCode::ServiceUnavailable)
                .with_message("Connection admission is unavailable")
        })?;
        let mut slot = scope.slot.lock();
        if let Some(existing) = slot.as_ref() {
            return if existing.key == app {
                Ok(())
            } else {
                Err(CustomError::new(EnumErrorCode::Forbidden)
                    .with_message("Connection already belongs to another app"))
            };
        }

        *slot = Some(
            Slot::acquire(&scope.counts, app, MAX_CONNECTIONS_PER_APP).ok_or_else(|| {
                CustomError::new(EnumErrorCode::TooManyRequests)
                    .with_message("Too many connections for this app")
            })?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id_types::NanoId;

    #[test]
    fn resource_admission_app_scope_releases_with_the_last_connection_context() {
        futures::executor::block_on(async {
            let hook = AppAdmission(Arc::new(Mutex::new(HashMap::new())));
            let app = AppPublicId::from(NanoId::new());
            let other_app = AppPublicId::from(NanoId::new());
            let mut admitted = Vec::new();
            for _ in 0..MAX_CONNECTIONS_PER_APP {
                let mut ext = Extensions::new();
                hook.on_connect(&PeerIdentity::Unknown, &mut ext)
                    .await
                    .unwrap();
                AppAdmissionScope::claim(&ext, app).unwrap();
                admitted.push(ext);
            }

            let mut next = Extensions::new();
            hook.on_connect(&PeerIdentity::Unknown, &mut next)
                .await
                .unwrap();
            let error = AppAdmissionScope::claim(&next, app).unwrap_err();
            assert_eq!(error.code.to_u32(), EnumErrorCode::TooManyRequests as u32);

            let mut independent = Extensions::new();
            hook.on_connect(&PeerIdentity::Unknown, &mut independent)
                .await
                .unwrap();
            AppAdmissionScope::claim(&independent, other_app).unwrap();

            let context = admitted.pop().unwrap().clone();
            assert!(AppAdmissionScope::claim(&next, app).is_err());
            drop(context);
            AppAdmissionScope::claim(&next, app).unwrap();
        });
    }
}
