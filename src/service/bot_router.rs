use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use crossfire::spsc::{One, new};
use crossfire::stream::AsyncStream;
use crossfire::{AsyncRx, AsyncTx};
use eyre::Result;
use serde::Serialize;
use tgbot::api::Client;
use tgbot::handler::{LongPoll, UpdateHandler};
use tgbot::types::{ChatPeerId, Command, ReplyTo, SendMessage, Update, UpdateType};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};
use worktable::prelude::SelectQueryExecutor;

use crate::db::schema::support_message::{SupportMessageRow, SupportMessageWorkTable};
use crate::db::schema::support_user::{ChatIdByHandleQuery, SupportUserWorkTable};
use crate::id_types::{AppPublicId, SessionId, PackedNanoId};
use crate::handlers::utils::routing_message::RoutingMessage;

pub type SessionKey = (AppPublicId, SessionId);

pub type SupportEventTx = AsyncTx<One<RoutingMessage<SessionKey, ChatMessageEvent>>>;
pub type SupportEventRx = AsyncRx<One<RoutingMessage<SessionKey, ChatMessageEvent>>>;
pub type SupportEventStream = AsyncStream<One<RoutingMessage<SessionKey, ChatMessageEvent>>>;
pub type SupportEventProducer = Arc<Mutex<SupportEventTx>>;

#[derive(Clone, Serialize)]
pub struct ChatMessageEvent {
    #[serde(serialize_with = "serialize_session_id")]
    pub session_id: SessionId,
    pub incoming: bool,
    pub sent_by: String,
    pub sent_at: i64,
    pub content: String,
}

fn serialize_session_id<S>(id: &SessionId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let nanoid: crate::id_types::NanoId = (*id).into();
    nanoid.to_string().serialize(serializer)
}

pub struct BotRouter {
    bots: RwLock<HashMap<AppPublicId, BotInstance>>,
    event_tx: SupportEventProducer,
    event_rx: Mutex<Option<SupportEventRx>>,
    support_user_table: Arc<SupportUserWorkTable>,
    support_message_table: Arc<SupportMessageWorkTable>,
}

impl BotRouter {
    pub fn new(
        support_user_table: Arc<SupportUserWorkTable>,
        support_message_table: Arc<SupportMessageWorkTable>,
    ) -> Self {
        let (tx, rx) = new::<One<RoutingMessage<SessionKey, ChatMessageEvent>>, AsyncTx<_>, AsyncRx<_>>();
        Self {
            bots: RwLock::new(HashMap::new()),
            event_tx: Arc::new(Mutex::new(tx)),
            event_rx: Mutex::new(Some(rx)),
            support_user_table,
            support_message_table,
        }
    }

    pub async fn take_event_stream(&self) -> eyre::Result<SupportEventStream> {
        self.event_rx
            .lock()
            .await
            .take()
            .map(|rx| rx.into_stream())
            .ok_or_else(|| eyre::eyre!("event stream already taken"))
    }

    pub async fn register_bot(&self, app_public_id: AppPublicId, token: String) -> Result<()> {
        let client = Client::new(token).map_err(|e| eyre::eyre!("Failed to create TG client: {e}"))?;
        let client_arc = Arc::new(client);
        let handler = BotUpdateHandler {
            client: client_arc.clone(),
            support_user_table: self.support_user_table.clone(),
            support_message_table: self.support_message_table.clone(),
            event_tx: self.event_tx.clone(),
        };

        let mut bots = self.bots.write().await;
        if bots.contains_key(&app_public_id) {
            warn!(?app_public_id, "bot already registered, replacing");
            bots.remove(&app_public_id);
        }

        let instance = BotInstance::new(client_arc, handler);
        bots.insert(app_public_id, instance);
        info!(?app_public_id, "bot registered");
        Ok(())
    }

    pub async fn unregister_bot(&self, app_public_id: AppPublicId) {
        let mut bots = self.bots.write().await;
        if let Some(mut instance) = bots.remove(&app_public_id) {
            instance.stop().await;
            info!(?app_public_id, "bot unregistered");
        }
    }

    pub async fn send_message(
        &self,
        app_public_id: AppPublicId,
        session_id: SessionId,
        content: String,
        sender_name: String,
    ) -> Result<i64> {
        // Find support users for this app
        let all_users = self.support_user_table.select_all().execute()
            .map_err(|e| eyre::eyre!("DB error: {e}"))?;
        let app_public_id_packed: PackedNanoId = app_public_id.pack()?;
        let supports: Vec<_> = all_users
            .into_iter()
            .filter(|r| r.app_public_id == app_public_id_packed)
            .filter_map(|r| r.chat_id)
            .collect();

        let sent_at = Utc::now().timestamp_millis();
        let nanoid: crate::id_types::NanoId = session_id.into();
        let session_id_str = nanoid.to_string();
        let msg_prefix = format!("{session_id_str}\nfrom: {sender_name}\n");

        let session_id_packed: PackedNanoId = session_id.pack()?;

        for chat_id in supports {
            self.support_message_table
                .insert(SupportMessageRow {
                    id: self.support_message_table.get_next_pk().into(),
                    session_id: session_id_packed,
                    app_public_id: app_public_id_packed,
                    incoming: false,
                    sent_by: sender_name.clone(),
                    sent_at,
                    content: content.clone(),
                    tg_chat_id: Some(chat_id),
                })
                .map_err(|e| eyre::eyre!("Insert error: {e}"))?;

            let client = self.get_bot_client(app_public_id).await?;
            let method = SendMessage::new(ChatPeerId::from(chat_id), format!("{msg_prefix}{content}"));
            if let Err(e) = client.execute(method).await {
                warn!(?app_public_id, ?chat_id, "failed to send TG message: {e:?}");
            }
        }

        let event = ChatMessageEvent {
            session_id,
            incoming: false,
            sent_by: sender_name,
            sent_at,
            content,
        };
        let key = (app_public_id, session_id);
        let _ = self.event_tx.lock().await.send(RoutingMessage::for_concrete(key, event)).await;

        Ok(sent_at)
    }

    async fn get_bot_client(&self, app_public_id: AppPublicId) -> Result<Arc<Client>> {
        let bots = self.bots.read().await;
        let instance = bots.get(&app_public_id)
            .ok_or_else(|| eyre::eyre!("bot not found for app"))?;
        Ok(instance.client.clone())
    }
}

struct BotInstance {
    client: Arc<Client>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl BotInstance {
    fn new(client: Arc<Client>, handler: BotUpdateHandler) -> Self {
        let client_for_poll = Arc::unwrap_or_clone(client.clone());
        let handle = tokio::spawn(async move {
            LongPoll::new(client_for_poll, handler).run().await;
        });
        Self {
            client,
            handle: Some(handle),
        }
    }

    async fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[derive(Clone)]
struct BotUpdateHandler {
    client: Arc<Client>,
    support_user_table: Arc<SupportUserWorkTable>,
    support_message_table: Arc<SupportMessageWorkTable>,
    event_tx: SupportEventProducer,
}

impl BotUpdateHandler {
    async fn try_send_msg(&self, chat_id: i64, msg: String) {
        let _ = self
            .client
            .execute(SendMessage::new(ChatPeerId::from(chat_id), msg))
            .await
            .inspect_err(|e| warn!("Error sending message: {e:?}"));
    }
}

impl UpdateHandler for BotUpdateHandler {
    async fn handle(&self, update: Update) {
        let UpdateType::Message(message) = update.update_type else {
            return;
        };
        let chat_id: i64 = message.chat.get_id().into();

        if let Some(ReplyTo::Message(ref origin_msg)) = message.reply_to {
            if let Some(origin_txt) = origin_msg.get_text() {
                let lines: Vec<&str> = origin_txt.data.lines().collect();
                if lines.is_empty() {
                    self.try_send_msg(chat_id, "Malformed reply".to_string()).await;
                    return;
                }
                let session_id_str = lines[0].trim();
                // Session ID is a 16-char Nanoid string
                if session_id_str.len() == 16 {
                    // Find the session by looking up messages with this session_id string representation
                    let all_msgs = self.support_message_table.select_all().execute()
                        .unwrap_or_default();

                    // Find a message where the packed session_id unpacks to this string
                    let Some(first_msg) = all_msgs.iter().find(|m| {
                        let unpacked = m.session_id.unpack();
                        unpacked.map(|n| n.to_string() == session_id_str).unwrap_or(false)
                    }) else {
                        self.try_send_msg(chat_id, "Session not found".to_string()).await;
                        return;
                    };
                    let Ok(session_id) = SessionId::from_packed(first_msg.session_id) else {
                        self.try_send_msg(chat_id, "Invalid session ID".to_string()).await;
                        return;
                    };
                    let Ok(app_public_id) = AppPublicId::from_packed(first_msg.app_public_id) else {
                        self.try_send_msg(chat_id, "Invalid app ID".to_string()).await;
                        return;
                    };

                    let Some(reply_txt) = message.get_text() else {
                        self.try_send_msg(chat_id, "Error fetching reply text".to_string()).await;
                        return;
                    };

                    let sent_at = Utc::now().timestamp_millis();

                    let Ok(packed_session_id) = session_id.pack() else {
                        warn!("Failed to pack session_id");
                        self.try_send_msg(chat_id, "Internal Server Error".to_string()).await;
                        return;
                    };
                    let Ok(packed_app_public_id) = app_public_id.pack() else {
                        warn!("Failed to pack app_public_id");
                        self.try_send_msg(chat_id, "Internal Server Error".to_string()).await;
                        return;
                    };
                    if let Err(e) = self.support_message_table.insert(SupportMessageRow {
                        id: self.support_message_table.get_next_pk().into(),
                        session_id: packed_session_id,
                        app_public_id: packed_app_public_id,
                        incoming: true,
                        sent_by: "Support".to_string(),
                        sent_at,
                        content: reply_txt.data.clone(),
                        tg_chat_id: Some(chat_id),
                    }) {
                        warn!("Error saving support msg: {e:?}");
                        self.try_send_msg(chat_id, "Internal Server Error".to_string()).await;
                        return;
                    }

                    let event = ChatMessageEvent {
                        session_id,
                        incoming: true,
                        sent_by: "Support".to_string(),
                        sent_at,
                        content: reply_txt.data.clone(),
                    };
                    let key = (app_public_id, session_id);
                    let _ = self.event_tx.lock().await.send(RoutingMessage::for_concrete(key, event)).await;
                } else {
                    self.try_send_msg(chat_id, "Session ID not found in reply".to_string()).await;
                }
            }
        } else if let Ok(cmd) = Command::try_from(message.clone()) && cmd.get_name() == "/start" {
            let Some(user_handle) = message.chat.get_username() else {
                self.try_send_msg(chat_id, "Couldn't fetch user handle".to_string()).await;
                return;
            };
            let handle_str = format!("@{user_handle}");

            // Find and update this support user's chat_id
            let all_users = self.support_user_table.select_all().execute()
                .unwrap_or_default();
            if let Some(user) = all_users.iter().find(|u| u.tg_handle == handle_str) {
                if let Err(e) = self.support_user_table.update_chat_id_by_handle(ChatIdByHandleQuery { chat_id: Some(chat_id) }, user.tg_handle.clone()).await {
                    warn!("Error updating support chat_id: {e:?}");
                    self.try_send_msg(chat_id, "Internal Server Error".to_string()).await;
                } else {
                    self.try_send_msg(chat_id, "Your chat is saved for future use".to_string()).await;
                }
            }
        }
    }
}
