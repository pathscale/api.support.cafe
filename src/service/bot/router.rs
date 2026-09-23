use std::collections::HashMap;
use std::sync::Arc;

use crate::codegen::model::AppMemberRole;
use crate::codegen::model::ChatMessage;
use chrono::Utc;
use crossfire::mpsc::{One, new};
use crossfire::stream::AsyncStream;
use crossfire::{AsyncRx, MAsyncTx};
use eyre::Result;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use futures::{StreamExt, future};
use nagoya::reactor::{Reactor, block_on_with};
use nagoya::sync::RwLock;
use parking_lot::Mutex as StdMutex;
use serde::Serialize;
use std::pin::pin;
use std::time::Duration;
use tracing::{info, warn};
use worktable::prelude::SelectQueryExecutor;

use crate::db::schema::app_member::AppMemberWorkTable;
use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_info::{SupportInfoColumns, SupportInfoWorkTable};
use crate::db::schema::support_message::SupportMessageRow;
use crate::handlers::utils::routing_message::RoutingMessage;
use crate::id_types::{AppPublicId, PackedNanoId, SessionId};
use crate::service::message_store::MessageStore;

use super::telegram;

pub type SessionKey = (AppPublicId, SessionId);

pub type SupportEventTx = MAsyncTx<One<RoutingMessage<SessionKey, ChatMessage>>>;
pub type SupportEventRx = AsyncRx<One<RoutingMessage<SessionKey, ChatMessage>>>;
pub type SupportEventStream = AsyncStream<One<RoutingMessage<SessionKey, ChatMessage>>>;

/// The sender, shared as itself.
///
/// It used to be an `Arc<Mutex<_>>` over a single-producer sender, which is
/// what a single-producer channel costs when every bot is a producer: one
/// global lock, held across the send, serialising every app's events behind
/// every other app's. An mpsc sender is `Clone` and `Sync`, so there is nothing
/// left to serialise and nothing left to lock.
pub type SupportEventProducer = SupportEventTx;

#[derive(Clone, Debug, Serialize)]
pub enum BotStatus {
    Running,
    Stopped,
    Restarting { next_attempt_ms: u64 },
    Error(String),
}

pub struct BotRouter {
    bots: RwLock<HashMap<AppPublicId, BotInstance>>,
    event_tx: SupportEventProducer,
    /// A plain mutex, and not an async one: the receiver is taken exactly once
    /// and `take` does not await. An async lock here would also not compile,
    /// because an `AsyncRx` is deliberately not `Sync`.
    event_rx: StdMutex<Option<SupportEventRx>>,
    app_member_table: Arc<AppMemberWorkTable>,
    chat_session_table: Arc<ChatSessionWorkTable>,
    support_info_table: Arc<SupportInfoWorkTable>,
    message_store: Arc<MessageStore>,
}

impl BotRouter {
    pub fn new(
        app_member_table: Arc<AppMemberWorkTable>,
        chat_session_table: Arc<ChatSessionWorkTable>,
        support_info_table: Arc<SupportInfoWorkTable>,
        message_store: Arc<MessageStore>,
    ) -> Self {
        let (tx, rx) =
            new::<One<RoutingMessage<SessionKey, ChatMessage>>, MAsyncTx<_>, AsyncRx<_>>();
        Self {
            bots: RwLock::new(HashMap::new()),
            event_tx: tx,
            event_rx: StdMutex::new(Some(rx)),
            app_member_table,
            chat_session_table,
            support_info_table,
            message_store,
        }
    }

    pub fn take_event_stream(&self) -> eyre::Result<SupportEventStream> {
        self.event_rx
            .lock()
            .take()
            .map(|rx| rx.into_stream())
            .ok_or_else(|| eyre::eyre!("event stream already taken"))
    }

    pub async fn register_bot(&self, app_public_id: AppPublicId, token: String) -> Result<()> {
        let handler = BotUpdateHandler {
            app_public_id,
            app_member_table: self.app_member_table.clone(),
            chat_session_table: self.chat_session_table.clone(),
            support_info_table: self.support_info_table.clone(),
            message_store: self.message_store.clone(),
            event_tx: self.event_tx.clone(),
        };

        let mut bots = self.bots.write().await;
        if bots.contains_key(&app_public_id) {
            warn!(?app_public_id, "bot already registered, replacing");
            if let Some(mut instance) = bots.remove(&app_public_id) {
                instance.stop().await;
            }
        }

        let instance = BotInstance::new(token, handler)?;
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
        let app_public_id_packed: PackedNanoId = app_public_id.pack()?;
        let supports = self.enabled_support_chat_ids(app_public_id_packed)?;

        let sent_at = Utc::now().timestamp_millis();
        let nanoid: crate::id_types::NanoId = session_id.into();
        let session_id_str = nanoid.to_string();
        let msg_prefix = format!("{session_id_str}\nfrom: {sender_name}\n");

        self.message_store
            .store_message(SupportMessageRow {
                id: 0,
                message_id: new_message_id()?,
                session_id: session_id.pack()?,
                app_public_id: app_public_id.pack()?,
                incoming: false,
                sent_by: sender_name.clone(),
                sent_at,
                content: content.clone(),
                tg_chat_id: None,
            })
            .await?;

        if !supports.is_empty() {
            let bots = self.bots.read().await;
            let instance = bots
                .get(&app_public_id)
                .ok_or_else(|| eyre::eyre!("bot not found for app"))?;
            for chat_id in supports {
                instance.send(chat_id, format!("{msg_prefix}{content}"));
            }
        }

        let session_id_nanoid: crate::id_types::NanoId = session_id.into();
        let event = ChatMessage {
            session_id: session_id_nanoid,
            incoming: false,
            sent_by: sender_name,
            sent_at,
            content,
        };
        let key = (app_public_id, session_id);
        let _ = self
            .event_tx
            .send(RoutingMessage::for_concrete(key, event))
            .await;

        Ok(sent_at)
    }

    fn enabled_support_chat_ids(&self, app_public_id: PackedNanoId) -> Result<Vec<i64>> {
        Ok(self
            .app_member_table
            .select_by_app_public_id(app_public_id)
            .execute()
            .map_err(|e| eyre::eyre!("DB error: {e}"))?
            .into_iter()
            .filter(|r| r.is_support_enabled)
            .filter(|r| {
                matches!(
                    r.role,
                    AppMemberRole::Owner | AppMemberRole::Admin | AppMemberRole::Support
                )
            })
            .filter_map(|r| self.support_info_table.select(r.user_pub_id))
            .filter_map(|r| r.chat_id)
            .collect())
    }

    pub async fn get_status(&self, app_public_id: AppPublicId) -> Option<BotStatus> {
        let bots = self.bots.read().await;
        let instance = bots.get(&app_public_id)?;
        Some(instance.status.read().await.clone())
    }

    pub async fn get_all_statuses(&self) -> HashMap<AppPublicId, BotStatus> {
        let bots = self.bots.read().await;
        let mut statuses = HashMap::new();
        for (id, instance) in bots.iter() {
            statuses.insert(*id, instance.status.read().await.clone());
        }
        statuses
    }

    pub async fn shutdown(&self) {
        let mut bots = self.bots.write().await;
        for instance in bots.values_mut() {
            instance.stop().await;
        }
        bots.clear();
        info!("All bots stopped");
    }
}

struct BotInstance {
    outbox: UnboundedSender<(i64, String)>,
    stop: Option<oneshot::Sender<()>>,
    status: Arc<RwLock<BotStatus>>,
}

impl BotInstance {
    /// One thread per bot, driving its own local reactor.
    ///
    /// Telegram long polling is sockets end to end. It used to stay on tokio
    /// because nagoya had no I/O; nagoya has TCP and DNS now, so the bot runs
    /// on a reactor it owns. A thread rather than a task on a shared reactor,
    /// because `getaddrinfo` blocks and this way a slow lookup stalls one bot,
    /// not the server.
    fn new(token: String, handler: BotUpdateHandler) -> Result<Self> {
        let status = Arc::new(RwLock::new(BotStatus::Running));
        let (outbox, outbox_rx) = mpsc::unbounded();
        let (stop, stop_rx) = oneshot::channel();
        let thread_status = status.clone();
        let app_public_id = handler.app_public_id;

        std::thread::Builder::new()
            .name("tg-bot".to_string())
            .spawn(move || {
                let outcome = run_bot(token, &handler, &thread_status, outbox_rx, stop_rx);
                let final_status = match outcome {
                    Ok(()) => {
                        info!(?app_public_id, "Bot stopped");
                        BotStatus::Stopped
                    }
                    Err(e) => {
                        warn!(?app_public_id, "Bot failed: {e:#}");
                        BotStatus::Error(e.to_string())
                    }
                };
                *nagoya::block_on(thread_status.write()) = final_status;
            })
            .map_err(|e| eyre::eyre!("Failed to start bot thread: {e}"))?;

        Ok(Self {
            outbox,
            stop: Some(stop),
            status,
        })
    }

    fn send(&self, chat_id: i64, text: String) {
        if self.outbox.unbounded_send((chat_id, text)).is_err() {
            warn!(?chat_id, "bot is not running, TG message dropped");
        }
    }

    async fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
            *self.status.write().await = BotStatus::Stopped;
        }
    }
}

fn run_bot(
    token: String,
    handler: &BotUpdateHandler,
    status: &RwLock<BotStatus>,
    outbox: UnboundedReceiver<(i64, String)>,
    stop: oneshot::Receiver<()>,
) -> Result<()> {
    let target = nago_http::Target::resolve(telegram::HOST, 443)?;
    let reactor = Reactor::local().map_err(|e| eyre::eyre!("reactor setup failed: {e:?}"))?;
    let api = telegram::Api::new(token, target, reactor.handle());

    block_on_with(&reactor, async {
        let poll = poll_updates(&api, handler, status);
        let send = outbox.for_each(|(chat_id, text)| {
            let api = &api;
            async move {
                if let Err(e) = api.send_message(chat_id, &text).await {
                    warn!(?chat_id, "failed to send TG message: {e:#}");
                }
            }
        });
        let work = pin!(future::join(poll, send));
        // A dropped sender is a stop too: the instance is gone.
        future::select(work, stop).await;
    });
    Ok(())
}

async fn poll_updates(api: &telegram::Api, handler: &BotUpdateHandler, status: &RwLock<BotStatus>) {
    const MAX_BACKOFF: Duration = Duration::from_secs(60);
    let mut offset = 0;
    let mut backoff = Duration::from_secs(1);
    loop {
        match api.get_updates(offset).await {
            Ok(updates) => {
                backoff = Duration::from_secs(1);
                if !matches!(*status.read().await, BotStatus::Running) {
                    *status.write().await = BotStatus::Running;
                }
                for update in updates {
                    offset = offset.max(update.update_id + 1);
                    if let Some(message) = update.message {
                        handler.handle(api, message).await;
                    }
                }
            }
            Err(e) => {
                warn!(app_public_id = ?handler.app_public_id, "getUpdates failed, retrying in {backoff:?}: {e:#}");
                *status.write().await = BotStatus::Restarting {
                    next_attempt_ms: (Utc::now() + backoff).timestamp_millis() as u64,
                };
                nagoya::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

#[derive(Clone)]
struct BotUpdateHandler {
    app_public_id: AppPublicId,
    app_member_table: Arc<AppMemberWorkTable>,
    chat_session_table: Arc<ChatSessionWorkTable>,
    support_info_table: Arc<SupportInfoWorkTable>,
    message_store: Arc<MessageStore>,
    event_tx: SupportEventProducer,
}

impl BotUpdateHandler {
    fn is_chat_enabled_for_app(&self, app_public_id: PackedNanoId, chat_id: i64) -> bool {
        let Ok(members) = self
            .app_member_table
            .select_by_app_public_id(app_public_id)
            .execute()
        else {
            return false;
        };

        members
            .into_iter()
            .filter(|r| r.is_support_enabled)
            .filter(|r| {
                matches!(
                    r.role,
                    AppMemberRole::Owner | AppMemberRole::Admin | AppMemberRole::Support
                )
            })
            .filter_map(|r| self.support_info_table.select(r.user_pub_id))
            .any(|info| info.chat_id == Some(chat_id))
    }

    async fn handle(&self, api: &telegram::Api, message: telegram::Message) {
        let chat_id = message.chat.id;
        let try_send_msg = |msg: &'static str| async move {
            let _ = api
                .send_message(chat_id, msg)
                .await
                .inspect_err(|e| warn!("Error sending message: {e:#}"));
        };
        if let Some(origin_msg) = &message.reply_to_message {
            if let Some(origin_txt) = origin_msg.text() {
                let lines: Vec<&str> = origin_txt.lines().collect();
                if lines.is_empty() {
                    try_send_msg("Malformed reply").await;
                    return;
                }
                let session_id_str = lines[0].trim();
                // Session ID is a 16-char Nanoid string
                if session_id_str.len() == 16 {
                    let Ok(session_nanoid) = session_id_str.parse::<crate::id_types::NanoId>()
                    else {
                        try_send_msg("Invalid session ID").await;
                        return;
                    };

                    let session_id: SessionId = session_nanoid.into();
                    let Ok(packed_session_id) = session_id.pack() else {
                        warn!("Failed to pack session_id");
                        try_send_msg("Internal Server Error").await;
                        return;
                    };

                    let Some(session) = self
                        .chat_session_table
                        .select_by_session_id(packed_session_id)
                    else {
                        try_send_msg("Session not found").await;
                        return;
                    };

                    let Ok(packed_app_public_id) = self.app_public_id.pack() else {
                        warn!("Failed to pack app_public_id");
                        try_send_msg("Internal Server Error").await;
                        return;
                    };

                    if session.app_public_id != packed_app_public_id {
                        try_send_msg("Session not found").await;
                        return;
                    }

                    let Some(reply_txt) = message.text() else {
                        try_send_msg("Error fetching reply text").await;
                        return;
                    };

                    let sent_at = Utc::now().timestamp_millis();

                    if !self.is_chat_enabled_for_app(packed_app_public_id, chat_id) {
                        try_send_msg("Support access is disabled").await;
                        return;
                    }

                    let Ok(message_id) = new_message_id() else {
                        warn!("Failed to create message_id");
                        try_send_msg("Internal Server Error").await;
                        return;
                    };

                    if let Err(e) = self
                        .message_store
                        .store_message(SupportMessageRow {
                            id: 0,
                            message_id,
                            session_id: packed_session_id,
                            app_public_id: packed_app_public_id,
                            incoming: true,
                            sent_by: "Support".to_string(),
                            sent_at,
                            content: reply_txt.to_string(),
                            tg_chat_id: Some(chat_id),
                        })
                        .await
                    {
                        warn!("Error saving support msg: {e:?}");
                        try_send_msg("Internal Server Error").await;
                        return;
                    }

                    let session_id_nanoid: crate::id_types::NanoId = session_id.into();
                    let event = ChatMessage {
                        session_id: session_id_nanoid,
                        incoming: true,
                        sent_by: "Support".to_string(),
                        sent_at,
                        content: reply_txt.to_string(),
                    };
                    let key = (self.app_public_id, session_id);
                    let _ = self
                        .event_tx
                        .send(RoutingMessage::for_concrete(key, event))
                        .await;
                } else {
                    try_send_msg("Session ID not found in reply").await;
                }
            }
        } else if message.command() == Some("/start") {
            let Some(user_handle) = message.chat.username.as_deref() else {
                try_send_msg("Couldn't fetch user handle").await;
                return;
            };
            let handle_str = format!("@{user_handle}");

            if self
                .support_info_table
                .select_by_tg_handle(handle_str.clone())
                .is_some()
            {
                if let Err(e) = self
                    .support_info_table
                    .update_by_tg_handle(handle_str, SupportInfoColumns::CHAT_ID, Some(chat_id))
                    .await
                {
                    warn!("Error updating support chat_id: {e:?}");
                    try_send_msg("Internal Server Error").await;
                } else {
                    try_send_msg("Your chat is saved for future use").await;
                }
            }
        }
    }
}

fn new_message_id() -> eyre::Result<PackedNanoId> {
    PackedNanoId::pack(&crate::id_types::NanoId::new())
        .map_err(|e| eyre::eyre!("Failed to pack message_id: {e}"))
}
