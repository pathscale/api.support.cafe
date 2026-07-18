use std::sync::Arc;

use endpoint_libs::libs::toolbox::{ArcToolbox, CustomError, RequestContext};
use endpoint_libs::libs::ws::{AuthResponse, SubAuthController, WsConnection};
use futures::FutureExt;
use futures::future::LocalBoxFuture;

use crate::codegen::model::{AppConnectRequest, AppConnectResponse, UserRole};
use crate::id_types::AppPublicId;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::user_connection_registry::UserConnectionRegistry;

pub struct MethodAppConnect {
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

impl SubAuthController for MethodAppConnect {
    type Request = AppConnectRequest;
    type Error = CustomError;

    fn auth(
        self: Arc<Self>,
        _toolbox: &ArcToolbox,
        req: AppConnectRequest,
        _ctx: RequestContext,
        conn: Arc<WsConnection>,
    ) -> LocalBoxFuture<'static, AuthResponse<Self::Request, Self::Error>> {
        let registry = self.app_connection_registry.clone();
        let user_registry = self.user_connection_registry.clone();
        let conn_id = conn.connection_id;
        async move {
            let app_public_id_nanoid = req.app_public_id;
            let app_public_id: AppPublicId = app_public_id_nanoid.into();
            let user_public_id = req.user_public_id.into();

            registry.register(conn_id, app_public_id).await;
            user_registry.register(conn_id, user_public_id).await;

            conn.set_roles(Arc::new(vec![UserRole::App as u32]));

            Ok(AppConnectResponse {
                app_public_id: app_public_id_nanoid,
                app_name: None,
            })
        }
        .boxed_local()
    }
}
