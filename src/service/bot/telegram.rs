//! The slice of the Telegram Bot API this service uses: `getUpdates` long
//! polling and `sendMessage`, and the few fields of a message the router reads.
//!
//! It replaces tgbot, whose client is reqwest on tokio. Everything here runs on
//! the bot's own thread, over that thread's local nagoya reactor, which is why
//! [`Api`] holds a reactor [`Handle`] and is not shared across threads.

use std::time::Duration;

use eyre::{Result, bail, eyre};
use nagoya::reactor::Handle;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const HOST: &str = "api.telegram.org";

/// How long Telegram may hold a `getUpdates` open before answering empty.
const LONG_POLL_SECS: u64 = 25;

/// Slack over the long poll for the round trip. A request past this is a dead
/// connection rather than a slow one.
const REQUEST_GRACE: Duration = Duration::from_secs(15);

pub struct Api {
    token: String,
    target: nago_http::Target,
    handle: Handle,
}

impl Api {
    pub fn new(token: String, target: nago_http::Target, handle: Handle) -> Self {
        Self {
            token,
            target,
            handle,
        }
    }

    pub async fn get_updates(&self, offset: i64) -> Result<Vec<Update>> {
        #[derive(Serialize)]
        struct GetUpdates {
            offset: i64,
            timeout: u64,
            allowed_updates: [&'static str; 1],
        }
        self.call(
            "getUpdates",
            &GetUpdates {
                offset,
                timeout: LONG_POLL_SECS,
                allowed_updates: ["message"],
            },
            Duration::from_secs(LONG_POLL_SECS) + REQUEST_GRACE,
        )
        .await
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        #[derive(Serialize)]
        struct SendMessage<'a> {
            chat_id: i64,
            text: &'a str,
        }
        let _: serde_json::Value = self
            .call("sendMessage", &SendMessage { chat_id, text }, REQUEST_GRACE)
            .await?;
        Ok(())
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: &impl Serialize,
        limit: Duration,
    ) -> Result<T> {
        let body = serde_json::to_vec(params)?;
        let path = format!("/bot{}/{method}", self.token);
        let request = nago_http::send(
            &self.target,
            &self.handle,
            nago_http::Request::post(&path).body("application/json", &body),
        );
        // The token is in the path, so no error below may carry the path.
        let response = nagoya::timeout(limit, request)
            .await
            .map_err(|_| eyre!("Telegram {method} timed out after {limit:?}"))??;
        let reply: Reply<T> = serde_json::from_slice(&response.body).map_err(|e| {
            eyre!(
                "Telegram {method} answered HTTP {} with an unreadable body: {e}",
                response.status
            )
        })?;
        match reply {
            Reply {
                ok: true,
                result: Some(result),
                ..
            } => Ok(result),
            Reply { description, .. } => bail!(
                "Telegram {method} failed with HTTP {}: {}",
                response.status,
                description.unwrap_or_default()
            ),
        }
    }
}

#[derive(Deserialize)]
struct Reply<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub chat: Chat,
    text: Option<String>,
    caption: Option<String>,
    pub reply_to_message: Option<Box<Message>>,
}

impl Message {
    /// The message text, or a media message's caption, as tgbot's `get_text`.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref().or(self.caption.as_deref())
    }

    /// The command this message starts with, without its `@botname` suffix.
    pub fn command(&self) -> Option<&str> {
        let word = self.text.as_deref()?.split_whitespace().next()?;
        if !word.starts_with('/') {
            return None;
        }
        Some(word.split('@').next().unwrap_or(word))
    }
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
    pub username: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(json: &str) -> Message {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn command_strips_the_bot_name() {
        let m = message(r#"{"chat":{"id":1},"text":"/start@support_bot payload"}"#);
        assert_eq!(m.command(), Some("/start"));
        let m = message(r#"{"chat":{"id":1},"text":"hello /start"}"#);
        assert_eq!(m.command(), None);
    }

    #[test]
    fn reply_and_caption_are_read() {
        let update: Update = serde_json::from_str(
            r#"{"update_id":7,"message":{"chat":{"id":-5,"username":"sup","type":"private"},
                "caption":"reply text",
                "reply_to_message":{"chat":{"id":-5},"text":"0123456789abcdef\nfrom: a\nhi"}}}"#,
        )
        .unwrap();
        let message = update.message.unwrap();
        assert_eq!(message.chat.username.as_deref(), Some("sup"));
        assert_eq!(message.text(), Some("reply text"));
        let origin = message.reply_to_message.unwrap();
        assert_eq!(
            origin.text().unwrap().lines().next(),
            Some("0123456789abcdef")
        );
    }

    #[test]
    #[ignore = "reaches api.telegram.org"]
    fn a_bad_token_is_refused_by_telegram() {
        let target = nago_http::Target::resolve(HOST, 443).unwrap();
        let reactor = nagoya::reactor::Reactor::local().unwrap();
        let api = Api::new("0:bogus".to_string(), target, reactor.handle());
        let err = nagoya::reactor::block_on_with(&reactor, api.get_updates(0)).unwrap_err();
        assert!(err.to_string().contains("HTTP 401"), "{err}");
        assert!(!err.to_string().contains("bogus"), "token leaked: {err}");
    }

    #[test]
    fn failed_reply_carries_the_description() {
        let reply: Reply<serde_json::Value> =
            serde_json::from_str(r#"{"ok":false,"error_code":401,"description":"Unauthorized"}"#)
                .unwrap();
        assert!(!reply.ok);
        assert_eq!(reply.description.as_deref(), Some("Unauthorized"));
    }
}
