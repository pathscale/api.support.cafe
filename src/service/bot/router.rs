use std::collections::HashMap;
use std::sync::Arc;

use crate::codegen::model::ChatMessage;
use chrono::Utc;
use crossfire::mpsc::{One, new};
use crossfire::stream::AsyncStream;
use crossfire::{AsyncRx, MAsyncTx};
use eyre::Result;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;
use futures::{StreamExt, future};
use honey_id_types::id_entities::UserPublicId;
use nagoya::sync::RwLock;
use parking_lot::Mutex as StdMutex;
use serde::Serialize;
use std::pin::pin;
use std::time::Duration;
use tracing::{info, warn};

use crate::db::schema::chat_session::ChatSessionWorkTable;
use crate::db::schema::support_message::SupportMessageRow;
use crate::handlers::utils::routing_message::RoutingMessage;
use crate::id_types::{AppPublicId, PackedNanoId, SessionId};
use crate::service::app::{AppService, AppUpdate, TelegramRecipient, private_telegram_sender};
use crate::service::message_store::MessageStore;

use nago_telegram::{Bot, Message, SendMessage};

/// How long Telegram may hold a `getUpdates` open before answering empty.
const LONG_POLL: Duration = Duration::from_secs(25);

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
    app_service: Arc<AppService>,
    chat_session_table: Arc<ChatSessionWorkTable>,
    message_store: Arc<MessageStore>,
}

impl BotRouter {
    pub fn new(
        chat_session_table: Arc<ChatSessionWorkTable>,
        message_store: Arc<MessageStore>,
        app_service: Arc<AppService>,
    ) -> Self {
        let (tx, rx) =
            new::<One<RoutingMessage<SessionKey, ChatMessage>>, MAsyncTx<_>, AsyncRx<_>>();
        Self {
            bots: RwLock::new(HashMap::new()),
            event_tx: tx,
            event_rx: StdMutex::new(Some(rx)),
            app_service,
            chat_session_table,
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
        let mut bots = self.bots.write().await;
        // Create and bootstrap supply a snapshot. A concurrent edit owns
        // reconciliation if it has already changed that configuration.
        let configured = self.app_service.get_app(app_public_id)?;
        if configured.is_some_and(|app| app.active && app.tg_bot_token == token) {
            self.reconcile_bot(&mut bots, app_public_id, Some(token))
                .await?;
        }
        Ok(())
    }

    pub async fn edit_app(&self, app: AppPublicId, update: AppUpdate) -> Result<()> {
        if update.tg_bot_token.is_none() && update.active.is_none() {
            return self.app_service.edit_app(app, update).await;
        }
        let mut bots = self.bots.write().await;
        let edited = self.app_service.edit_app(app, update).await;
        // A later field can fail after the token or active flag was written.
        let token = self
            .app_service
            .get_app(app)?
            .filter(|config| config.active)
            .map(|config| config.tg_bot_token);
        let reconciled = self.reconcile_bot(&mut bots, app, token).await;
        edited.and(reconciled)
    }

    async fn reconcile_bot(
        &self,
        bots: &mut HashMap<AppPublicId, BotInstance>,
        app_public_id: AppPublicId,
        token: Option<String>,
    ) -> Result<()> {
        if let (Some(instance), Some(token)) = (bots.get(&app_public_id), token.as_deref())
            && self
                .app_service
                .telegram_bot_matches(app_public_id, &instance.generation, token)
                .await
        {
            return Ok(());
        }
        self.app_service
            .deactivate_telegram_bot(app_public_id)
            .await;
        if let Some(mut instance) = bots.remove(&app_public_id) {
            instance.stop().await;
        }
        let Some(token) = token else {
            return Ok(());
        };
        let generation = Arc::new(());
        let handler = BotUpdateHandler {
            app_public_id,
            generation: generation.clone(),
            app_service: self.app_service.clone(),
            chat_session_table: self.chat_session_table.clone(),
            message_store: self.message_store.clone(),
            event_tx: self.event_tx.clone(),
        };

        self.app_service
            .activate_telegram_bot(app_public_id, generation.clone(), &token)
            .await;
        let instance = match BotInstance::new(token, handler) {
            Ok(instance) => instance,
            Err(error) => {
                self.app_service
                    .deactivate_telegram_generation(app_public_id, &generation)
                    .await;
                return Err(error);
            }
        };
        bots.insert(app_public_id, instance);
        info!(?app_public_id, "bot registered");
        Ok(())
    }

    pub async fn unregister_bot(&self, app_public_id: AppPublicId) {
        let mut bots = self.bots.write().await;
        self.app_service
            .deactivate_telegram_bot(app_public_id)
            .await;
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

        self.forward_to_telegram(app_public_id, format!("{msg_prefix}{content}"))
            .await;

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

    /// A reply from the desk's staff through the API rather than Telegram. The
    /// visitor sees it as support, exactly like a Telegram reply; the desk's
    /// support chats get a copy naming who answered, so on-duty staff see it.
    pub async fn send_support_reply(
        &self,
        app_public_id: AppPublicId,
        session_id: SessionId,
        content: String,
        staff: UserPublicId,
    ) -> Result<i64> {
        let app_public_id_packed: PackedNanoId = app_public_id.pack()?;
        let sent_at = Utc::now().timestamp_millis();

        self.message_store
            .store_message(SupportMessageRow {
                id: 0,
                message_id: new_message_id()?,
                session_id: session_id.pack()?,
                app_public_id: app_public_id_packed,
                incoming: true,
                sent_by: "Support".to_string(),
                sent_at,
                content: content.clone(),
                tg_chat_id: None,
            })
            .await?;

        let nanoid: crate::id_types::NanoId = session_id.into();
        self.forward_to_telegram(
            app_public_id,
            format!("{nanoid}\nanswered by {staff} from the dashboard:\n{content}"),
        )
        .await;

        let event = ChatMessage {
            session_id: session_id.into(),
            incoming: true,
            sent_by: "Support".to_string(),
            sent_at,
            content,
        };
        let _ = self
            .event_tx
            .send(RoutingMessage::for_concrete(
                (app_public_id, session_id),
                event,
            ))
            .await;

        Ok(sent_at)
    }

    async fn forward_to_telegram(&self, app: AppPublicId, text: String) {
        let bots = self.bots.read().await;
        if let Some(instance) = bots.get(&app) {
            match self
                .app_service
                .enabled_telegram_recipients(app, &instance.generation)
                .await
            {
                Ok(recipients) => {
                    for recipient in recipients {
                        instance.send(recipient, text.clone());
                    }
                }
                Err(e) => warn!(?app, "Could not resolve Telegram recipients: {e:#}"),
            }
        }
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
        for (app, instance) in bots.iter_mut() {
            self.app_service.deactivate_telegram_bot(*app).await;
            instance.stop().await;
        }
        bots.clear();
        info!("All bots stopped");
    }
}

struct BotInstance {
    generation: Arc<()>,
    outbox: UnboundedSender<(TelegramRecipient, String)>,
    stop: Option<oneshot::Sender<()>>,
    status: Arc<RwLock<BotStatus>>,
}

impl BotInstance {
    /// One thread per bot, blocking on its poll and send loops.
    ///
    /// The sockets are driven by the [`nago_http::Client`]s' own reactor
    /// threads, so this thread only waits. A thread rather than a task on a
    /// shared pool, because a client resolves a host on first use and
    /// `getaddrinfo` blocks: this way a slow lookup stalls one bot, not the
    /// server.
    fn new(token: String, handler: BotUpdateHandler) -> Result<Self> {
        let status = Arc::new(RwLock::new(BotStatus::Running));
        let (outbox, outbox_rx) = mpsc::unbounded();
        let (stop, stop_rx) = oneshot::channel();
        let thread_status = status.clone();
        let app_public_id = handler.app_public_id;
        let generation = handler.generation.clone();

        std::thread::Builder::new()
            .name("tg-bot".to_string())
            .spawn(move || {
                let outcome = run_bot(token, &handler, &thread_status, outbox_rx, stop_rx);
                nagoya::block_on(
                    handler
                        .app_service
                        .deactivate_telegram_generation(app_public_id, &handler.generation),
                );
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
            generation,
            outbox,
            stop: Some(stop),
            status,
        })
    }

    fn send(&self, recipient: TelegramRecipient, text: String) {
        let chat_id = recipient.chat_id;
        if self.outbox.unbounded_send((recipient, text)).is_err() {
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
    outbox: UnboundedReceiver<(TelegramRecipient, String)>,
    stop: oneshot::Receiver<()>,
) -> Result<()> {
    let api = Bot::new(token)?;

    nagoya::block_on(async {
        let poll = poll_updates(&api, handler, status);
        let send = outbox.for_each(|(recipient, text)| {
            let api = &api;
            async move {
                // Reissue can revoke this proof while the copy waits in the
                // outbox, even if the same chat proves a new link meanwhile.
                if handler.admit_delivery(&recipient).await
                    && let Err(e) = api
                        .send_message(&SendMessage::new(recipient.chat_id, text))
                        .await
                {
                    let chat_id = recipient.chat_id;
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

async fn poll_updates(api: &Bot, handler: &BotUpdateHandler, status: &RwLock<BotStatus>) {
    const MAX_BACKOFF: Duration = Duration::from_secs(60);
    let mut offset = 0;
    let mut backoff = Duration::from_secs(1);
    loop {
        match api.get_updates(Some(offset), LONG_POLL, &["message"]).await {
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
    generation: Arc<()>,
    app_service: Arc<AppService>,
    chat_session_table: Arc<ChatSessionWorkTable>,
    message_store: Arc<MessageStore>,
    event_tx: SupportEventProducer,
}

impl BotUpdateHandler {
    async fn admit_delivery(&self, recipient: &TelegramRecipient) -> bool {
        self.app_service
            .can_deliver_telegram(self.app_public_id, &self.generation, recipient)
            .await
    }

    async fn handle(&self, api: &Bot, message: Message) {
        if let Some(notice) = self.process(&message).await
            && let Err(e) = api
                .send_message(&SendMessage::new(message.chat.id, notice))
                .await
        {
            warn!("Error sending message: {e:#}");
        }
    }

    async fn process(&self, message: &Message) -> Option<&'static str> {
        let sender = private_telegram_sender(message)?;
        if message.command().is_some_and(|cmd| cmd.name == "/start") {
            return match self
                .app_service
                .redeem_telegram_start(self.app_public_id, &self.generation, message)
                .await
            {
                Ok(true) => Some("Telegram verified for this support desk."),
                Ok(false) => Some("Open the support dashboard for a current verification code."),
                Err(e) => {
                    warn!("Could not verify Telegram link: {e:#}");
                    Some("Could not verify Telegram right now.")
                }
            };
        }

        let origin = message.reply_to_message.as_ref()?.text()?;
        let chat_id = sender.id;
        let allowed = self
            .app_service
            .enabled_telegram_recipients(self.app_public_id, &self.generation)
            .await
            .is_ok_and(|recipients| {
                recipients
                    .iter()
                    .any(|recipient| recipient.chat_id == chat_id)
            });
        if !allowed {
            return Some("Support access is disabled");
        }

        let Some(session_id) = origin
            .lines()
            .next()
            .and_then(|line| line.trim().parse::<crate::id_types::NanoId>().ok())
            .map(SessionId::from)
        else {
            return Some("Session ID not found in reply");
        };
        let (Ok(packed_session_id), Ok(packed_app_public_id), Ok(message_id)) = (
            session_id.pack(),
            self.app_public_id.pack(),
            new_message_id(),
        ) else {
            return Some("Internal Server Error");
        };
        let Some(session) = self
            .chat_session_table
            .select_by_session_id(packed_session_id)
        else {
            return Some("Session not found");
        };
        if session.app_public_id != packed_app_public_id {
            return Some("Session not found");
        }
        let reply_txt = message.text()?;
        let sent_at = Utc::now().timestamp_millis();
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
            return Some("Internal Server Error");
        }
        let event = ChatMessage {
            session_id: session_id.into(),
            incoming: true,
            sent_by: "Support".to_string(),
            sent_at,
            content: reply_txt.to_string(),
        };
        let _ = self
            .event_tx
            .send(RoutingMessage::for_concrete(
                (self.app_public_id, session_id),
                event,
            ))
            .await;
        None
    }
}

fn new_message_id() -> eyre::Result<PackedNanoId> {
    PackedNanoId::pack(&crate::id_types::NanoId::new())
        .map_err(|e| eyre::eyre!("Failed to pack message_id: {e}"))
}

#[cfg(all(test, not(feature = "s3-sync")))]
mod telegram_ownership_tests;
