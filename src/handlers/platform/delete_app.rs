use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{DeleteAppRequest, DeleteAppResponse};
use crate::db::schema::app_config::AppConfigWorkTable;
use crate::id_types::{AppPublicId, PackedNanoId};
use crate::service::bot_router::BotRouter;

pub struct MethodDeleteApp {
    pub app_config_table: Arc<AppConfigWorkTable>,
    pub bot_router: Arc<BotRouter>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodDeleteApp {
    type Request = DeleteAppRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let app_public_id: AppPublicId = req.app_public_id.into();
        let packed_pub_id: PackedNanoId = app_public_id.pack()?;
        self.bot_router.unregister_bot(app_public_id).await;
        self.app_config_table
            .delete_by_public_id(packed_pub_id)
            .await
            .map_err(|e| eyre::eyre!("Delete error: {e}"))?;
        Ok(DeleteAppResponse {})
    }
}
