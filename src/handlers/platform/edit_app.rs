use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{EditAppRequest, EditAppResponse};
use crate::db::schema::app_config::{AppConfigWorkTable, TgBotTokenByPubIdQuery, AppNameByPubIdQuery, ActiveByPubIdQuery};
use crate::id_types::{AppPublicId, PackedNanoId};
use crate::service::bot_router::BotRouter;

pub struct MethodEditApp {
    pub app_config_table: Arc<AppConfigWorkTable>,
    pub bot_router: Arc<BotRouter>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodEditApp {
    type Request = EditAppRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let packed_pub_id: PackedNanoId = PackedNanoId::pack(&req.app_public_id)
            .map_err(|e| eyre::eyre!("Pack error: {e}"))?;
        let app_public_id: AppPublicId = packed_pub_id.into();

        if let Some(token) = &req.tg_bot_token {
            self.app_config_table
                .update_tg_bot_token_by_pub_id(TgBotTokenByPubIdQuery { tg_bot_token: token.clone() }, packed_pub_id)
                .await?;
            self.bot_router.unregister_bot(app_public_id).await;
            self.bot_router.register_bot(app_public_id, token.clone()).await?;
        }
        if let Some(name) = &req.app_name {
            self.app_config_table
                .update_app_name_by_pub_id(AppNameByPubIdQuery { app_name: Some(name.clone()) }, packed_pub_id)
                .await?;
        }
        if let Some(active) = req.active {
            self.app_config_table
                .update_active_by_pub_id(ActiveByPubIdQuery { active }, packed_pub_id)
                .await?;
        }

        Ok(EditAppResponse {})
    }
}
