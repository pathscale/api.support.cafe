use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use honey_id_types::id_entities::UserPublicId;
use nago_telegram::{Message, User};
use psc_nanoid::{Nanoid, alphabet::Base62Alphabet};
use ring::digest::{SHA256, digest};

use crate::codegen::model::AppMemberRole;
use crate::db::schema::app_member::AppMemberRow;
use crate::db::schema::telegram_link::TelegramLinkRow;
use crate::id_types::{AppPublicId, NanoId, PackedNanoId};

use super::AppService;

pub(crate) const TELEGRAM_CHALLENGE_TTL: Duration = Duration::from_secs(600);

#[derive(Default)]
pub(super) struct TelegramLinks {
    bots: HashMap<AppPublicId, BotLinks>,
    // WorkTable rows become readable before its persistence barrier returns.
    // Keep a failed or cancelled redemption blocked until its row is revoked.
    uncommitted_members: HashSet<u64>,
}

struct BotLinks {
    generation: Arc<()>,
    token_digest: String,
    pending: HashMap<u64, PendingLink>,
}

struct PendingLink {
    handle: String,
    challenge: String,
    expires_at: Instant,
}

pub(crate) struct TelegramRecipient {
    member_id: u64,
    pub chat_id: i64,
    proof_id: PackedNanoId,
}

pub(crate) struct TelegramLinkStatus {
    pub handle: Option<String>,
    pub state: &'static str,
}

pub(crate) fn normalize_telegram_handle(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let name = trimmed.strip_prefix('@').unwrap_or(trimmed);
    if name.is_empty()
        || name.len() > 32
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return None;
    }
    Some(format!("@{}", name.to_ascii_lowercase()))
}

pub(crate) fn private_telegram_sender(message: &Message) -> Option<&User> {
    message.from.as_ref().filter(|sender| {
        message.chat.is_private() && sender.id > 0 && sender.id == message.chat.id && !sender.is_bot
    })
}

fn can_link(member: &AppMemberRow) -> bool {
    matches!(
        member.role,
        AppMemberRole::Owner | AppMemberRole::Admin | AppMemberRole::Support
    )
}

fn token_digest(token: &str) -> String {
    hex::encode(digest(&SHA256, token.as_bytes()).as_ref())
}

impl AppService {
    pub(crate) fn telegram_member(
        &self,
        app: AppPublicId,
        user: UserPublicId,
    ) -> eyre::Result<Option<AppMemberRow>> {
        Ok(self.get_member(app, user)?.filter(can_link))
    }

    pub(crate) async fn activate_telegram_bot(
        &self,
        app: AppPublicId,
        generation: Arc<()>,
        token: &str,
    ) {
        self.telegram_links.lock().await.bots.insert(
            app,
            BotLinks {
                generation,
                token_digest: token_digest(token),
                pending: HashMap::new(),
            },
        );
    }

    pub(crate) async fn deactivate_telegram_bot(&self, app: AppPublicId) {
        self.telegram_links.lock().await.bots.remove(&app);
    }

    pub(crate) async fn deactivate_telegram_generation(
        &self,
        app: AppPublicId,
        generation: &Arc<()>,
    ) {
        let mut links = self.telegram_links.lock().await;
        if links
            .bots
            .get(&app)
            .is_some_and(|bot| Arc::ptr_eq(&bot.generation, generation))
        {
            links.bots.remove(&app);
        }
    }

    pub(crate) async fn telegram_bot_matches(
        &self,
        app: AppPublicId,
        generation: &Arc<()>,
        token: &str,
    ) -> bool {
        self.telegram_links
            .lock()
            .await
            .bots
            .get(&app)
            .is_some_and(|bot| {
                Arc::ptr_eq(&bot.generation, generation) && bot.token_digest == token_digest(token)
            })
    }

    pub(super) async fn revoke_telegram_app(
        &self,
        app: AppPublicId,
        links: &mut TelegramLinks,
    ) -> eyre::Result<()> {
        links.bots.remove(&app);
        self.telegram_link_table
            .delete_by_app_public_id(app.pack()?)
            .await?;
        self.telegram_link_table.wait_for_ops().await?;
        links
            .uncommitted_members
            .retain(|&id| self.telegram_link_table.select(id).is_some());
        Ok(())
    }

    pub(super) async fn revoke_telegram_member(
        &self,
        app: AppPublicId,
        member_id: u64,
        links: &mut TelegramLinks,
    ) -> eyre::Result<()> {
        if let Some(bot) = links.bots.get_mut(&app) {
            bot.pending.remove(&member_id);
        }
        if self.telegram_link_table.select(member_id).is_some() {
            self.telegram_link_table.delete(member_id).await?;
        }
        self.telegram_link_table.wait_for_ops().await?;
        links.uncommitted_members.remove(&member_id);
        Ok(())
    }

    pub(crate) async fn issue_telegram_link(
        &self,
        member: &AppMemberRow,
        handle: &str,
        now: Instant,
    ) -> eyre::Result<Option<String>> {
        let app = AppPublicId::from_packed(member.app_public_id)?;
        let mut links = self.telegram_links.lock().await;
        let Some(bot) = links.bots.get(&app) else {
            return Ok(None);
        };
        if !self.bot_is_configured(app, bot)
            || !self.app_member_table.select(member.id).is_some_and(|row| {
                can_link(&row)
                    && row.app_public_id == member.app_public_id
                    && row.user_pub_id == member.user_pub_id
                    && row.created_at == member.created_at
                    && row.role == member.role
            })
        {
            return Ok(None);
        }
        self.revoke_telegram_member(app, member.id, &mut links)
            .await?;
        let Some(bot) = links.bots.get_mut(&app) else {
            return Ok(None);
        };

        // ThreadRng is OS-seeded. 32 base62 characters give about 190 bits.
        let challenge = Nanoid::<32, Base62Alphabet>::new().to_string();
        bot.pending.retain(|_, pending| pending.expires_at > now);
        bot.pending.insert(
            member.id,
            PendingLink {
                handle: handle.to_owned(),
                challenge: challenge.clone(),
                expires_at: now + TELEGRAM_CHALLENGE_TTL,
            },
        );
        Ok(Some(challenge))
    }

    pub(crate) async fn telegram_link_status(
        &self,
        app: AppPublicId,
        member: &AppMemberRow,
        now: Instant,
    ) -> TelegramLinkStatus {
        let links = self.telegram_links.lock().await;
        if let Some(handle) = self.verified_telegram_handle(app, member, &links) {
            return TelegramLinkStatus {
                handle: Some(handle),
                state: "verified",
            };
        }
        if let Some(pending) = links
            .bots
            .get(&app)
            .filter(|bot| self.bot_is_configured(app, bot))
            .and_then(|bot| bot.pending.get(&member.id))
        {
            return TelegramLinkStatus {
                handle: Some(pending.handle.clone()),
                state: if now < pending.expires_at {
                    "pending"
                } else {
                    "unverified"
                },
            };
        }
        TelegramLinkStatus {
            handle: self
                .support_info_table
                .select(member.user_pub_id)
                .and_then(|row| normalize_telegram_handle(&row.tg_handle)),
            state: "unverified",
        }
    }

    pub(crate) async fn redeem_telegram_start(
        &self,
        app: AppPublicId,
        generation: &Arc<()>,
        message: &Message,
    ) -> eyre::Result<bool> {
        self.redeem_telegram_start_with_barrier(app, generation, message, async {
            self.telegram_link_table.wait_for_ops().await?;
            Ok(())
        })
        .await
    }

    pub(in crate::service) async fn redeem_telegram_start_with_barrier(
        &self,
        app: AppPublicId,
        generation: &Arc<()>,
        message: &Message,
        barrier: impl Future<Output = eyre::Result<()>>,
    ) -> eyre::Result<bool> {
        let Some(sender) = private_telegram_sender(message) else {
            return Ok(false);
        };
        let Some(handle) = sender
            .username
            .as_deref()
            .and_then(normalize_telegram_handle)
        else {
            return Ok(false);
        };
        let Some(text) = message.text.as_deref() else {
            return Ok(false);
        };
        let mut words = text.split_whitespace();
        let (Some("/start"), Some(code), None) = (words.next(), words.next(), words.next()) else {
            return Ok(false);
        };
        if code.len() != 32 || !code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Ok(false);
        }
        let mut links = self.telegram_links.lock().await;
        let TelegramLinks {
            bots,
            uncommitted_members,
        } = &mut *links;
        let Some(bot) = bots.get_mut(&app).filter(|bot| {
            Arc::ptr_eq(&bot.generation, generation) && self.bot_is_configured(app, bot)
        }) else {
            return Ok(false);
        };
        let now = Instant::now();
        let members = self.list_members(app)?;
        let Some(member) = members
            .iter()
            .filter(|member| can_link(member))
            .find(|member| {
                bot.pending.get(&member.id).is_some_and(|pending| {
                    pending.expires_at > now
                        && pending.handle == handle
                        && pending.challenge == code
                })
            })
        else {
            return Ok(false);
        };
        // A quarantined row may already have reached disk. Do not bind the
        // same chat to a second member until that first row is removed.
        if members.iter().any(|row| {
            self.stored_telegram_link(row, &bot.token_digest)
                .is_some_and(|link| link.chat_id == sender.id)
        }) {
            return Ok(false);
        }

        // Consume before persisting. Failed writes require a new challenge.
        bot.pending.remove(&member.id);
        uncommitted_members.insert(member.id);
        self.telegram_link_table
            .insert(TelegramLinkRow {
                member_id: member.id,
                app_public_id: member.app_public_id,
                user_pub_id: member.user_pub_id,
                member_created_at: member.created_at,
                member_role: member.role,
                bot_token_digest: bot.token_digest.clone(),
                proof_id: PackedNanoId::pack(&NanoId::new())?,
                handle,
                chat_id: sender.id,
            })
            .await?;
        barrier.await?;
        uncommitted_members.remove(&member.id);
        Ok(true)
    }

    // Call while holding telegram_links so status cannot observe a partial edit.
    pub(super) fn verified_telegram_handle(
        &self,
        app: AppPublicId,
        member: &AppMemberRow,
        links: &TelegramLinks,
    ) -> Option<String> {
        let live = self.app_member_table.select(member.id)?;
        if live.app_public_id != app.pack().ok()? || live.user_pub_id != member.user_pub_id {
            return None;
        }
        let digest = self.configured_bot_digest(app)?;
        self.valid_telegram_link(&live, &digest, &links.uncommitted_members)
            .map(|link| link.handle)
    }

    pub(crate) async fn enabled_telegram_recipients(
        &self,
        app: AppPublicId,
        generation: &Arc<()>,
    ) -> eyre::Result<Vec<TelegramRecipient>> {
        let links = self.telegram_links.lock().await;
        let Some(bot) = links.bots.get(&app).filter(|bot| {
            Arc::ptr_eq(&bot.generation, generation) && self.bot_is_configured(app, bot)
        }) else {
            return Ok(Vec::new());
        };
        Ok(self
            .list_members(app)?
            .into_iter()
            .filter(|member| member.is_support_enabled)
            .filter_map(|member| {
                self.valid_telegram_link(&member, &bot.token_digest, &links.uncommitted_members)
            })
            .map(|link| TelegramRecipient {
                member_id: link.member_id,
                chat_id: link.chat_id,
                proof_id: link.proof_id,
            })
            .collect())
    }

    pub(crate) async fn can_deliver_telegram(
        &self,
        app: AppPublicId,
        generation: &Arc<()>,
        recipient: &TelegramRecipient,
    ) -> bool {
        let links = self.telegram_links.lock().await;
        let Some(bot) = links.bots.get(&app).filter(|bot| {
            Arc::ptr_eq(&bot.generation, generation) && self.bot_is_configured(app, bot)
        }) else {
            return false;
        };
        let Ok(app_id) = app.pack() else {
            return false;
        };
        self.app_member_table
            .select(recipient.member_id)
            .filter(|member| member.is_support_enabled && member.app_public_id == app_id)
            .and_then(|member| {
                self.valid_telegram_link(&member, &bot.token_digest, &links.uncommitted_members)
            })
            .is_some_and(|link| {
                link.proof_id == recipient.proof_id && link.chat_id == recipient.chat_id
            })
    }

    fn valid_telegram_link(
        &self,
        member: &AppMemberRow,
        digest: &str,
        uncommitted_members: &HashSet<u64>,
    ) -> Option<TelegramLinkRow> {
        if uncommitted_members.contains(&member.id) {
            return None;
        }
        self.stored_telegram_link(member, digest)
    }

    fn stored_telegram_link(&self, member: &AppMemberRow, digest: &str) -> Option<TelegramLinkRow> {
        self.telegram_link_table.select(member.id).filter(|link| {
            can_link(member)
                && link.app_public_id == member.app_public_id
                && link.user_pub_id == member.user_pub_id
                && link.member_created_at == member.created_at
                && link.member_role == member.role
                && link.bot_token_digest == digest
        })
    }

    fn configured_bot_digest(&self, app: AppPublicId) -> Option<String> {
        self.get_app(app)
            .ok()
            .flatten()
            .filter(|config| config.active)
            .map(|config| token_digest(&config.tg_bot_token))
    }

    fn bot_is_configured(&self, app: AppPublicId, bot: &BotLinks) -> bool {
        self.configured_bot_digest(app)
            .is_some_and(|digest| digest == bot.token_digest)
    }
}
