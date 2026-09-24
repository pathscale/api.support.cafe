//! The slice of the Telegram Bot API this service uses: `getUpdates` long
//! polling and `sendMessage`, and the few fields of a message the router reads.
//!
//! It replaces tgbot, whose client is reqwest on tokio. Requests go through
//! [`nago_http::Client`]s, which run their own reactor thread, so [`Api`]'s
//! futures complete under whatever executor awaits them.

use std::sync::OnceLock;
use std::time::Duration;

use eyre::{Result, bail, eyre};
use nago_http::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const HOST: &str = "api.telegram.org";

/// How long Telegram may hold a `getUpdates` open before answering empty.
const LONG_POLL_SECS: u64 = 25;

/// Slack over the long poll for the round trip. A request past this is a dead
/// connection rather than a slow one.
const REQUEST_GRACE: Duration = Duration::from_secs(15);

/// The deadline for a `getUpdates`: the long poll plus the round trip.
const LONG_POLL_LIMIT: Duration = Duration::from_secs(LONG_POLL_SECS).saturating_add(REQUEST_GRACE);

/// One client per deadline, shared by every bot in the process.
fn client(cell: &'static OnceLock<Client>, timeout: Duration) -> Result<&'static Client> {
    if let Some(client) = cell.get() {
        return Ok(client);
    }
    let client = Client::new()?.with_timeout(timeout);
    // Losing a race drops this one; the winner serves everyone.
    Ok(cell.get_or_init(|| client))
}

pub struct Api {
    token: String,
    /// For `getUpdates`, whose answer can take the whole long poll.
    long_poll: &'static Client,
    /// For every other method.
    short: &'static Client,
}

impl Api {
    pub fn new(token: String) -> Result<Self> {
        static LONG_POLL: OnceLock<Client> = OnceLock::new();
        static SHORT: OnceLock<Client> = OnceLock::new();
        Ok(Self {
            token,
            long_poll: client(&LONG_POLL, LONG_POLL_LIMIT)?,
            short: client(&SHORT, REQUEST_GRACE)?,
        })
    }

    pub async fn get_updates(&self, offset: i64) -> Result<Vec<Update>> {
        #[derive(Serialize)]
        struct GetUpdates {
            offset: i64,
            timeout: u64,
            allowed_updates: [&'static str; 1],
        }
        self.call(
            self.long_poll,
            LONG_POLL_LIMIT,
            "getUpdates",
            &GetUpdates {
                offset,
                timeout: LONG_POLL_SECS,
                allowed_updates: ["message"],
            },
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
            .call(
                self.short,
                REQUEST_GRACE,
                "sendMessage",
                &SendMessage { chat_id, text },
            )
            .await?;
        Ok(())
    }

    /// `limit` is `client`'s deadline, named in the timeout error.
    async fn call<T: DeserializeOwned>(
        &self,
        client: &Client,
        limit: Duration,
        method: &str,
        params: &impl Serialize,
    ) -> Result<T> {
        let body = serde_json::to_vec(params)?;
        let url = format!("https://{HOST}/bot{}/{method}", self.token);
        // The token is in the path, so no error below may carry the URL.
        // nago_http's errors never carry the path.
        let response = client
            .post(&url, &[], "application/json", &body)
            .await
            .map_err(|e| match e {
                nago_http::Error::Timeout(_) => {
                    eyre!("Telegram {method} timed out after {limit:?}")
                }
                e => e.into(),
            })?;
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
