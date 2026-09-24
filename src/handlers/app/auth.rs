use std::sync::Arc;

use endpoint_libs::libs::toolbox::{ArcToolbox, CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::HandlerResultExt;
use endpoint_libs::libs::ws::{AuthResponse, SubAuthController, WsConnection};
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use honey_id_types::id_entities::UserPublicId;

use crate::codegen::model::{AppConnectRequest, AppConnectResponse, EnumErrorCode, UserRole};
use crate::db::tables::Tables;
use crate::id_types::AppPublicId;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::user_connection_registry::UserConnectionRegistry;

/// A widget's connection on behalf of an anonymous visitor.
///
/// The widget runs in a visitor's browser, so there is no app secret it could
/// hold, and the visitor's public id is the only credential there is: whoever
/// knows it can read that visitor's chats. The widget must mint it itself, as a
/// random psc-nanoid kept in the visitor's browser, and it must never be the
/// public id of a real account. So this refuses:
///
/// - an app that does not exist or is not active, rather than registering a
///   connection for it;
/// - the public id of any user support.cafe knows, since those are not secret
///   (staff see them, honey.id shows them) and an account signs in with `Init`.
///
/// Before this, any app id and any user id were accepted, so anyone could open
/// any signed-in user's connection and read their sessions. A visitor key the
/// server issues would remove the bearer-id weakness; that is a schema change.
pub struct MethodAppConnect {
    pub tables: Arc<Tables>,
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
        let conn_id = conn.connection_id;
        async move {
            let app_public_id: AppPublicId = req.app_public_id.into();
            let user_public_id: UserPublicId = req.user_public_id.into();

            let app = self
                .tables
                .app_config_table
                .select_by_public_id(app_public_id.pack().internal()?)
                .filter(|app| app.active)
                .ok_or_else(|| {
                    CustomError::new(EnumErrorCode::NotFound).with_message("No such app")
                })?;

            if self
                .tables
                .user_table
                .select_by_pub_id(user_public_id.pack().internal()?)
                .is_some()
            {
                return Err(CustomError::new(EnumErrorCode::Forbidden)
                    .with_message("A registered account signs in with Init, not AppConnect")
                    .into());
            }

            self.app_connection_registry
                .register(conn_id, app_public_id)
                .await;
            self.user_connection_registry
                .register(conn_id, user_public_id)
                .await;
            conn.set_roles(Arc::new(vec![UserRole::App as u32]));

            Ok(AppConnectResponse {
                app_public_id: req.app_public_id,
                app_name: app.app_name,
            })
        }
        .boxed_local()
    }
}
