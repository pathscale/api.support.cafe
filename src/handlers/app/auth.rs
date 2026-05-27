use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{ArcToolbox, RequestContext};
use endpoint_libs::libs::ws::{SubAuthController, WsConnection};
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use serde_json::Value;

use crate::codegen::model::{AppConnectRequest, AppConnectResponse, UserRole};
use crate::id_types::AppPublicId;
use crate::service::app_connection_registry::AppConnectionRegistry;

pub struct MethodAppConnect {
    pub app_connection_registry: Arc<AppConnectionRegistry>,
}

#[async_trait(?Send)]
impl SubAuthController for MethodAppConnect {
    fn auth(
        self: Arc<Self>,
        _toolbox: &ArcToolbox,
        param: Value,
        _ctx: RequestContext,
        conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, eyre::Result<Value>> {
        let registry = self.app_connection_registry.clone();
        let conn_id = conn.connection_id;
        async move {
            let req: AppConnectRequest = serde_json::from_value(param)
                .map_err(|e| eyre::eyre!("Invalid request: {e}"))?;

            let app_public_id_nanoid = req.app_public_id;
            let app_public_id: AppPublicId = app_public_id_nanoid.into();

            registry.register(conn_id, app_public_id).await;

            conn.set_roles(Arc::new(vec![UserRole::App as u32]));

            Ok(serde_json::to_value(AppConnectResponse {
                app_public_id: app_public_id_nanoid,
                app_name: None,
            })?)
        }
        .boxed_local()
    }
}