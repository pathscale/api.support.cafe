use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use psc_nanoid::alphabet::Base62Alphabet;
use psc_nanoid::Nanoid;

use crate::codegen::model::{CreateAppRequest, CreateAppResponse};
use crate::db::schema::app_config::{AppConfigRow, AppConfigWorkTable};
use crate::id_types::{AppPublicId, PackedNanoId};
use crate::service::bot_router::BotRouter;

pub struct MethodCreateApp {
    pub app_config_table: Arc<AppConfigWorkTable>,
    pub bot_router: Arc<BotRouter>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodCreateApp {
    type Request = CreateAppRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let api_key = uuid::Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp_millis();

        let nanoid: Nanoid<16, Base62Alphabet> = req.app_public_id.to_string().parse()
            .map_err(|e| eyre::eyre!("Invalid app_public_id: {e}"))?;
        let packed_pub_id: PackedNanoId = PackedNanoId::pack(&nanoid)
            .map_err(|e| eyre::eyre!("Failed to pack public ID: {e}"))?;
        let app_public_id: AppPublicId = packed_pub_id.into();

        let row = AppConfigRow {
            id: self.app_config_table.get_next_pk().into(),
            public_id: packed_pub_id,
            tg_bot_token: req.tg_bot_token.clone(),
            api_key: api_key.clone(),
            app_name: req.app_name.clone(),
            active: true,
            created_at,
        };

        self.app_config_table.insert(row)?;
        self.bot_router.register_bot(app_public_id, req.tg_bot_token).await?;

        Ok(CreateAppResponse {
            app_public_id: req.app_public_id,
            api_key,
            created_at,
        })
    }
}
