use std::sync::Arc;

use async_trait::async_trait;
use endpoint_libs::libs::toolbox::{CustomError, RequestContext};
use endpoint_libs::libs::ws::handler::{HandlerResultExt, RequestHandler, Response};

use crate::codegen::model::{
    EnumErrorCode, SubmitContactMessageRequest, SubmitContactMessageResponse,
};
use crate::id_types::SessionId;
use crate::service::app_connection_registry::AppConnectionRegistry;
use crate::service::session::ChatSessionService;
use crate::service::user_connection_registry::UserConnectionRegistry;

const MAX_NAME_CHARS: usize = 200;
const MAX_EMAIL_CHARS: usize = 320;
const MAX_MESSAGE_CHARS: usize = 5000;

#[derive(Clone)]
pub struct MethodSubmitContactMessage {
    pub session_service: Arc<ChatSessionService>,
    pub app_connection_registry: Arc<AppConnectionRegistry>,
    pub user_connection_registry: Arc<UserConnectionRegistry>,
}

#[async_trait(?Send)]
impl RequestHandler for MethodSubmitContactMessage {
    type Request = SubmitContactMessageRequest;
    type Error = CustomError;

    async fn handle(&self, ctx: RequestContext, req: Self::Request) -> Response<Self::Request> {
        tracing::debug!(
            connection_id = ctx.connection_id,
            "SubmitContactMessage: received request"
        );

        validate_contact_message(&req.name, &req.email, &req.message)
            .map_err(|message| CustomError::new(EnumErrorCode::BadRequest).with_message(message))?;

        let app_public_id = self
            .app_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("App connection not authenticated")
            })?;
        let user_pub_id = self
            .user_connection_registry
            .get(ctx.connection_id)
            .await
            .ok_or_else(|| {
                CustomError::new(EnumErrorCode::Unauthorized)
                    .with_message("Visitor identity not authenticated")
            })?;

        let row = self
            .session_service
            .create_session(user_pub_id, app_public_id)
            .await
            .internal()?;
        let session_id = SessionId::from_packed(row.session_id).internal()?;

        let content = format!(
            "Contact form\nName: {}\nEmail: {}\n\n{}",
            req.name, req.email, req.message
        );
        let sent_at = self
            .session_service
            .send_message(session_id, content, user_pub_id)
            .await
            .internal()?;

        tracing::debug!(
            connection_id = ctx.connection_id,
            session_id = %session_id,
            sent_at,
            "SubmitContactMessage: completed successfully"
        );

        Ok(SubmitContactMessageResponse {
            session_id: row.session_id.unpack().expect("valid nanoid"),
            created_at: row.created_at,
        })
    }
}

fn validate_contact_message(name: &str, email: &str, message: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() {
        return Err("Name must not be empty");
    }
    if email.trim().is_empty() {
        return Err("Email must not be empty");
    }
    if message.trim().is_empty() {
        return Err("Message must not be empty");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err("Name must be at most 200 characters");
    }
    if email.chars().count() > MAX_EMAIL_CHARS {
        return Err("Email must be at most 320 characters");
    }
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return Err("Message must be at most 5000 characters");
    }

    let Some((local, domain)) = email.split_once('@') else {
        return Err("Email must contain exactly one '@' with text on both sides");
    };
    if local.trim().is_empty() || domain.trim().is_empty() || domain.contains('@') {
        return Err("Email must contain exactly one '@' with text on both sides");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_EMAIL_CHARS, MAX_MESSAGE_CHARS, MAX_NAME_CHARS, validate_contact_message};

    #[test]
    fn validation_accepts_values_at_character_limits() {
        let name = "é".repeat(MAX_NAME_CHARS);
        let email = format!("{}@x", "é".repeat(MAX_EMAIL_CHARS - 2));
        let message = "é".repeat(MAX_MESSAGE_CHARS);

        assert_eq!(validate_contact_message(&name, &email, &message), Ok(()));
    }

    #[test]
    fn validation_rejects_blank_values() {
        for (name, email, message) in [
            ("  ", "visitor@example.com", "Hello"),
            ("Visitor", "\t", "Hello"),
            ("Visitor", "visitor@example.com", "\n"),
        ] {
            assert!(validate_contact_message(name, email, message).is_err());
        }
    }

    #[test]
    fn validation_rejects_invalid_emails() {
        for email in [
            "missing-at",
            "@example.com",
            "visitor@",
            "visitor@@example.com",
        ] {
            assert!(validate_contact_message("Visitor", email, "Hello").is_err());
        }
    }

    #[test]
    fn validation_rejects_values_over_character_limits() {
        let name = "a".repeat(MAX_NAME_CHARS + 1);
        let email = format!("{}@x", "a".repeat(MAX_EMAIL_CHARS - 1));
        let message = "a".repeat(MAX_MESSAGE_CHARS + 1);

        assert!(validate_contact_message(&name, "visitor@example.com", "Hello").is_err());
        assert!(validate_contact_message("Visitor", &email, "Hello").is_err());
        assert!(validate_contact_message("Visitor", "visitor@example.com", &message).is_err());
    }
}
