use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::{RequestHandler, Response};
use worktable::prelude::SelectQueryExecutor;

use crate::codegen::model::{GetUsersRequest, GetUsersResponse, UserInfo};
use crate::db::schema::user::UserWorkTable;

#[derive(Clone)]
pub struct MethodGetUsers {
    pub user_table: Arc<UserWorkTable>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodGetUsers {
    type Request = GetUsersRequest;

    async fn handle(&self, ctx: RequestContext, _req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "MethodGetUsers: received request"
        );

        let rows = self.user_table.select_all().execute()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            count = rows.len(),
            "MethodGetUsers: listed users successfully"
        );

        let data: Vec<UserInfo> = rows.into_iter().map(Into::into).collect();

        Ok(GetUsersResponse { data })
    }
}