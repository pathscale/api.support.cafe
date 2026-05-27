use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{ArcToolbox, RequestContext};
use endpoint_libs::libs::ws::{SubAuthController, WsConnection};
use eyre::bail;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

use crate::codegen::model::{PlatformConnectRequest, PlatformConnectResponse, EnumUserRole};

pub struct MethodPlatformConnect {
    pub platform_api_key: SecretString,
}

#[async_trait(?Send)]
impl SubAuthController for MethodPlatformConnect {
    fn auth(
        self: Arc<Self>,
        _toolbox: &ArcToolbox,
        param: Value,
        _ctx: RequestContext,
        conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, eyre::Result<Value>> {
        async move {
            let req: PlatformConnectRequest = serde_json::from_value(param)
                .map_err(|e| eyre::eyre!("Invalid request: {e}"))?;

            if &req.platform_api_key != self.platform_api_key.expose_secret() {
                bail!("Invalid platform API key");
            }

            conn.set_roles(Arc::new(vec![EnumUserRole::Platform as u32]));

            Ok(serde_json::to_value(PlatformConnectResponse {
                role: EnumUserRole::Platform,
            })?)
        }
        .boxed_local()
    }
}
