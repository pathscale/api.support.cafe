use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;
use std::time::Instant;

use endpoint_libs::libs::toolbox::RequestContext;
use endpoint_libs::libs::ws::handler::RequestHandler;
use futures::{FutureExt, Stream};
use nago_telegram::{Chat, User};

use super::*;
use crate::codegen::model::{
    AppMemberRole, GetMyTgHandleRequest, ListAppsRequest, SetMyTgHandleRequest, UserRole,
};
use crate::config::DatabaseConfig;
use crate::db::schema::app_config::AppConfigRow;
use crate::db::schema::app_member::{AppMemberRow, membership_key};
use crate::db::schema::chat_session::ChatSessionRow;
use crate::db::schema::support_info::SupportInfoRow;
use crate::db::schema::user::UserRow;
use crate::db::tables::Tables;
use crate::handlers::app_admin::list_apps::MethodListApps;
use crate::handlers::support::get_my_tg_handle::MethodGetMyTgHandle;
use crate::handlers::support::set_my_tg_handle::MethodSetMyTgHandle;
use crate::handlers::utils::receiver::Receiver;
use crate::id_types::NanoId;
use crate::service::user_connection_registry::UserConnectionRegistry;

const LEGACY_CHAT: i64 = 999;
const OWNER_CHAT: i64 = 42;
const TOKEN_FIXTURE: &str = "fixture-never-a-bot-token";
const REPLACEMENT_TOKEN: &str = "replacement-never-a-bot-token";

#[test]
fn telegram_ownership_gates_live_routing() {
    let path = std::env::temp_dir().join(format!("telegram-ownership-{}", NanoId::new()));
    std::fs::create_dir(&path).unwrap();
    let config = DatabaseConfig { path: path.clone() };
    let app = AppPublicId::from(NanoId::new());
    let other_app = AppPublicId::from(NanoId::new());
    let session = SessionId::from(NanoId::new());
    let owner: UserPublicId = NanoId::new().into();
    let claimant: UserPublicId = NanoId::new().into();
    let outsider: UserPublicId = NanoId::new().into();
    // Run each WorkTable-heavy phase separately with its future on the heap.
    nagoya::block_on(Box::pin(seed_legacy_fixture(
        config.clone(),
        app,
        other_app,
        session,
        owner,
        claimant,
        outsider,
    )));
    let pending = nagoya::block_on(Box::pin(exercise_linking(
        config.clone(),
        app,
        other_app,
        session,
        owner,
        claimant,
        outsider,
    )));
    nagoya::block_on(Box::pin(exercise_reopen_and_revoke(
        config.clone(),
        app,
        other_app,
        session,
        owner,
        claimant,
        pending,
    )));
    nagoya::block_on(Box::pin(check_revocations_after_reopen(
        config, app, other_app, session, owner, claimant,
    )));
    std::fs::remove_dir_all(path).unwrap();
}

async fn seed_legacy_fixture(
    config: DatabaseConfig,
    app: AppPublicId,
    other_app: AppPublicId,
    session: SessionId,
    owner: UserPublicId,
    claimant: UserPublicId,
    outsider: UserPublicId,
) {
    let db = Box::pin(Tables::new(config)).await.unwrap();
    for id in [app, other_app] {
        db.app_config_table
            .insert(AppConfigRow {
                id: db.app_config_table.get_next_pk().into(),
                public_id: id.pack().unwrap(),
                tg_bot_token: TOKEN_FIXTURE.to_owned(),
                app_name: None,
                active: true,
                message_persistence_enabled: false,
                created_at: 0,
            })
            .await
            .unwrap();
    }
    for (user, role) in [(owner, UserRole::AppAdmin), (claimant, UserRole::Support)] {
        db.user_table
            .insert(UserRow {
                id: db.user_table.get_next_pk().into(),
                pub_id: user.pack().unwrap(),
                username: user.to_string(),
                role,
            })
            .await
            .unwrap();
    }
    for (desk, user, role) in [
        (app, owner, AppMemberRole::Owner),
        (app, claimant, AppMemberRole::Support),
        (other_app, owner, AppMemberRole::Owner),
    ] {
        let app_public_id = desk.pack().unwrap();
        let user_pub_id = user.pack().unwrap();
        db.app_member_table
            .insert(AppMemberRow {
                id: db.app_member_table.get_next_pk().into(),
                app_public_id,
                user_pub_id,
                membership_key: membership_key(app_public_id, user_pub_id),
                role,
                created_at: 0,
                is_support_enabled: true,
            })
            .await
            .unwrap();
    }
    // Legacy case collisions and previously routable IDs survive disk reopen.
    for (user, handle, chat_id) in [
        (owner, "@ALICE", OWNER_CHAT),
        (claimant, "@Alice", LEGACY_CHAT),
    ] {
        db.support_info_table
            .insert(SupportInfoRow {
                user_pub_id: user.pack().unwrap(),
                tg_handle: handle.to_owned(),
                chat_id: Some(chat_id),
            })
            .await
            .unwrap();
    }
    db.chat_session_table
        .insert(ChatSessionRow {
            id: db.chat_session_table.get_next_pk().into(),
            session_id: session.pack().unwrap(),
            app_public_id: app.pack().unwrap(),
            user_pub_id: outsider.pack().unwrap(),
            created_at: 0,
            closed_at: None,
        })
        .await
        .unwrap();
    db.wait_for_ops().await.unwrap();
}

async fn reopen(config: DatabaseConfig) -> (Tables, Arc<MessageStore>, Arc<AppService>, BotRouter) {
    let db = Box::pin(Tables::new(config)).await.unwrap();
    let messages = Arc::new(MessageStore::new(
        db.app_config_table.clone(),
        db.chat_session_table.clone(),
        db.support_message_table.clone(),
        db.support_memory_message_table.clone(),
        db.app_slot_table.clone(),
    ));
    let apps = Arc::new(AppService::new(
        db.app_config_table.clone(),
        db.app_member_table.clone(),
        db.support_info_table.clone(),
        db.user_table.clone(),
        messages.clone(),
        db.telegram_link_table.clone(),
    ));
    let router = BotRouter::new(
        db.chat_session_table.clone(),
        messages.clone(),
        apps.clone(),
    );
    (db, messages, apps, router)
}

async fn exercise_linking(
    config: DatabaseConfig,
    app: AppPublicId,
    other_app: AppPublicId,
    session: SessionId,
    owner: UserPublicId,
    claimant: UserPublicId,
    outsider: UserPublicId,
) -> String {
    let (db, messages, apps, router) = Box::pin(reopen(config)).await;
    let mut events = pin!(router.take_event_stream().unwrap());
    let (handler, mut outbox) = install_offline_bot(&router, app).await;
    let (other_bot, mut other_outbox) = install_offline_bot(&router, other_app).await;

    let registry = Arc::new(UserConnectionRegistry::new());
    registry.register(1, owner).await;
    registry.register(2, claimant).await;
    registry.register(3, outsider).await;
    let list = MethodListApps {
        app_service: apps.clone(),
        user_connection_registry: registry.clone(),
    };
    let get = MethodGetMyTgHandle {
        app_service: apps.clone(),
        user_connection_registry: registry.clone(),
    };
    let set = MethodSetMyTgHandle {
        app_service: apps.clone(),
        user_connection_registry: registry,
    };
    let request = |handle: &str| SetMyTgHandleRequest {
        app_public_id: app.into(),
        tg_handle: handle.to_owned(),
    };
    let read = || GetMyTgHandleRequest {
        app_public_id: app.into(),
    };
    let owner_ctx = || context(1, UserRole::AppAdmin);
    let support_apps = list
        .handle(context(2, UserRole::Support), ListAppsRequest {})
        .await
        .unwrap()
        .data;
    assert_eq!(support_apps.len(), 1);
    assert_eq!(AppPublicId::from(support_apps[0].app_public_id), app);
    assert!(support_apps[0].tg_bot_token.is_empty());
    let owner_apps = list
        .handle(owner_ctx(), ListAppsRequest {})
        .await
        .unwrap()
        .data;
    assert_eq!(owner_apps.len(), 2);
    assert!(
        owner_apps
            .iter()
            .all(|app| app.tg_bot_token == TOKEN_FIXTURE)
    );
    let old = get.handle(owner_ctx(), read()).await.unwrap();
    assert_eq!(old.tg_handle.as_deref(), Some("@alice"));
    assert_eq!(old.link_state, "unverified");
    assert!(apps.enable_support_user(app, owner).await.is_err());
    assert!(
        apps.list_members_with_support_info(app)
            .await
            .unwrap()
            .iter()
            .all(|m| m.tg_handle.is_none())
    );
    assert!(
        set.handle(context(1, UserRole::App), request("Alice"))
            .await
            .is_err()
    );
    assert!(
        set.handle(context(3, UserRole::Support), request("Alice"))
            .await
            .is_err()
    );
    assert!(
        set.handle(owner_ctx(), request("@https://t.me/Alice"))
            .await
            .is_err()
    );

    assert_start_rejected(&handler, &update(OWNER_CHAT, "Alice", "/start")).await;
    let legacy_reply = reply(LEGACY_CHAT, "Alice", session);
    let owner_reply = reply(OWNER_CHAT, "aLiCe", session);
    assert_eq!(
        process(&handler, &legacy_reply).await,
        Some("Support access is disabled")
    );
    assert_eq!(
        process(&handler, &owner_reply).await,
        Some("Support access is disabled")
    );
    assert!(messages.list_messages(session).await.unwrap().is_empty());
    assert!(events.next().now_or_never().is_none());
    outgoing(events.as_mut(), &router, app, session, owner).await;
    assert!(outbox.next().now_or_never().is_none());

    // A failed durability barrier can leave a real WorkTable row readable.
    let member = apps.telegram_member(app, owner).unwrap().unwrap();
    let failed_code = set
        .handle(owner_ctx(), request("Alice"))
        .await
        .unwrap()
        .challenge;
    let failed_start = start(OWNER_CHAT, "Alice", &failed_code);
    let failure = apps
        .redeem_telegram_start_with_barrier(app, &handler.generation, &failed_start, async {
            assert!(db.telegram_link_table.select(member.id).is_some());
            eyre::bail!("fixture durability failure");
        })
        .await
        .unwrap_err();
    assert_eq!(failure.to_string(), "fixture durability failure");
    assert_eq!(
        get.handle(owner_ctx(), read()).await.unwrap().link_state,
        "unverified"
    );
    assert!(apps.enable_support_user(app, owner).await.is_err());
    assert_eq!(
        process(&handler, &owner_reply).await,
        Some("Support access is disabled")
    );
    outgoing(events.as_mut(), &router, app, session, owner).await;
    assert!(outbox.next().now_or_never().is_none());
    assert_start_rejected(&handler, &failed_start).await;

    // A claimant cannot reserve a canonical username against another member.
    let claimant_code = set
        .handle(context(2, UserRole::Support), request("alice"))
        .await
        .unwrap()
        .challenge;
    assert_start_rejected(&handler, &start(OWNER_CHAT, "Alice", &claimant_code)).await;
    let expired = apps
        .issue_telegram_link(&member, "@alice", Instant::now() - Duration::from_secs(601))
        .await
        .unwrap()
        .unwrap();
    assert_start_rejected(&handler, &start(OWNER_CHAT, "Alice", &expired)).await;
    assert_eq!(
        get.handle(owner_ctx(), read()).await.unwrap().link_state,
        "unverified"
    );
    let first = set
        .handle(owner_ctx(), request("Alice"))
        .await
        .unwrap()
        .challenge;
    let code = set
        .handle(owner_ctx(), request(" @aLiCe "))
        .await
        .unwrap()
        .challenge;
    assert_eq!(
        get.handle(owner_ctx(), read()).await.unwrap().link_state,
        "pending"
    );
    assert!(apps.enable_support_user(app, owner).await.is_err());
    assert_start_rejected(&handler, &start(OWNER_CHAT, "Alice", &first)).await;
    let mut wrong_handle = start(LEGACY_CHAT, "Mallory", &code);
    wrong_handle.chat.username = Some("Alice".into());
    assert_start_rejected(&handler, &wrong_handle).await;
    let good = start(OWNER_CHAT, "ALICE", &code);
    let mut group = good.clone();
    group.chat.kind = "group".into();
    assert_eq!(process(&handler, &group).await, None);
    let mut mismatched_sender = good.clone();
    mismatched_sender.from.as_mut().unwrap().id = LEGACY_CHAT;
    assert_eq!(process(&handler, &mismatched_sender).await, None);
    let mut bot_sender = good.clone();
    bot_sender.from.as_mut().unwrap().is_bot = true;
    assert_eq!(process(&handler, &bot_sender).await, None);
    assert_start_rejected(&other_bot, &good).await;
    assert_eq!(
        process(&handler, &owner_reply).await,
        Some("Support access is disabled")
    );
    outgoing(events.as_mut(), &router, app, session, owner).await;
    assert!(outbox.next().now_or_never().is_none());

    assert_eq!(
        process(&handler, &good).await,
        Some("Telegram verified for this support desk.")
    );
    assert_start_rejected(&handler, &good).await;
    assert_start_rejected(&handler, &start(OWNER_CHAT, "ALICE", &claimant_code)).await;
    router
        .edit_app(
            app,
            AppUpdate {
                tg_bot_token: Some(TOKEN_FIXTURE.into()),
                app_name: Some("Renamed desk".into()),
                active: Some(true),
                ..AppUpdate::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        get.handle(owner_ctx(), read()).await.unwrap().link_state,
        "verified"
    );
    assert_eq!(
        apps.list_members_with_support_info(app)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|m| m.tg_handle)
            .collect::<Vec<_>>(),
        ["@alice"]
    );
    outgoing(events.as_mut(), &router, app, session, owner).await;
    let visitor_copy = outbox.next().now_or_never().flatten().unwrap();
    let staff_copy = outbox.next().now_or_never().flatten().unwrap();
    assert_eq!(visitor_copy.0.chat_id, OWNER_CHAT);
    assert!(handler.admit_delivery(&visitor_copy.0).await);
    assert!(visitor_copy.1.ends_with("visitor question"));
    assert_eq!(staff_copy.0.chat_id, OWNER_CHAT);
    assert!(handler.admit_delivery(&staff_copy.0).await);
    assert!(staff_copy.1.ends_with("dashboard answer"));
    assert!(outbox.next().now_or_never().is_none());
    outgoing(
        events.as_mut(),
        &router,
        other_app,
        SessionId::from(NanoId::new()),
        owner,
    )
    .await;
    assert!(other_outbox.next().now_or_never().is_none());
    assert_eq!(
        process(&other_bot, &owner_reply).await,
        Some("Support access is disabled")
    );
    expect_telegram_reply(events.as_mut(), &handler, &owner_reply, (app, session)).await;
    assert!(
        messages
            .list_messages(session)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content == "telegram answer" && m.incoming)
    );
    apps.disable_support_user(app, owner).await.unwrap();
    assert!(!handler.admit_delivery(&visitor_copy.0).await);
    assert_eq!(
        process(&handler, &owner_reply).await,
        Some("Support access is disabled")
    );
    outgoing(events.as_mut(), &router, app, session, owner).await;
    assert!(outbox.next().now_or_never().is_none());
    apps.enable_support_user(app, owner).await.unwrap();

    // Reissue revokes immediately; two simultaneous polls cannot both redeem.
    let relink = set
        .handle(owner_ctx(), request("Alice"))
        .await
        .unwrap()
        .challenge;
    assert!(!handler.admit_delivery(&staff_copy.0).await);
    outgoing(events.as_mut(), &router, app, session, owner).await;
    assert!(outbox.next().now_or_never().is_none());
    let next = start(OWNER_CHAT, "Alice", &relink);
    let redeem = || {
        nagoya::block_on(Box::pin(apps.redeem_telegram_start(
            app,
            &handler.generation,
            &next,
        )))
        .unwrap()
    };
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(&redeem);
        let b = scope.spawn(&redeem);
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_ne!(a, b);
    assert!(!handler.admit_delivery(&staff_copy.0).await);

    // Bot replacement invalidates old handlers, links and pending codes.
    let old_code = set
        .handle(owner_ctx(), request("Alice"))
        .await
        .unwrap()
        .challenge;
    router
        .edit_app(
            app,
            AppUpdate {
                tg_bot_token: Some(REPLACEMENT_TOKEN.into()),
                active: Some(false),
                ..AppUpdate::default()
            },
        )
        .await
        .unwrap();
    // A delayed bootstrap call cannot reactivate this app.
    router
        .register_bot(app, TOKEN_FIXTURE.into())
        .await
        .unwrap();
    assert!(router.get_status(app).await.is_none());
    assert!(set.handle(owner_ctx(), request("Alice")).await.is_err());
    apps.edit_app(
        app,
        AppUpdate {
            active: Some(true),
            ..AppUpdate::default()
        },
    )
    .await
    .unwrap();
    router
        .register_bot(app, TOKEN_FIXTURE.into())
        .await
        .unwrap();
    assert!(router.get_status(app).await.is_none());
    let (replacement, mut replacement_outbox) = install_offline_bot(&router, app).await;
    assert_start_rejected(&replacement, &start(OWNER_CHAT, "Alice", &old_code)).await;
    let fresh_code = set
        .handle(owner_ctx(), request("Alice"))
        .await
        .unwrap()
        .challenge;
    let fresh = start(OWNER_CHAT, "Alice", &fresh_code);
    assert_start_rejected(&handler, &fresh).await;
    assert_eq!(
        process(&replacement, &fresh).await,
        Some("Telegram verified for this support desk.")
    );
    assert_eq!(
        process(&handler, &owner_reply).await,
        Some("Support access is disabled")
    );

    // Removing and re-adding a user never restores a link from the old row.
    let claimant_new = set
        .handle(context(2, UserRole::Support), request("Bob"))
        .await
        .unwrap()
        .challenge;
    assert_eq!(
        process(&replacement, &start(LEGACY_CHAT, "Bob", &claimant_new)).await,
        Some("Telegram verified for this support desk.")
    );
    apps.remove_member(app, claimant, AppMemberRole::Support)
        .await
        .unwrap();
    apps.add_member(app, claimant, AppMemberRole::Support)
        .await
        .unwrap();
    assert!(apps.enable_support_user(app, claimant).await.is_err());
    assert_eq!(
        process(&replacement, &reply(LEGACY_CHAT, "Bob", session)).await,
        Some("Support access is disabled")
    );
    outgoing(events.as_mut(), &router, app, session, owner).await;
    let (copy, _) = replacement_outbox.next().now_or_never().flatten().unwrap();
    assert_eq!(copy.chat_id, OWNER_CHAT);
    assert!(replacement.admit_delivery(&copy).await);
    assert_eq!(
        replacement_outbox
            .next()
            .now_or_never()
            .flatten()
            .unwrap()
            .0
            .chat_id,
        OWNER_CHAT
    );
    assert!(replacement_outbox.next().now_or_never().is_none());

    let claimant_code = set
        .handle(context(2, UserRole::Support), request("Bob"))
        .await
        .unwrap()
        .challenge;
    assert_eq!(
        process(&replacement, &start(LEGACY_CHAT, "Bob", &claimant_code)).await,
        Some("Telegram verified for this support desk.")
    );
    apps.enable_support_user(app, claimant).await.unwrap();

    let other_owner = apps.telegram_member(other_app, owner).unwrap().unwrap();
    let other_code = apps
        .issue_telegram_link(&other_owner, "@alice", Instant::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        process(&other_bot, &start(OWNER_CHAT, "Alice", &other_code)).await,
        Some("Telegram verified for this support desk.")
    );
    let other_claimant = apps
        .add_member(other_app, claimant, AppMemberRole::Support)
        .await
        .unwrap();
    let pending = apps
        .issue_telegram_link(&other_claimant, "@bob", Instant::now())
        .await
        .unwrap()
        .unwrap();

    // The v2 row is a display hint only. Neither issuance nor redemption writes it.
    let row = db.support_info_table.select(owner.pack().unwrap()).unwrap();
    assert_eq!(row.chat_id, Some(OWNER_CHAT));
    assert_eq!(row.tg_handle, "@ALICE");
    router.shutdown().await;
    db.wait_for_ops().await.unwrap();
    pending
}

async fn exercise_reopen_and_revoke(
    config: DatabaseConfig,
    app: AppPublicId,
    other_app: AppPublicId,
    session: SessionId,
    owner: UserPublicId,
    claimant: UserPublicId,
    pending: String,
) {
    // No previous Tables, AppService or poller instances are still open.
    let (db, _, apps, router) = Box::pin(reopen(config)).await;
    let mut events = pin!(router.take_event_stream().unwrap());
    let (bot, mut outbox) = install_offline_bot(&router, app).await;
    let (other_bot, mut other_outbox) = install_offline_bot(&router, other_app).await;
    assert_start_rejected(&other_bot, &start(LEGACY_CHAT, "Bob", &pending)).await;
    expect_telegram_reply(
        events.as_mut(),
        &bot,
        &reply(OWNER_CHAT, "Alice", session),
        (app, session),
    )
    .await;
    expect_telegram_reply(
        events.as_mut(),
        &bot,
        &reply(LEGACY_CHAT, "Bob", session),
        (app, session),
    )
    .await;
    outgoing(events.as_mut(), &router, app, session, owner).await;
    let mut chats = Vec::new();
    while let Some((recipient, _)) = outbox.next().now_or_never().flatten() {
        assert!(bot.admit_delivery(&recipient).await);
        chats.push(recipient.chat_id);
    }
    chats.sort_unstable();
    assert_eq!(chats, [OWNER_CHAT, OWNER_CHAT, LEGACY_CHAT, LEGACY_CHAT]);
    outgoing(events.as_mut(), &router, other_app, session, owner).await;
    let (recipient, _) = other_outbox.next().now_or_never().flatten().unwrap();
    assert_eq!(recipient.chat_id, OWNER_CHAT);
    assert!(other_bot.admit_delivery(&recipient).await);

    apps.edit_app(
        other_app,
        AppUpdate {
            tg_bot_token: Some(REPLACEMENT_TOKEN.into()),
            ..AppUpdate::default()
        },
    )
    .await
    .unwrap();
    assert!(!other_bot.admit_delivery(&recipient).await);
    apps.remove_member(app, claimant, AppMemberRole::Support)
        .await
        .unwrap();
    router.shutdown().await;
    db.wait_for_ops().await.unwrap();
}

async fn check_revocations_after_reopen(
    config: DatabaseConfig,
    app: AppPublicId,
    other_app: AppPublicId,
    session: SessionId,
    owner: UserPublicId,
    claimant: UserPublicId,
) {
    let (db, _, apps, router) = Box::pin(reopen(config)).await;
    let mut events = pin!(router.take_event_stream().unwrap());
    let (bot, mut outbox) = install_offline_bot(&router, app).await;
    let (other_bot, mut other_outbox) = install_offline_bot(&router, other_app).await;
    apps.add_member(app, claimant, AppMemberRole::Support)
        .await
        .unwrap();
    assert!(apps.enable_support_user(app, claimant).await.is_err());
    assert_eq!(
        process(&bot, &reply(LEGACY_CHAT, "Bob", session)).await,
        Some("Support access is disabled")
    );
    assert_eq!(
        process(&other_bot, &reply(OWNER_CHAT, "Alice", session)).await,
        Some("Support access is disabled")
    );
    outgoing(events.as_mut(), &router, app, session, owner).await;
    for _ in 0..2 {
        let (recipient, _) = outbox.next().now_or_never().flatten().unwrap();
        assert_eq!(recipient.chat_id, OWNER_CHAT);
        assert!(bot.admit_delivery(&recipient).await);
    }
    assert!(outbox.next().now_or_never().is_none());
    outgoing(events.as_mut(), &router, other_app, session, owner).await;
    assert!(other_outbox.next().now_or_never().is_none());

    // Restoring the original token cannot revive an invalidated proof.
    apps.edit_app(
        other_app,
        AppUpdate {
            tg_bot_token: Some(TOKEN_FIXTURE.into()),
            ..AppUpdate::default()
        },
    )
    .await
    .unwrap();
    let (other_bot, _) = install_offline_bot(&router, other_app).await;
    assert_eq!(
        process(&other_bot, &reply(OWNER_CHAT, "Alice", session)).await,
        Some("Support access is disabled")
    );
    router.shutdown().await;
    db.wait_for_ops().await.unwrap();
}

fn context(connection_id: u32, role: UserRole) -> RequestContext {
    let mut ctx = RequestContext::empty();
    ctx.connection_id = connection_id;
    ctx.roles = Arc::new(vec![role as u32]);
    ctx
}

async fn install_offline_bot(
    router: &BotRouter,
    app_public_id: AppPublicId,
) -> (
    BotUpdateHandler,
    UnboundedReceiver<(TelegramRecipient, String)>,
) {
    let generation = Arc::new(());
    let (outbox, rx) = mpsc::unbounded();
    let token = router
        .app_service
        .get_app(app_public_id)
        .unwrap()
        .unwrap()
        .tg_bot_token;
    router
        .app_service
        .activate_telegram_bot(app_public_id, generation.clone(), &token)
        .await;
    router.bots.write().await.insert(
        app_public_id,
        BotInstance {
            generation: generation.clone(),
            outbox,
            stop: None,
            status: Arc::new(RwLock::new(BotStatus::Running)),
        },
    );
    (
        BotUpdateHandler {
            app_public_id,
            generation,
            app_service: router.app_service.clone(),
            chat_session_table: router.chat_session_table.clone(),
            message_store: router.message_store.clone(),
            event_tx: router.event_tx.clone(),
        },
        rx,
    )
}

async fn drive_events<T>(
    mut events: Pin<&mut SupportEventStream>,
    operation: impl Future<Output = T>,
) -> (T, Vec<RoutingMessage<SessionKey, ChatMessage>>) {
    let mut operation = pin!(operation);
    let mut received = Vec::new();
    // Drain the single slot even while a send is pending, including the last
    // event when the operation completes. Missing events fail assertions below.
    let result = poll_fn(|cx| {
        let result = operation.as_mut().poll(cx);
        while let Poll::Ready(Some(event)) = events.as_mut().poll_next(cx) {
            received.push(event);
        }
        result
    })
    .await;
    (result, received)
}

fn assert_events(
    received: &[RoutingMessage<SessionKey, ChatMessage>],
    key: SessionKey,
    contents: &[&str],
) {
    assert_eq!(received.len(), contents.len());
    for (event, expected) in received.iter().zip(contents) {
        assert!(matches!(&event.receiver, Receiver::Concrete(actual) if *actual == key));
        assert_eq!(event.payload.content, *expected);
    }
}

async fn outgoing(
    events: Pin<&mut SupportEventStream>,
    router: &BotRouter,
    app: AppPublicId,
    session: SessionId,
    staff: UserPublicId,
) {
    let ((), received) = drive_events(events, async {
        Box::pin(router.send_message(app, session, "visitor question".into(), "visitor".into()))
            .await
            .unwrap();
        Box::pin(router.send_support_reply(app, session, "dashboard answer".into(), staff))
            .await
            .unwrap();
    })
    .await;
    assert_events(
        &received,
        (app, session),
        &["visitor question", "dashboard answer"],
    );
}

async fn expect_telegram_reply(
    events: Pin<&mut SupportEventStream>,
    handler: &BotUpdateHandler,
    message: &Message,
    key: SessionKey,
) {
    let (notice, received) = drive_events(events, process(handler, message)).await;
    assert_eq!(notice, None);
    assert_events(&received, key, &["telegram answer"]);
}

fn update(chat: i64, username: &str, text: &str) -> Message {
    Message {
        message_id: 1,
        date: 0,
        chat: Chat {
            id: chat,
            kind: "private".into(),
            title: None,
            username: Some(username.into()),
        },
        from: Some(User {
            id: chat,
            is_bot: false,
            first_name: username.into(),
            username: Some(username.into()),
        }),
        text: Some(text.into()),
        caption: None,
        reply_to_message: None,
    }
}

fn start(chat: i64, username: &str, challenge: &str) -> Message {
    update(chat, username, &format!("/start {challenge}"))
}

fn reply(chat: i64, username: &str, session: SessionId) -> Message {
    let mut reply = update(chat, username, "telegram answer");
    reply.reply_to_message = Some(Box::new(update(
        chat,
        username,
        &format!("{session}\nfrom: visitor"),
    )));
    reply
}

async fn process(handler: &BotUpdateHandler, message: &Message) -> Option<&'static str> {
    Box::pin(handler.process(message)).await
}

async fn assert_start_rejected(handler: &BotUpdateHandler, msg: &Message) {
    assert_eq!(
        process(handler, msg).await,
        Some("Open the support dashboard for a current verification code.")
    );
}
