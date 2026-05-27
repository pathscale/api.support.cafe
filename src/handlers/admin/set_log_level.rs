use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use tokio::sync::RwLock;

use crate::codegen::model::{SetLogLevelRequest, SetLogLevelResponse, LogLevel};

pub struct MethodSetLogLevel {
    pub log_level: Arc<RwLock<tracing::Level>>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSetLogLevel {
    type Request = SetLogLevelRequest;

    async fn handle(
        &self,
        _ctx: RequestContext,
        req: Self::Request,
    ) -> Response<Self::Request> {
        let level = match req.level {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        };
        *self.log_level.write().await = level;
        Ok(SetLogLevelResponse {})
    }
}
