use std::sync::Arc;

use endpoint_libs::libs::toolbox::{ArcToolbox, CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::HandlerResultExt;
use endpoint_libs::libs::ws::{AuthResponse, SubAuthController, WsConnection};
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use honey_id_types::id_entities::UserPublicId;
use ring::digest::{SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use subtle::ConstantTimeEq;

use crate::admission::AppAdmissionScope;
use crate::codegen::model::{AppConnectRequest, AppConnectResponse, EnumErrorCode, UserRole};
use crate::db::schema::visitor_credential::VisitorCredentialRow;
use crate::db::tables::Tables;
use crate::id_types::{AppPublicId, NanoId, PackedNanoId};
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::user_connection_registry::UserConnectionRegistry;

mod upgrade;

pub(crate) use upgrade::upgrade_visitor_handshake;

const SECRET_BYTES: usize = 32;
const ID_ATTEMPTS: usize = 8;

/// An anonymous visitor is bound to one app by a server-issued credential.
pub struct MethodAppConnect {
    pub tables: Arc<Tables>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

fn invalid_credential() -> CustomError {
    CustomError::new(EnumErrorCode::Forbidden).with_message("Invalid visitor credential")
}

fn secret_digest(secret: &[u8; SECRET_BYTES]) -> String {
    hex::encode(digest(&SHA256, secret).as_ref())
}

fn resume_visitor(
    tables: &Tables,
    app_public_id: PackedNanoId,
    user_public_id: NanoId,
    proof: &str,
) -> Result<(), CustomError> {
    let mut secret = [0; SECRET_BYTES];
    hex::decode_to_slice(proof, &mut secret).map_err(|_| invalid_credential())?;
    let user_public_id = PackedNanoId::pack(&user_public_id).map_err(|_| invalid_credential())?;
    let row = tables
        .visitor_credential_table
        .select(user_public_id)
        .filter(|row| row.app_public_id == Some(app_public_id))
        .ok_or_else(invalid_credential)?;
    let stored_digest = row.secret_digest.ok_or_else(invalid_credential)?;
    let submitted_digest = secret_digest(&secret);
    if !bool::from(stored_digest.as_bytes().ct_eq(submitted_digest.as_bytes()))
        || tables.user_table.select_by_pub_id(user_public_id).is_some()
    {
        return Err(invalid_credential());
    }
    Ok(())
}

async fn issue_visitor(
    tables: &Tables,
    app_public_id: PackedNanoId,
) -> eyre::Result<(NanoId, String)> {
    let mut secret = [0; SECRET_BYTES];
    SystemRandom::new()
        .fill(&mut secret)
        .map_err(|_| eyre::eyre!("Could not generate visitor credential"))?;
    let digest = secret_digest(&secret);

    // UserService uses the same lock when creating a local account.
    let _allocation = tables.visitor_identity_allocation.lock().await;
    for _ in 0..ID_ATTEMPTS {
        let user_public_id = NanoId::new();
        let visitor_id = PackedNanoId::pack(&user_public_id)
            .map_err(|_| eyre::eyre!("Could not pack generated visitor ID"))?;
        if tables.visitor_credential_table.select(visitor_id).is_some()
            || tables.user_table.select_by_pub_id(visitor_id).is_some()
        {
            continue;
        }
        tables
            .visitor_credential_table
            .insert(VisitorCredentialRow {
                visitor_id,
                app_public_id: Some(app_public_id),
                secret_digest: Some(digest),
            })
            .await?;
        tables.wait_for_visitor_ops().await?;
        return Ok((user_public_id, hex::encode(secret)));
    }
    eyre::bail!("Could not allocate a visitor ID")
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
            let packed_app_id = app_public_id.pack().internal()?;
            let app = self
                .tables
                .app_config_table
                .select_by_public_id(packed_app_id)
                .filter(|app| app.active)
                .ok_or_else(|| {
                    CustomError::new(EnumErrorCode::NotFound).with_message("No such app")
                })?;

            let (user_public_id, visitor_secret) = match (req.user_public_id, req.visitor_secret) {
                (None, None) => {
                    AppAdmissionScope::claim(&conn.extensions, app_public_id)?;
                    let (id, secret) = issue_visitor(&self.tables, packed_app_id)
                        .await
                        .internal()?;
                    (id, Some(secret))
                }
                (Some(id), Some(secret)) => {
                    resume_visitor(&self.tables, packed_app_id, id, &secret)?;
                    AppAdmissionScope::claim(&conn.extensions, app_public_id)?;
                    (id, None)
                }
                _ => return Err(invalid_credential().into()),
            };

            self.app_connection_registry
                .register(conn_id, app_public_id)
                .await;
            self.user_connection_registry
                .register(conn_id, UserPublicId::from(user_public_id))
                .await;
            conn.set_roles(Arc::new(vec![UserRole::App as u32]));

            Ok(AppConnectResponse {
                app_public_id: req.app_public_id,
                app_name: app.app_name,
                user_public_id,
                visitor_secret,
            })
        }
        .boxed_local()
    }
}
