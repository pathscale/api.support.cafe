use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};

use crate::codegen::model::{SetLogLevelRequest, SetLogLevelResponse};
use crate::service::log::LogService;

pub struct MethodSetLogLevel {
    pub log_service: Arc<LogService>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetLogLevel {
    type Request = SetLogLevelRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        self.log_service.set_level(req.level)?;
        Ok(SetLogLevelResponse {})
    }
}