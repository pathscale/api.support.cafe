use std::sync::Arc;

use endpoint_libs::libs::ws::{EndpointAuthController, WebsocketServer};
use honey_id_types::enums::HoneyEndpointMethodCode;
use honey_id_types::handlers::auth_to_app::{MethodApiKeyConnect, MethodReceiveToken, MethodReceiveUserDeleted, MethodReceiveUserInfo};
use honey_id_types::handlers::convenience_utils::generic_auth_handler::{AuthorizedConnectContext, AuthorizedConnectRequest, GenericAuthorizedConnect};
use honey_id_types::handlers::convenience_utils::token_management::TokenStorage;
use honey_id_types::HoneyIdClient;

use crate::codegen::model::{EnumEndpoint, InitRequest, InitResponse};
use crate::db::tables::Tables;
use crate::handlers::app::auth::MethodAppConnect;
use crate::service::app_connection_registry::AppConnectionRegistry;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

async fn init_handler(
    _req: InitRequest,
    ctx: AuthorizedConnectContext,
    tables: Arc<Tables>,
) -> eyre::Result<InitResponse> {
    let packed_id = ctx.user_pub_id.pack()?;
    let user = tables
        .user_table
        .select_by_pub_id(packed_id)
        .ok_or_else(|| eyre::eyre!("User {} not found", ctx.user_pub_id))?;

    ctx.conn.set_user_id(user.id);

    Ok(InitResponse {
        user_id: user.pub_id().into(),
        role: user.role,
        version: APP_VERSION.to_string(),
    })
}

impl AuthorizedConnectRequest for InitRequest {
    fn get_access_token(&self) -> &str {
        &self.access_token
    }
}

pub fn register_auth_api_handlers(
    server: &mut WebsocketServer,
    tables: Arc<Tables>,
    token_storage: Arc<dyn TokenStorage + Sync + Send>,
    honey_id_client: Arc<HoneyIdClient>,
    app_connection_registry: Arc<AppConnectionRegistry>,
) {
    let mut auth_controller = EndpointAuthController::default();

    let tables_clone = tables.clone();

    auth_controller.add_auth_endpoint(
        EnumEndpoint::Init.schema(),
        GenericAuthorizedConnect::<InitRequest, InitResponse>::new(
            token_storage.clone(),
            tables.user_table.clone(),
            move |req, ctx| {
                let tables = tables_clone.clone();
                init_handler(req, ctx, tables)
            },
        ),
    );

    auth_controller.add_auth_endpoint(
        HoneyEndpointMethodCode::ApiKeyConnect.schema(),
        MethodApiKeyConnect {
            honey_id_client: honey_id_client.clone(),
            user_storage: tables.user_table.clone(),
        },
    );

    auth_controller.add_auth_endpoint(
        EnumEndpoint::AppConnect.schema(),
        MethodAppConnect {
            app_connection_registry,
        },
    );

    server.set_auth_controller(auth_controller);

    server.add_handler(MethodReceiveToken {
        token_storage: token_storage.clone(),
        user_storage: tables.user_table.clone(),
    });

    server.add_handler(MethodReceiveUserInfo {
        token_storage,
        user_storage: tables.user_table.clone(),
    });

    server.add_handler(MethodReceiveUserDeleted {
        token_storage: Arc::new(honey_id_types::handlers::convenience_utils::token_management::TokenWorkTableStorage::default()),
        user_storage: tables.user_table.clone(),
    });
}