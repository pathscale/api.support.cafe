# api.support.cafe Review: Full

**Date:** 2026-07-27
**Scope:** whole repo. `src/**` (76 .rs files, 10,137 LOC incl. 2,235 generated), `migration/**`, `config/**` (RON schemas), `docs/**`, `AGENTS.md`, `CLAUDE.md`, `README.md`, `.github/workflows/ci.yml`, `Cargo.toml`/`Cargo.lock`, `.claude/**`, `fly.toml`, the helper scripts. Cross-checked against `endpoint-libs 2.0.0` and `honey_id-types 2.0.0` sources in the cargo registry.
**Commit:** `feef288` ("build: move to worktable 0.9.1"), working tree clean
**Reviewer slice:** full (sole reviewer for this repo; no sibling slices)

## Summary

- The code is **tidy and consistent** for its size: one clear layering (`handlers/` → `service/` → `db/`), no god files outside generated code, no `unsafe`, no SQL, no shell-outs, no hand-rolled crypto. The endpoint-libs 2.0 port and the MCP enablement both look correctly done. That is the good news, and it is genuinely good.
- The bad news is concentrated and serious. **Live-looking Tigris S3 credentials are committed at `migration/config.migrate.toml:7-8`** and have been in the history since `a9475fb`. **`AppConnect` grants `UserRole::App` to any anonymous socket that merely *declares* an app id and a victim's user id**, which is a full read/write IDOR over any user's support conversations; enabling MCP just published that surface to LLM agents too.
- The last commit bumped `worktable` from `0.9.0-beta0.2.3` to `0.9.1` with **no migration and no operational note**, which is exactly what `docs/worktable-schema-and-deploy.md:28-30,47-48` says must never happen. Separately, the `migration/` crate only covers **4 of the 6 persisted tables** (`ChatSession` and `User` are missing), so the documented escape hatch would silently drop data if used.
- As a **reference implementation** the highest-leverage defect is not a bug: 46 `.internal()` call sites blanket-convert *authorization denials* into internal 500/`-32603` errors. endpoint-libs 2.0's whole typed-public-error feature exists to avoid this, one handler (`list_messages.rs:51`) does it right, and 22 do it wrong. Whatever the next service copies from here, it will copy that.
- Top 3 to do, in order: **(1)** rotate the Tigris keys and scrub the file from history; **(2)** decide the `AppConnect` identity model (app-signed user tokens) or gate it behind a per-app shared secret; **(3)** land a `handler_ctx` helper + `CustomError` mapping so the 20x auth preamble and the 46 `.internal()`s collapse into one correct shape.
- Zero tests exist (`rg '#\[test\]' src/ migration/` → 0) while `AGENTS.md:31` lists `cargo test` in the build loop. `cargo audit` reports 6 advisories, all transitive through the S3 stack.

## Findings

### [SEV-1] Live Tigris S3 credentials committed to a public repo

- **ID:** `cafe-full-01`
- **Severity:** Critical
- **Category:** Security
- **Confidence:** High (the values are in the tree; whether they are still *valid* needs a human to check Tigris)
- **Location:** `migration/config.migrate.toml:7-8`; introduced in `a9475fb`, still present at `feef288`
- **What:** The migration config carries a real-shaped Tigris access key (`tid_bwKSOzpIY…`) and secret key (`tsec_9mk_7Npd2QN7…`) for bucket `support-cafe-master-tigris`, prefix `db`. `git log -- migration/config.migrate.toml` shows two commits; the file has never been redacted. `.gitignore:9` ignores `.env` but nothing ignores this path.
- **Why it matters:** That bucket is the durable store for every persisted WorkTable (`src/db/tables.rs:84-137`): all apps, all Telegram bot tokens (`AppConfig.tg_bot_token`), all memberships, all support messages, all users. Anyone with the repo gets read *and write* on production data. Write access is worse than read here: the server loads tables from S3 at boot (`Tables::new` → `<$Table>::load(engine)`), so a poisoned snapshot is loaded and trusted on the next machine restart. Rotating the keys alone does not close the history.
- **Fix:** (1) Rotate the Tigris key pair now. (2) Replace lines 7-8 with empty values and rely on the existing `SUPPORT_CAFE_MIGRATE__S3__ACCESS_KEY` env override (`migration/src/config.rs:36-41`): the loader already supports it, the file only needs to stop carrying secrets. (3) Scrub from history (`git filter-repo --path migration/config.migrate.toml`) and force-push, coordinating with everyone who has a clone; note `AGENTS.md:104-105` forbids force-pushing the default branch, so this needs a human decision. (4) Add `*.migrate.toml` or a `config.migrate.example.toml` convention. Also rotate every Telegram bot token that was in the bucket, since they were readable.
- **Effort:** M (rotation is S; history scrub plus coordination is M)
- **Blast radius:** one file, plus every clone of the repo and every bot token in the bucket.

### [SEV-2] `AppConnect` is trust-on-declaration: any anonymous socket can read and write any user's support conversations

- **ID:** `cafe-full-02`
- **Severity:** Critical
- **Category:** Security
- **Confidence:** High
- **Location:** `src/handlers/app/auth.rs:32-46`; consumed at `src/handlers/app/list_chat_sessions.rs:32-45`, `list_messages.rs:34-55`, `send_message.rs:33-46`, `close_chat_session.rs:32-43`, `create_chat_session.rs:31-43`; check itself at `src/service/session.rs:172-207`
- **What:** `MethodAppConnect::auth` takes `app_public_id` and `user_public_id` straight off the wire, registers both in the two connection registries, and calls `conn.set_roles(vec![UserRole::App as u32])`. There is no signature, no shared secret, no existence check on either id, and no `Result::Err` path at all, so the closure cannot fail. Every downstream authorization decision then compares against those *self-declared* values: `verify_session_access` (`session.rs:191`) only asserts `session.user_pub_id == packed_user_id`, where `packed_user_id` is whatever the caller claimed at handshake.
- **Why it matters:** Attack: open a WebSocket with `Sec-WebSocket-Protocol: 0appconnect,1<app_id>,2<victim_user_pub_id>`. You now hold `UserRole::App`. Call `list_chat_sessions` → every session the victim has with that app. Call `list_messages` on each → the full transcript. Call `send_message` → impersonate the victim to their own support desk, and the message is relayed to real Telegram staff as if from them. Call `close_chat_session` → denial of service. The only secret required is a 16-char public id, and public ids are handed out by `ListAppMembers` (to any app member) and `GetUsers` (to platform admins), and are embedded in any widget that ever connected. `app_public_id` is not secret either. `docs/appApi_mcp_tools.json` now publishes this tool surface to MCP agents, and `roles_allowed` in `endpoint-libs-2.0.0/src/libs/ws/mcp.rs:399-404` lets an `App`-role connection call all six tools, so an MCP client reaches it with no credential at all.
- **Fix:** Needs design discussion, not a patch. The minimum viable version: give each app a signing secret at `CreateApp` time, have the app's backend mint a short-lived JWT/HMAC over `(app_public_id, user_public_id, exp)`, and have `MethodAppConnect` verify it before `set_roles`. Interim mitigations that are cheap and worth doing regardless: (a) reject `app_public_id`s that do not exist (`AppService::exists` already exists at `src/service/app.rs:199-206` and is currently dead code); (b) reject `user_public_id`s with no `User` row; (c) require `active == true`. None of these fix the model, they only raise the cost from "guess a public id" to "know a real public id".
- **Effort:** L for the real fix; S for (a)-(c).
- **Blast radius:** `auth.rs`, the app RON schema (`020_app_connect.ron` gains a token field), and every widget client. Breaking API change for widgets.

### [SEV-3] `worktable` 0.9.1 bump shipped with no migration and no note about beta-written S3 snapshots

- **ID:** `cafe-full-03`
- **Severity:** High
- **Category:** Correctness / Docs
- **Confidence:** High that the process rule was skipped; Medium on whether the on-disk format actually changed (needs a WorkTable maintainer to confirm 0.9.0-beta0.2.3 → 0.9.1 snapshot compatibility)
- **Location:** commit `feef288` (`Cargo.toml:37`, `migration/Cargo.toml:12`); the rule at `docs/worktable-schema-and-deploy.md:28-30` and `:47-48`; the "Precedent" log at `docs/worktable-schema-and-deploy.md:50-59`
- **What:** `docs/worktable-schema-and-deploy.md:28-30` names "bumping the `worktable` crate version" as a schema change, because it "can change the on-disk format with no source edit at all", and `:47-48` says "If (2) shows **any** movement, a migration script is required." Commit `feef288` moved `worktable` from `0.9.0-beta0.2.3` to `0.9.1` in both crates and shipped **only** `Cargo.toml`/`Cargo.lock` changes (`git show --stat feef288`: 3 files). No migration script, no entry appended to the Precedent section, no operational runbook step. The commit message says "fresh start plus reload-from-disk run with no errors", which validates *locally written* 0.9.1 files, not the beta-written snapshots sitting in `support-cafe-master-tigris`.
- **Why it matters:** 0.9.1 carries the duplicate-key secondary-index reload fix (WorkTable#175). The corollary the fix implies is that **snapshots written by the beta may contain incomplete non-unique index nodes and a table-of-contents the new loader reads differently**. `src/db/tables.rs:112-123` loads every persisted table straight from the S3-backed engine at boot with `?`, so a format mismatch is a boot failure on the next Fly deploy, and a *silent partial* index reload is worse: `select_by_app_public_id` (used by `list_members`, `enabled_support_chat_ids`, `is_chat_enabled_for_app`) would quietly return fewer rows, so support staff stop receiving messages with no error anywhere. There is nothing in the repo telling an operator to do a rewrite pass.
- **Fix:** Two things. (1) Add a short section to `docs/worktable-schema-and-deploy.md` recording this bump as a Precedent entry with the actual verdict, and if a rewrite pass is needed, the exact command (the `migration/` crate already does download → re-persist, which is the rewrite; see `cafe-full-04` first, it is currently incomplete). (2) Before the next deploy, run the migration binary against a *copy* of the production prefix and diff row counts per table against the running instance. Mechanical once someone confirms the format question upstream.
- **Effort:** S for the doc; M for the verification pass.
- **Blast radius:** deploy procedure; no source change.

### [SEV-4] The migration crate silently skips 2 of the 6 persisted tables

- **ID:** `cafe-full-04`
- **Severity:** High
- **Category:** Correctness
- **Confidence:** High
- **Location:** `migration/src/main.rs:18-22` (module list), `:47-105` (S3 download), `:109-112` (migrate calls); versus `src/db/tables.rs:37-45`
- **What:** `Tables` holds 7 tables, 6 with `persist: true`: `AppConfig`, `AppMember`, `ChatSession`, `SupportMessage`, `SupportInfo`, `User`. The migration binary downloads and migrates exactly four: `AppConfig`, `AppMember`, `SupportInfo`, `SupportMessage`. **`ChatSession` and `User` are not downloaded, not migrated, and not copied.** There is no warning; `run()` prints "Migration complete" and exits 0.
- **Why it matters:** `docs/worktable-schema-and-deploy.md:19-20` presents "write the WT data migration script and ship it with the change" as the safe path, and this crate is the only implementation of that path. If anyone runs it as the rewrite pass for `cafe-full-03`, the output directory has no users and no chat sessions. On restart the server would boot with an empty `User` table, which means every existing user fails `Init` (`src/handlers/auth_api.rs:104-107` errors "User not found") and `bootstrap_admin_user` bails, so the server refuses to start (`src/app.rs:196-200`). Total outage plus loss of every session-to-user binding, which is what makes the retained `SupportMessage` rows readable.
- **Fix:** Mechanical. Add `migration/src/chat_session.rs` and `migration/src/user.rs` following the existing shape, plus the two download blocks and two `migrate_*` calls. `ChatSession` is `version: 1` and `User` has no `version:` at all (`src/db/schema/user.rs:11-32`), so neither has a v1→v2 `Migration` impl to write; they only need the download-and-copy path (the `Unsupported version` arm at `main.rs:136-139`). Better still, drive the whole thing from one list so a new table cannot be forgotten: a `const MIGRATED_TABLES` array plus a macro, mirroring `disk_load!` in `src/db/tables.rs:52-62`.
- **Effort:** M
- **Blast radius:** `migration/` only.

### [SEV-5] Connection registries never unregister, and connection ids wrap every ~72 minutes

- **ID:** `cafe-full-05`
- **Severity:** High
- **Category:** Correctness / Security
- **Confidence:** High on the leak and the id formula; Medium on the practical impact of a collision (see below)
- **Location:** `src/service/app_connection_registry.rs:25-27`, `src/service/user_connection_registry.rs:24-26` (both `unregister` fns, zero call sites; `rg 'registry.*unregister|\.unregister\(' src/` finds only `unregister_bot`); id source at `endpoint-libs-2.0.0/src/libs/utils.rs:7-9` and `server.rs:244`
- **What:** Both registries are `RwLock<HashMap<u32, Id>>` with `register`/`unregister`/`get`. `register` is called from `auth.rs:37-38` and `auth_api.rs:113`. **`unregister` is never called by anything.** endpoint-libs 2.0 exposes no connection-close callback (`rg 'on_disconnect|fn.*disconnect' endpoint-libs-2.0.0/src/libs/ws/` finds nothing), so there is no hook to call it from; the `// TODO: remove when elibs will be capable to have xustome context.` comment at both files line 8-9 is the acknowledgment. Meanwhile `get_conn_id()` is `chrono::Utc::now().timestamp_micros() as u32`: a truncating cast, so ids wrap every 2^32 µs ≈ **71.6 minutes** and are not unique across that window.
- **Why it matters:** Two effects. (a) **Unbounded growth**: one 16-byte-ish entry per connection ever accepted, forever, on a 256 MB VM (`fly.toml:33`). Any bot that opens and closes sockets in a loop is a slow memory-exhaustion DoS with no rate limit in front of it (see `cafe-full-07`). (b) **Identity confusion on wrap**: an entry left by a dead connection is inherited by a new connection with the same truncated id. I traced the exploit paths and they land on *denial*, not disclosure: a stale `app_connection_registry` entry makes `list_messages.rs:49-55` reject a legitimate session and makes `list_chat_sessions.rs:41-45` filter the user's own sessions to the wrong app; `create_chat_session` needs `UserRole::App`, which only `AppConnect` grants and which re-registers both maps. So I would not call this a live privilege escalation today, but it is one refactor away from being one, and the correctness bug is real now.
- **Fix:** Short term, two independent changes: (1) bound the maps by evicting on `register` when a `get` for the same id already exists, and/or store `(id, created_at)` and sweep entries older than the wrap window from the existing hourly purge task (`src/service/message_store.rs:135-151` already runs one); (2) file an endpoint-libs issue for a connection-close callback *and* for a non-truncating, non-reused `ConnectionId` (an `AtomicU64` counter is the obvious fix, and it also removes the wrap). Long term, this whole pair of registries is the workaround the TODOs describe: endpoint-libs should let a `SubAuthController` attach typed context to `WsConnection`, and then both files delete.
- **Effort:** S for the bound; the endpoint-libs fix is upstream.
- **Blast radius:** two service files; the upstream ask touches every endpoint-libs consumer.

### [SEV-6] Authorization denials are reported as internal server errors (46 `.internal()` sites)

- **ID:** `cafe-full-06`
- **Severity:** High
- **Category:** Design
- **Confidence:** High
- **Location:** the 9 `ensure_*` sites: `admin/delete_app.rs:35-37`, `app_admin/add_app_member.rs:35-37`, `disable_message_persistence.rs:37-39`, `disable_support_user.rs:33-35`, `edit_app.rs:42-44`, `enable_message_persistence.rs:37-39`, `enable_support_user.rs:33-35`, `list_app_members.rs:33-35`, `set_app_member_role.rs:33-35`; the session-access sites: `app/list_messages.rs:43-46`, `subscribe_events.rs:48-51`, `send_message.rs:42-46`, `close_chat_session.rs:41-44`. Counterexample done correctly: `app/list_messages.rs:50-54`. Total `.internal()` call sites: 46 across 23 files.
- **What:** `AppService::ensure_app_owner` / `ensure_app_admin_or_owner` / `ensure_app_member` return `eyre::Result<()>` and `bail!("User is not the app owner")` (`src/service/app/member.rs:217-266`). Every handler then writes `.internal()?`, which `HandlerResultExt` converts to `HandlerError::internal`, i.e. a 500 on the legacy protocol and `-32603` with a `logId` on MCP. Same for `verify_session_access`, whose "Session does not belong to this user" (`src/service/session.rs:209`) is the IDOR denial. Exactly one place in the repo builds a public error for an authorization decision: `list_messages.rs:51` returns `CustomError::new(EnumErrorCode::Forbidden)`.
- **Why it matters:** Three costs, and the third is the reason this is High rather than Medium. (1) Callers cannot distinguish "you are not allowed" from "the server is broken", so clients cannot render a sensible message and cannot decide whether to retry. (2) Every denied request is logged at internal-error severity with a log id, which turns routine permission checks into alert noise and buries real faults. (3) **This repo is what other services copy.** The single headline feature of the endpoint-libs 1.8/2.0 error refactor is typed public errors, `EnumErrorCode` is fully populated with `Forbidden = 100403` (`src/codegen/model.rs:869`), and the reference implementation uses it once out of 23 opportunities. Whatever the next backend copies, it copies this.
- **Fix:** Mechanical, and it composes with `cafe-full-09`. Give the authorization helpers a typed error instead of `eyre`:
  ```rust
  // src/service/app/member.rs
  pub fn ensure_app_owner(&self, app: AppPublicId, user: UserPublicId)
      -> Result<(), CustomError>
  {
      match self.get_member(app, user).internal()? {
          Some(m) if m.role == AppMemberRole::Owner => Ok(()),
          Some(_) => Err(CustomError::new(EnumErrorCode::Forbidden)
              .with_message("Caller is not the app owner")),
          None    => Err(CustomError::new(EnumErrorCode::Forbidden)
              .with_message("Caller is not a member of this app")),
      }
  }
  ```
  then the 9 handler sites become `self.app_service.ensure_app_owner(app, actor)?;` with no `.internal()`. Same treatment for `verify_session_access` → `Forbidden`, and for `session not found` → `NotFound`. Watch one thing while doing it: keep "session not found" and "session belongs to someone else" indistinguishable to the caller so the endpoint is not an existence oracle for session ids; return `NotFound` for both.
- **Effort:** M
- **Blast radius:** `service/app/member.rs`, `service/session.rs`, 13 handlers. Client-visible behaviour change (500 → 403), which is the point.

### [SEV-7] No size limits on any string input and no rate limiting, on a 256 MB machine

- **ID:** `cafe-full-07`
- **Severity:** High
- **Category:** Security (DoS)
- **Confidence:** High
- **Location:** `config/schema_lists/020_app/021_app_api.ron:34` (`content: String`), `050_support/051_support_api.ron:17` (`tg_handle: String`), `030_app_admin/030_app_admin_api.ron:17,36-37` (`tg_bot_token`, `app_name`); storage at `src/service/bot/router.rs:132-144`; no limit config in `src/config.rs:45-58` and `..Default::default()` at `src/config.rs:68`
- **What:** No endpoint validates the length or shape of any string. `SendMessage.content` goes straight into a `SupportMessage`/`SupportMemoryMessage` row. `SetMyTgHandle.tg_handle` is stored verbatim with no `@` prefix check and no charset check, even though the `/start` binding at `router.rs:431-437` matches against `format!("@{user_handle}")` and therefore only ever matches handles that begin with `@`. endpoint-libs 2.0 sets no `WebSocketConfig` frame/message caps (`rg 'WebSocketConfig|max_message_size' endpoint-libs-2.0.0/src/libs/ws/` → nothing), so tungstenite defaults apply: 16 MiB per frame, 64 MiB per message. There is no rate limiter anywhere in endpoint-libs 2.0 (the only `rate` hits are the log throttling layer).
- **Why it matters:** The VM is 1 shared CPU / 256 MB (`fly.toml:31-34`) and WorkTable is memory-resident, which is the whole reason 256 MB works (`api.support.cafe-analysis.md:100`). An attacker with `AppConnect` (i.e. anyone, per `cafe-full-02`) can create sessions in a loop and push 16 MiB messages into the in-memory `SupportMemoryMessage` table, which is only purged at 24h retention (`src/app.rs:127-130`). Sixteen such messages exhaust the machine. Persisted apps are worse: those rows never expire. Both paths need no credential.
- **Fix:** Three layers, all small. (1) Add explicit caps in the handlers, or better in one place: a `validate` step in the shared handler preamble from `cafe-full-09`, returning `EnumErrorCode::PayloadTooLarge` (already generated, `model.rs:889`). Concrete numbers: `content` ≤ 4000 (see `cafe-full-13`), `tg_handle` ≤ 33 and matching `^@[A-Za-z0-9_]{4,32}$`, `app_name` ≤ 128, `tg_bot_token` ≤ 64 and matching `^\d+:[A-Za-z0-9_-]+$`. (2) Ask endpoint-libs to expose `max_message_size`/`max_frame_size` on `WsServerConfig` and set them to something like 256 KiB here. (3) Rate limiting is an upstream ask; in the meantime Fly can do connection-level limiting at the edge.
- **Effort:** S for (1), which removes most of the exposure.
- **Blast radius:** handlers plus RON descriptions; no schema change needed if validation is code-side.

### [SEV-8] `ListApps` returns every app's Telegram bot token to every member, and now to MCP agents

- **ID:** `cafe-full-08`
- **Severity:** High
- **Category:** Security
- **Confidence:** High
- **Location:** `src/handlers/app_admin/list_apps.rs:50-60` (maps `r.tg_bot_token` into the response); schema at `config/structs.ron` → `AppConfig`; role gate at `config/schema_lists/030_app_admin/030_app_admin_api.ron:58`; the honest description at `:50`; MCP publication at `docs/appAdminApi_mcp_tools.json`
- **What:** `ListApps` is gated on the *connection* role `Admin | AppAdmin`, and the handler then chooses between `list_apps()` (all apps, platform admin) and `list_apps_for_user()` (every app the caller has any membership in, `src/service/app/member.rs:190-204`). The response includes `tg_bot_token` unredacted. Because `UserRole` is derived from memberships (`recompute_user_role_from_memberships`, `member.rs:297-333`), a user who is Owner/Admin of *one* app holds `AppAdmin` globally, and `list_apps_for_user` filters by membership rather than by admin-level membership, so being a mere `Support` member of app B is enough to receive app B's bot token.
- **Why it matters:** A Telegram bot token is full control of that bot: read every message the support desk receives, impersonate the support desk to every end user, and revoke the tenant's own access. Cross-tenant. The RON description at line 50 acknowledges it ("treat it as a secret"), which is honest but is not a control. Enabling MCP made it materially worse: `tools/call list_apps` puts raw bot tokens into an LLM's context window and therefore into whatever transcript store, telemetry sink, or downstream model that agent is wired to. `api.support.cafe-mcp-plan.md:133` flagged exactly this as something to fix "as part of this work"; it was not done.
- **Fix:** Small and worth doing before anything else on this list, because it is 10 lines. Redact by default: return `tg_bot_token: Option<String>` set to `None` unless the caller is `Owner` of that specific app, or drop the field from `AppConfig` entirely and add a separate `GetAppBotToken(app_public_id)` endpoint gated on `ensure_app_owner`, which also gives you an audit point. Either way, change `list_apps_for_user` to filter on `Owner | Admin` membership so a Support member does not see admin-shaped rows at all.
- **Effort:** S
- **Blast radius:** `list_apps.rs`, `config/structs.ron`, regenerate; breaking for any client reading `tgBotToken` from `ListApps`.

### [SEV-9] The same 9-line auth preamble is written out in 20 handlers

- **ID:** `cafe-full-09`
- **Severity:** Medium
- **Category:** Design
- **Confidence:** High
- **Location:** all 20 files matching `rg -l 'Connection not authenticated' src/handlers`; the `AppPublicId` variant in 9 of them (`rg -l 'let app_public_id: AppPublicId = req.app_public_id.into\(\);' src/handlers`)
- **What:** Every authenticated handler opens with a byte-identical block:
  ```rust
  let user_pub_id = self
      .user_connection_registry
      .get(ctx.connection_id)
      .await
      .ok_or_else(|| {
          CustomError::new(EnumErrorCode::Unauthorized)
              .with_message("Connection not authenticated")
      })?;
  ```
  Nine of them precede it with `let app_public_id: AppPublicId = req.app_public_id.into();` and follow it with an `ensure_*` call. Every one of the 24 `impl RequestHandler` blocks also repeats `type Error = CustomError;`, a `#[derive(Clone)]`-or-not struct holding `Arc`s copied from `AppCtx`, and a `tracing::debug!` entry/exit pair with hand-written field lists. Handler code is 1,626 lines for 24 endpoints, most of which do one service call.
- **Why it matters:** Beyond the obvious, this is the shape that makes `cafe-full-06` unfixable one file at a time: the error decision is duplicated 20 times, so *any* cross-cutting change to auth, validation, or logging is a 20-file diff with 20 chances to drift. It already has drifted once: `list_messages.rs:51` uses `Forbidden` where its 19 siblings use `.internal()`.
- **Fix:** One extension trait, no macro needed, no endpoint-libs change:
  ```rust
  // src/handlers/utils/auth.rs
  pub trait AuthedCtx {
      async fn caller(&self, reg: &UserConnectionRegistry) -> Result<UserPublicId, CustomError>;
      async fn app(&self, reg: &AppConnectionRegistry) -> Result<AppPublicId, CustomError>;
  }
  impl AuthedCtx for RequestContext { /* the 9 lines, once */ }
  ```
  Handlers become `let actor = ctx.caller(&self.user_connection_registry).await?;`. Then fold the DI in: rather than 24 structs each listing the `Arc`s they need, hold `Arc<AppCtx>` in one `Handler<M>` wrapper (`AppCtx` is already the container, `src/app.rs:27-40`), which also collapses the 10 near-identical `server.add_handler(MethodX { … })` blocks in `app_admin/mod.rs:26-71`. Combined with `cafe-full-06` this takes the 20 handlers from ~65 lines each to ~25.
- **Effort:** M
- **Blast radius:** all of `src/handlers/**`; internal only, no wire change.

### [SEV-10] `ChatSession` has no index on `user_pub_id`, so `list_sessions` full-scans and packs per row

- **ID:** `cafe-full-10`
- **Severity:** Medium
- **Category:** Performance
- **Confidence:** High
- **Location:** `src/db/schema/chat_session.rs:21-24` (indexes: `app_public_id`, `session_id` only); `src/service/session.rs:228-262`
- **What:** `list_sessions` is the *only* consumer of `ChatSession` by user, and it does `select_all().execute()` then filters in Rust on `r.user_pub_id == packed_user_id`. Worse, the app filter closure calls `app_id.pack().expect("app_id packs")` **inside the per-row predicate** (`session.rs:247`), so it re-derives the same 12-byte packed id once per row in the table, allocating and `expect`-ing each time.
- **Why it matters:** `ListChatSessions` is called on every widget open. At demo scale this is invisible; at 100k sessions it is 100k comparisons plus 100k redundant packs per call, on a 1-shared-CPU VM, holding whatever read guard `select_all` takes. The interesting part is *why* it was written this way: adding the index is not free (see `cafe-full-16`), so the shortcut is rational and the fix is a deployment event.
- **Fix:** Two independent changes. (1) Free and immediate: hoist the pack out of the closure with `let packed_app = app_filter.map(|a| a.pack()).transpose()?;` before the iterator, then compare against `packed_app.as_ref()`. This also removes an `expect` from a network-reachable path. (2) Add `user_pub_id_idx: user_pub_id` to the `worktable!` indexes and switch to `select_by_user_pub_id(...).execute()`. Per `docs/worktable-schema-and-deploy.md:26` an index change is a schema change, so (2) must ship with a `ChatSession` v1→v2 migration, which the migration crate cannot currently do for this table at all (`cafe-full-04`). Do `cafe-full-04` first.
- **Effort:** S for (1); M for (2) once the migration crate covers `ChatSession`.
- **Blast radius:** (1) one function. (2) schema + migration + deploy.

### [SEV-11] The event hot path is an SPSC channel behind a global async mutex

- **ID:** `cafe-full-11`
- **Severity:** Medium
- **Category:** Performance / Design
- **Confidence:** High
- **Location:** `src/service/bot/router.rs:29-32` (type aliases), `:59-60` (`crossfire::spsc::new`), `:166-171` and `:412-417` (send sites); consumer at `src/handlers/utils/subscription_router.rs:33-57`
- **What:** `BotRouter` creates a **single-producer** crossfire channel and then wraps the sender in `Arc<Mutex<SupportEventTx>>` so that N producers can share it: every API `send_message` and every inbound Telegram reply from every app bot takes the same `tokio::Mutex` and holds it across `.send().await`. Both send sites discard the result with `let _ =`. On the consumer side, `SubscriptionRouter` takes a **write** lock on `RwLock<SubscriptionManager>` for every publish (`subscription_router.rs:38-54`), including the read-only `publish_to_key`.
- **Why it matters:** This is the message-delivery path for the entire service, and it is serialized twice: once on the producer mutex, once on the subscriber write lock. If the consumer task stalls (a slow WebSocket, and `message_buffer_size` defaults to 256 with `drop_conn_on_buffer_full: false` per `endpoint-libs-2.0.0/src/libs/ws/server.rs:638-644`), `send().await` blocks *while holding the mutex*, so one slow subscriber stalls every app's message sending, including the WorkTable write that precedes it. The `let _ =` means that when the receiver is gone, every event is silently dropped and `SendMessage` still returns success.
- **Fix:** Use `crossfire::mpsc` and hand each producer its own `Tx` clone; the mutex disappears entirely and with it the head-of-line blocking. Log the send error instead of `let _ =` (a failed publish means the subscriber fan-out is dead, which is worth a `warn!`). Separately, `SubscriptionManager` needing `&mut` for a publish is worth raising with endpoint-libs; if `publish_to_key` can take `&self`, the `RwLock` becomes a real read lock.
- **Effort:** S for the mpsc swap; the endpoint-libs ask is upstream.
- **Blast radius:** `bot/router.rs` type aliases and two send sites; `subscription_router.rs` unchanged.

### [SEV-12] Startup sleeps 500 ms per app before the server starts listening

- **ID:** `cafe-full-12`
- **Severity:** Medium
- **Category:** Performance / Availability
- **Confidence:** High
- **Location:** `src/service/bot/service.rs:55-67`; sequenced at `src/app.rs:126` (before `server.listen()` at `:153`)
- **What:** `bootstrap_bots` iterates active apps, calls `register_bot`, and `sleep(500ms)` after each, sequentially, and `App::run` awaits it to completion before constructing the server and calling `listen()`.
- **Why it matters:** Time-to-listen is `0.5 × active_apps` seconds. At 60 tenant apps that is 30 seconds of refused connections on every deploy and every machine restart; `fly.toml:12-14` uses a rolling strategy with `max_unavailable = 1` and `min_machines_running = 1`, so the health check window and the restart window are exactly when this bites. Nothing in the code caps the app count. The stagger is presumably there to be polite to Telegram's API, but Telegram rate-limits per bot token, and these are all different tokens, so the stagger buys nothing that matters here.
- **Fix:** Move `bootstrap_bots` into a `tokio::spawn` after `listen()` starts (it does not need to complete before the server accepts connections; `send_message` already handles a missing bot client at `router.rs:176-182`), or at minimum `futures::stream::iter(...).buffer_unordered(8)` it. If the stagger is genuinely needed, keep it but do it off the startup path.
- **Effort:** S
- **Blast radius:** `app.rs` ordering; a bot may be a second late for the first request after boot, which `router.rs:180` already reports as an error rather than a panic.

### [SEV-13] Messages over Telegram's 4096-char limit are stored and acknowledged but never delivered

- **ID:** `cafe-full-13`
- **Severity:** Medium
- **Category:** Correctness
- **Confidence:** Medium (the Telegram limit is well known; I did not exercise the API to confirm the exact failure shape)
- **Location:** `src/service/bot/router.rs:146-155`
- **What:** `send_message` stores the row, then for each enabled support chat builds `format!("{msg_prefix}{content}")` and calls `client.execute(method).await`. On error it does `warn!(…"failed to send TG message")` **and continues**, so no error is propagated. The function then publishes the event and returns `Ok(sent_at)`, so `SendMessage` answers success.
- **Why it matters:** Telegram's `sendMessage` caps text at 4096 UTF-16 code units, and the prefix (`{session_id}\nfrom: {sender}\n`) adds ~25 more. There is no length validation anywhere (`cafe-full-07`), so a user pasting a stack trace gets a green tick in the widget, sees their message in `ListMessages`, and support never receives it. This is the worst failure mode for a support product: the user believes they have been heard. The same swallow-and-continue hides genuine outages (revoked token, bot blocked by the user, Telegram 5xx).
- **Fix:** Validate `content.len()` up front and reject over ~4000 with `PayloadTooLarge` (part of `cafe-full-07`), or chunk long content across multiple `sendMessage` calls. Independently, count the failures: if *every* enabled chat failed, that is not a warning, it is a failed send: return an error, or at minimum add a `delivered: bool` to the stored row so the UI can show "not yet delivered to support".
- **Effort:** S for validation; M for chunking plus delivery status.
- **Blast radius:** `bot/router.rs`; adding a `delivered` column is a `SupportMessage` schema change and therefore a deployment event.

### [SEV-14] `DeleteApp` orphans sessions and messages; several maps are never pruned

- **ID:** `cafe-full-14`
- **Severity:** Medium
- **Category:** Correctness
- **Confidence:** High
- **Location:** `src/service/app.rs:223-256` (`delete_app` removes `AppMember` rows and the `AppConfig` row only); `src/service/message_store.rs:25,160-166` (`app_locks` never removed); `src/service/bot/router.rs:43` (`bots` is cleared only at shutdown)
- **What:** `delete_app` deletes memberships and the config row. `ChatSession` rows for that app and `SupportMessage` / `SupportMemoryMessage` rows for that app are left behind with no owner. The RON description is honest about it (`040_admin/041_admin_api.ron:15`, "Existing sessions and messages are not cascaded"), so this is known, but the consequences are not documented.
- **Why it matters:** Concretely: after `DeleteApp`, `MessageStore::persistence_enabled` (`message_store.rs:153-158`) looks the app up and returns `Err("App not found")`, so `list_messages` on any orphaned session now fails with an *internal* error (via `cafe-full-06`) rather than a clean not-found, and `store_message` fails the same way, so an in-flight `SendMessage` 500s. Persisted rows are then immortal: nothing purges them (only the memory table has a purge task) and no code path can reach them. On a fixed-size machine with S3 sync, that is unbounded growth of unreachable data plus a GDPR-shaped problem, since support transcripts are personal data. The `app_locks` map and the registries (`cafe-full-05`) leak on the same principle.
- **Fix:** Make `delete_app` cascade: delete `ChatSession` by `app_public_id` (the index exists, `chat_session.rs:22`), delete `SupportMessage` and `SupportMemoryMessage` by `app_public_id` (indexes exist), then remove the `app_locks` entry and the `bots` entry. `AppMember` already has `ByAppPublicId` delete; `ChatSession` and `SupportMessage` do not declare delete queries, so adding them is a `worktable!` change; check whether adding a `delete:` block counts as a schema change under `docs/worktable-schema-and-deploy.md:26` (I believe it does not change on-disk layout, but that needs confirming before merge, per the repo's own rule).
- **Effort:** M
- **Blast radius:** `service/app.rs`, two or three `db/schema/*.rs` query blocks, possibly a migration.

### [SEV-15] `README.md` claims the committed MCP tool JSONs are the exact `tools/list` output; they are not

- **ID:** `cafe-full-15`
- **Severity:** Medium
- **Category:** Docs
- **Confidence:** High
- **Location:** `README.md:5`; contradicted by `config/schema_lists/010_auth/010_init.ron:15`, `config/schema_lists/020_app/020_app_connect.ron:15`, `src/handlers/auth_api.rs:45-72` vs `:76-89`
- **What:** `README.md:5` states "The per-service `docs/*_mcp_tools.json` files are the exact `tools/list` output." Three ways that is wrong:
  1. **`docs/authApi_mcp_tools.json` (`init`) and `docs/appConnect_mcp_tools.json` (`app_connect`) can never appear in `tools/list`.** Both are registered with `add_auth_endpoint` (`auth_api.rs:45,66`), not `add_handler`, so they are absent from the `handlers` map that `McpState::build` walks (`endpoint-libs-2.0.0/src/libs/ws/mcp.rs:262-320`). The RON descriptions themselves say "not callable as a tool", so the repo contradicts itself in two files.
  2. **The real tool surface is larger than the docs.** `MethodReceiveToken`, `MethodReceiveUserInfo` and `MethodReceiveUserDeleted` *are* registered via `add_handler` (`auth_api.rs:76-89`) and therefore *are* MCP tools, gated on role 6 (`honey_id-types-2.0.0/src/types/generated.rs:1800,1853,1918`, matching `UserRole::HoneyAuth = 6` in `config/enums.ron:42-46`). No `docs/*_mcp_tools.json` covers them.
  3. `tools/list` is role-filtered per connection (`mcp.rs:338`), so no single connection ever sees any of these files in full.
- **Why it matters:** This is the README of the repo that exists to be copied, and the claim is the one thing a reader would rely on when adding MCP elsewhere. Point 2 is the one with teeth: a reviewer diffing `docs/*_mcp_tools.json` to check "what did we just expose to agents" would miss three honey.id callback tools entirely.
- **Fix:** Reword to "the per-service tool schemas endpoint-gen derives from the RON; handshake-only endpoints (`init`, `app_connect`) are listed for completeness but are never returned by `tools/list`, and `tools/list` is additionally filtered by the calling connection's roles." Then either generate a file for the honey.id callbacks or add a line naming them.
- **Effort:** S
- **Blast radius:** README only.

### [SEV-16] WorkTable API gaps this repo is working around (feedback for the WorkTable repo)

- **ID:** `cafe-full-16`
- **Severity:** Medium
- **Category:** Design
- **Confidence:** High
- **Location:** as listed per item
- **What:** Collected because the shapes here are all "the consumer hand-rolls something the database should provide". In rough order of cost to this repo:
  1. **No transactions, not even multi-row.** `MessageStore::move_persisted_to_memory` / `move_memory_to_persisted` (`src/service/message_store.rs:168-254`) is a hand-written two-phase commit: copy every row, re-scan to verify every row copied, flip the config flag, then best-effort delete the source rows with `if let Err(e) = … warn!`. If the process dies between the flag flip and the deletes, rows exist in both tables forever. `AppService::create_app` (`src/service/app.rs:95-119`) inserts the config row then the owner membership row with no rollback, so a failed second insert leaves an ownerless app. Both would be three lines with a transaction.
  2. **Adding an index requires a version bump and a data migration.** This is why `cafe-full-10` exists: the cheap fix (index `ChatSession.user_pub_id`) costs a deployment event, so a full scan was written instead. Index metadata is derived data; rebuilding it on load would make index changes free and would remove the single biggest friction in this codebase.
  3. **`autoincrement` does not autoincrement.** Every insert writes `id: self.table.get_next_pk().into()` (7 sites: `session.rs:53`, `app.rs:86,104`, `member.rs:37`, `user.rs:86`, `message_store.rs:49,53,181,225`), and `MessageStore` even constructs rows with `id: 0` and patches them afterwards (`router.rs:134`, `message_store.rs:49`). The macro declares `primary_key autoincrement`; it should assign it.
  4. **Three different select shapes.** Primary key is `table.select(pk) -> Option<Row>`; unique index is `table.select_by_x(v) -> Option<Row>`; non-unique index is `table.select_by_x(v).execute() -> Result<Vec<Row>>`. Forgetting `.execute()` is a type error, but the asymmetry means every call site reads differently and `use worktable::prelude::SelectQueryExecutor` has to be imported in 8 files.
  5. **No bulk delete.** `purge_memory_before` (`message_store.rs:119-130`) and both `move_*` functions loop row-by-row awaiting each `delete`. A `delete_where`/`delete_by_index` returning a count would replace all three loops.
  6. **Table construction boilerplate forces a local macro, three times.** `src/db/tables.rs:52-62`, `:97-110` and `:153-166` are three copies of the same `disk_load!`/`s3_load!` macro because `DiskConfig::new_with_table_name(path, T::name_snake_case(), T::version())` has to be spelled out and because the engine type name differs between the plain and S3 builds (`XPersistenceEngine` vs `XS3SyncPersistenceEngine`). A `T::load_from(path, opts)` associated function, with the S3 variant selected by config rather than by a differently-named type, would delete ~140 lines here and the entire `#[cfg]` fork of `Tables::new`.
  7. **Errors are strings.** `map_err(|e| eyre::eyre!("Delete error: {e}"))` (`app.rs:246`), `"DB error: {e}"` (`router.rs:189`). There is no way to distinguish "unique constraint violated" from "disk full", which is why `add_member` pre-checks for an existing membership (`member.rs:28-34`) instead of just inserting and matching on the error, a TOCTOU race in the process.
- **Why it matters:** Items 1 and 2 are the ones that changed this codebase's shape: they produced the hand-rolled 2PC and the full-table scan respectively. This is the highest-value feedback the demo can give its own database.
- **Fix:** Not a fix here; file as WorkTable issues, ordered 1, 2, 3, 6, 5, 7, 4.
- **Effort:** n/a (upstream)
- **Blast radius:** n/a

### [SEV-17] Dead code census: ~14 unused items, including two invented abstractions

- **ID:** `cafe-full-17`
- **Severity:** Medium
- **Category:** AI-smell
- **Confidence:** High
- **Location:** each item below
- **What:** Functions and types defined and never called from anywhere in `src/` or `migration/`:
  - `ChatSessionService::is_for_app` (`src/service/session.rs:139-169`): 31 lines including two tracing spans, zero callers. Its job is done inline at `list_messages.rs:50`.
  - `AppService::remove_member` (`src/service/app/member.rs:51-72`): zero callers; there is no remove-member endpoint.
  - `AppService::exists` (`src/service/app.rs:199-206`): zero callers. Ironically it is exactly the check `AppConnect` is missing (`cafe-full-02`).
  - `BotService::get_status` / `get_all_statuses` and `BotRouter::get_status` / `get_all_statuses` (`service.rs:69-75`, `router.rs:203-216`): four functions, zero callers, no endpoint exposes bot status.
  - `BotStatus::Restarting { next_attempt_ms }` and `BotStatus::Error(String)` (`router.rs:38-39`): never constructed. Status is set to `Running` at `router.rs:236` and `Stopped` at `:245,258`; there is no restart logic at all, so a bot whose long-poll dies stays silently dead.
  - `PurgeableTable` (`src/db/util.rs:9-12`): a trait with exactly one implementor (`support_memory_message.rs:63-76`) whose single method is never called. `MessageStore::purge_memory_before` (`message_store.rs:113-133`) reimplements the same logic with per-app locking. Textbook invented abstraction plus a near-duplicate that drifted.
  - `RoutingMessage::for_all` / `for_multi` (`src/handlers/utils/routing_message.rs:17-29`) and therefore `Receiver::All` / `Receiver::ConcreteMulti` (`receiver.rs:5,7`): never constructed. `SubscriptionRouter` has match arms for both (`subscription_router.rs:43-54`) that can never execute; the `ConcreteMulti` arm even contains a needless `.clone()` at `:46`.
  - `RuntimeConfig::working_threads` and `tasks_ratio` (`src/config.rs:36,39-43`): `main.rs:16` uses `config.runtime.threads` directly.
  - `ServiceConfig::platform_api_key` (`src/config.rs:151-154`): never read anywhere, and it ships a default of `"default-platform-key"`. A config key that looks like a credential, is settable via Doppler, and is silently ignored.
  - `LogService::get_level` (`src/service/log.rs:31-33`): zero callers.
  - Generated but unused worktable queries: `TgBotTokenById`, `AppNameById`, `ActiveById` (`app_config.rs:28,30,32`), `RoleById` (`user.rs:25`).
- **Why it matters:** Individually trivial; together they are the tell that the code was generated broadly and pruned never, and each one costs the next reader time deciding whether it is load-bearing. `ServiceConfig::platform_api_key` is the one with a real edge: an operator setting it in Doppler would reasonably believe it does something. `BotStatus::Restarting`/`Error` are a promise of resilience the code does not keep.
- **Fix:** Delete all of it. Two judgement calls: keep `AppService::exists` if `cafe-full-02`'s interim mitigation is going in (it is the right function), and either implement bot restart or delete the two `BotStatus` variants, because a half-modelled state machine is worse than no state machine.
- **Effort:** S
- **Blast radius:** internal only; `git grep` confirms no external consumers (it is a binary crate plus one workspace member).

### [SEV-18] CI deploys with config files that are not in the repo

- **ID:** `cafe-full-18`
- **Severity:** Medium
- **Category:** Docs / Ops
- **Confidence:** High
- **Location:** `.github/workflows/ci.yml:138` (`--config fly.dev.toml`), `:31` (path filter names `Dockerfile.ci` and `.github/workflows/prod.yml`)
- **What:** The deploy step runs `flyctl deploy --config fly.dev.toml`. There is no `fly.dev.toml` in the repo, only `fly.toml`. The change-detection filter at line 31 also matches `Dockerfile\.ci` and `\.github/workflows/prod\.yml`, neither of which exists (`ls .github/workflows/` → `ci.yml` only).
- **Why it matters:** Either the deploy is silently using flyctl's default resolution (in which case `fly.toml` is what ships and the flag is a lie), or it is failing and nobody noticed. Both are bad, and the second is worse: `AGENTS.md:66-82` explains that CI does not attach checks to PRs, so a failing deploy is not visible at review time. The dangling `Dockerfile.ci` / `prod.yml` entries mean the path filter is describing a pipeline that no longer exists, so the "did anything deploy-relevant change" decision is being made against a stale map. This also makes `fly.toml`'s committed settings (256 MB, `min_machines_running = 1`, rolling with `max_unavailable = 1`) unverifiable as the settings actually in force, which matters for `cafe-full-07` and `cafe-full-12`.
- **Fix:** Decide which file is authoritative. If `fly.toml` is, drop `--config fly.dev.toml`. If a separate dev config genuinely lives out-of-band, say so in a comment at line 138 and in `docs/`. Prune `Dockerfile.ci` and `prod.yml` from the filter at line 31. Then re-check the fly app's actual VM size against `fly.toml:31-34`.
- **Effort:** S
- **Blast radius:** CI only.

### [SEV-19] N+1 and duplicated authorization logic on the Telegram paths

- **ID:** `cafe-full-19`
- **Severity:** Low
- **Category:** Performance / Maintainability
- **Confidence:** High
- **Location:** `src/service/app/member.rs:112-136` (N+1); `src/service/bot/router.rs:184-201` vs `:283-303` (duplicate)
- **What:** (a) `list_members_with_support_info` fetches the member list, then per member does `user_table.select_by_pub_id` **and** `support_info_table.select`, i.e. 2N point lookups per `ListAppMembers`. (b) `BotRouter::enabled_support_chat_ids` and `BotUpdateHandler::is_chat_enabled_for_app` are the same filter chain written twice: `select_by_app_public_id → is_support_enabled → role in {Owner,Admin,Support} → support_info lookup`. One collects `chat_id`s, the other asks whether a given `chat_id` is present. They have already drifted in error handling (`map_err` + `?` vs `let Ok(…) else { return false }`).
- **Why it matters:** (a) is cheap in-process so the cost is not latency, it is the pattern: this is the reference implementation, and this is the canonical N+1 shape. (b) is the one that could bite: these two functions *are* the authorization check for inbound support replies, written twice. If someone adds a "member is suspended" condition to one and not the other, a disabled staff member can still reply into a session (`is_chat_enabled_for_app` is the gate at `router.rs:369`) or stops receiving messages while still being able to reply. Duplicated authorization logic is exactly the kind that drifts.
- **Fix:** (b) first: extract one `fn enabled_support_chat_ids(&self, app: PackedNanoId) -> Result<Vec<i64>>` onto a shared struct, and implement `is_chat_enabled_for_app` as `.contains(&chat_id)` on its result. (a): once WorkTable grows a join or a batch-get (`cafe-full-16`), fold it; until then it is acceptable, but add a comment saying why.
- **Effort:** S
- **Blast radius:** `bot/router.rs`, `service/app/member.rs`.

### [SEV-20] Zero tests, in the repo other services copy

- **ID:** `cafe-full-20`
- **Severity:** Medium
- **Category:** Maintainability
- **Confidence:** High
- **Location:** repo-wide (`rg '#\[test\]|#\[tokio::test\]|mod tests' src/ migration/` → 0 matches); `AGENTS.md:31` lists `cargo test` in the build loop
- **What:** No unit tests, no integration tests, no test module anywhere. `cargo test` passes because it compiles and runs nothing. `AGENTS.md:52-53` ("Run what you build before reporting it done") is therefore leaning entirely on manual verification.
- **Why it matters:** I am naming specific branches rather than asking for coverage. The untested logic that would actually catch regressions: **(1)** `recompute_user_role_from_memberships` (`member.rs:297-333`): the Admin-sticky branch at `:306-308` and the Owner/Admin → `AppAdmin` → Support → User ladder is the only thing standing between a demoted app admin and retained `AppAdmin` platform role, and it is pure over in-memory tables, so it is trivially testable. **(2)** `MessageStore::set_app_persistence` in both directions (`message_store.rs:90-111`): the hand-rolled 2PC from `cafe-full-16` item 1, with a verify step that returns an error nobody has ever seen fire. **(3)** `verify_session_access` (`session.rs:172-207`): the IDOR gate. **(4)** `BotUpdateHandler::handle`'s reply parsing (`router.rs:313-421`): five early-return branches driven by attacker-influenced Telegram text, including the `lines[0].trim().len() == 16` check. All four are synchronous or single-task and need no network.
- **Fix:** Four test modules, one per item above, using `WorkTable::default()` in-memory tables (no persistence engine needed for 1-3). That is a few hours and it covers every authorization decision in the service.
- **Effort:** M
- **Blast radius:** new test code only.

### [SEV-21] Six `cargo audit` advisories, all transitive through the S3 stack

- **ID:** `cafe-full-21`
- **Severity:** Low
- **Category:** Security (supply chain)
- **Confidence:** High (advisories verified against the current `Cargo.lock`; reachability assessed from `cargo tree`)
- **Location:** `Cargo.lock`; `Cargo.toml:39` (`cert-provider` git dependency)
- **What:** `cargo audit` on `feef288` reports 6 vulnerabilities + 3 warnings:
  - `quick-xml 0.39.4`: RUSTSEC-2026-0194/0195 (quadratic parse, unbounded namespace allocation). Path: `worktable 0.9.1 → rusty-s3-temp 0.9.0 → quick-xml`. **Reachable** whenever an S3 response is parsed, i.e. every 300 s sync and every boot.
  - `quick-xml 0.38.4`: same two advisories. Path: `cert-provider → rust-s3 0.37.2 / aws-creds 0.39.1`. Reachable on the ACME cert-sync path.
  - `crossbeam-epoch 0.9.18`: RUSTSEC-2026-0204, invalid pointer deref in the `fmt::Pointer` impl. Path: `worktable → data_bucket → WorkTablesIndex → crossbeam-skiplist`. **Not reachable**: it requires `{:p}`-formatting an `Atomic`/`Shared`, which nothing here does.
  - `quinn-proto 0.11.14`: RUSTSEC-2026-0185. In the lockfile but **not in the normal dependency graph on any target** (`cargo tree --target all -i quinn-proto` → "nothing to print"); lockfile residue, not compiled in.
  - Warnings: `derivative` and `paste` unmaintained, `anyhow 1.0.102` unsound `downcast_mut`.
  - Separately, `cert-provider` is pinned to a **git rev on a personal fork** (`github.com/dVeon-loch/cert-provider.git`, rev `eb2387f2`) rather than a published crate, a single-maintainer supply-chain dependency in the TLS path of a production service.
- **Why it matters:** The two `quick-xml` advisories are the only genuinely reachable ones, and the attacker would have to control the S3 endpoint's XML responses, which means they already have the credentials from `cafe-full-01`. So the practical severity is Low *today* and rises if the credentials are not rotated. The `cert-provider` fork is the item with the longer tail: it sits in the certificate provisioning path and nobody outside one person can publish a fix.
- **Fix:** Push `worktable` to move `rusty-s3-temp` onto `quick-xml ≥ 0.41` (upstream ask; this repo cannot force it since `rusty-s3-temp` is a hard dependency). Prune `quinn-proto` with a `cargo update`. Consider vendoring or forking `cert-provider` into the pathscale org so the TLS path is not one person's repo. Add `cargo audit` to CI; the tooling already exists (`mcp_verify.sh:62`) but only as a manual script.
- **Effort:** S for the CI wiring; the rest is upstream.
- **Blast radius:** dependency graph only.

### [SEV-22] `AppConnectResponse.app_name` is always `None`, and the response is never validated

- **ID:** `cafe-full-22`
- **Severity:** Low
- **Category:** Correctness
- **Confidence:** High
- **Location:** `src/handlers/app/auth.rs:42-45`; schema at `config/schema_lists/020_app/020_app_connect.ron:22`; docs at `docs/README.md:73`
- **What:** The `AppConnect` schema advertises `app_name: Optional(String)` and the handler hardcodes `app_name: None`. It never looks the app up, which is the same omission as `cafe-full-02` seen from the other side.
- **Why it matters:** A widget that renders "You are chatting with {app_name}" gets nothing, forever, with no error. The field is in `docs/README.md:73` and in `docs/appConnect_mcp_tools.json`, so it is documented as available. Small, but it is the kind of thing a copying service inherits verbatim.
- **Fix:** Look the app up (`AppService::get_app`, which exists and is used elsewhere), fail the handshake if it is absent or `active == false` (which is also mitigation (a) and (c) from `cafe-full-02`), and return the real name. Note `MethodAppConnect` currently has no `AppService`; it would need one injected in `auth_api.rs:66-72`.
- **Effort:** S
- **Blast radius:** `auth.rs`, `auth_api.rs` wiring.

### [SEV-23] The `s3-sync` build without credentials points every table at `placeholder.local`

- **ID:** `cafe-full-23`
- **Severity:** Low
- **Category:** Design
- **Confidence:** High
- **Location:** `src/db/tables.rs:139-188`, in particular the comment at `:143` and the config at `:144-151`
- **What:** When the `s3-sync` feature is on but no credentials are configured, `new_disk_only` builds a fake `S3Config { bucket_name: "placeholder", endpoint: "https://placeholder.local", access_key: "placeholder", … }` and wraps every table in a full S3 sync engine anyway. The comment says "Use placeholder S3 config - sync will fail but disk ops will work."
- **Why it matters:** CI builds with `--features s3-sync,acme,cert-s3-sync` (`ci.yml:111`), so this is the code path anyone running a local or CI binary without Doppler hits. Every table then attempts a sync to a non-resolving host on a 300 s timer, producing recurring DNS failures in the logs of exactly the environment where you are trying to read logs. It also means "S3 sync is broken" and "S3 sync is intentionally off" look identical at runtime. And the function is a third near-verbatim copy of `Tables::new` (`:48-82`, `:84-137`, `:139-188`), differing only in the config it feeds the same macro.
- **Fix:** The clean version is for the persistence engine to take the S3 config as an `Option` so "no S3" is representable (a `cafe-full-16` item 6 ask). Locally: at minimum make the fallback loud once at startup (`warn!("s3-sync feature is on but no credentials configured; running disk-only")`) and confirm the sync task is actually inert rather than retrying; if it retries, that is a background task burning the shared CPU forever.
- **Effort:** S locally; M with the upstream change that also collapses the three copies.
- **Blast radius:** `db/tables.rs`.

### [SEV-24] `CLAUDE.md` asserts the hook watchlist and `permissions.ask` are in sync; they are not

- **ID:** `cafe-full-24`
- **Severity:** Low
- **Category:** Docs
- **Confidence:** High
- **Location:** `CLAUDE.md:13` and `AGENTS.md:107-118`; `.claude/settings.json:14-27` vs `.claude/hooks/ask-before-risky-commands.sh:25,48-63`
- **What:** `CLAUDE.md:13` says to keep the hook's `RISKY_WORDS` and `permissions.ask` "in sync", because "they back each other up". They diverge in both directions. The hook's `RISKY_WORDS` includes `terragrunt` and `fly`, which `permissions.ask` does not. The hook additionally gates `git clean`, recursive `rm`, `find -delete`, `npm/pnpm/yarn/bun/cargo/gem publish`, `twine upload`, `gh repo delete`, `gh release create|delete`, `gh api -X POST|PUT|PATCH|DELETE`, any `*/deploy*` script, and `regenerate_endpoints`, none of which appear in `permissions.ask`.
- **Why it matters:** Low impact (the hook is the stricter of the two, so the net effect is more prompting, not less) but it makes the stated invariant false, and `AGENTS.md:114-116` tells non-Claude agents to "apply the same rule yourself: ask before running any command family listed in the hook", so the hook is load-bearing documentation for other tools, and the claim that the two files mirror each other is the thing a reader would check first.
- **Fix:** Either add the missing families to `permissions.ask`, or reword `CLAUDE.md:13` to say the hook is a deliberate superset and `permissions.ask` covers the declarative subset. The second is more honest: `permissions.ask` cannot express "a deploy script invoked by path", which is why the hook exists.
- **Effort:** S
- **Blast radius:** two config/doc files.

<details>
<summary>Nits (one line each)</summary>

- `src/service/app_connection_registry.rs:9` and `user_connection_registry.rs:8`: `xustome` → `custom` (same typo, copy-pasted).
- `src/config.rs:48` (`name` default `"tg_support"`) and `:145` (`/var/lib/tg_support/data`) still use the pre-rename product name, while `:81` uses `/var/log/support_cafe`. Pick one.
- `Cargo.toml:44` has no trailing newline (`\ No newline at end of file` in `feef288`).
- `docs/error_codes/error_codes.json` is a 0-byte file; `error_codes.md` next to it is populated. Either generate the JSON or delete it.
- `docs/README.md:47` has a stray whitespace-only line between the enums block and the first service section (generator artifact).
- `src/service/bot/router.rs:238`: `handler.app_public_id.clone()` on a `Copy` type; clippy would flag it under `clippy::clone_on_copy` if warnings were denied.
- `src/service/bot/router.rs:242`: `Arc::unwrap_or_clone(client_for_poll)` always clones here because the `Arc` has ≥2 strong refs; the intent reads as "take ownership if possible", which never happens.
- `src/handlers/app/mod.rs:23`: `.expect("event stream already taken")` on a fallible startup step. Fine because it is startup, but `register_handlers` is `async fn` returning `()` so it cannot propagate; making it return `Result` would remove the panic.
- `src/service/message_store.rs:119-130`: `purge_memory_before` selects rows, then takes the per-app lock *inside* the loop and re-checks by `message_id` before deleting by `row.id`; the re-check does not confirm it is the same row, so a concurrent persistence toggle could delete a re-inserted row's predecessor. Low confidence that it is reachable; worth a comment either way.
- `src/handlers/app_admin/list_apps.rs:37`: `// TODO: Split this into separate endpoints for platform admin and regular users`. Agreed, and it would also fix `cafe-full-08`'s over-broad membership filter.
- `src/service/app.rs:190-194`: `edit_app` routes `message_persistence_enabled` through `MessageStore::set_app_persistence`, which takes the per-app lock and migrates messages, a potentially long operation buried in a field-wise update. Not wrong, but surprising from the call site.
- `AGENTS.md:31` lists `cargo test` in the standard loop; with zero tests it always passes, which is worse than not listing it (see `cafe-full-20`).
- `rq2_out/` and `mcp_verify.sh` / `rq1_scan.sh` are research artifacts for a paper, committed at the repo root and iterating over six sibling repos by absolute path. They are not part of the service; consider moving them under `scripts/research/` or out of the repo.
- `api.support.cafe-analysis.md` (external, read-only) is stale at `:13-15` (worktable 0.9.0-beta → 0.9.1, endpoint-libs 1.7.28 → 2.0, endpoint-gen 1.5.2 → 1.10.1) and at `:111` ("error-code enum is a placeholder (`Xxx = 0`)", which is in fact fully populated at `src/codegen/model.rs:861-940`). Its risk list items 1, 2, 3, 4, 5, 6, 7 all still hold.

</details>

## Cross-cutting recommendations

**1. Close the credential and identity holes before anything else.**
`cafe-full-01` and `cafe-full-02` are the only two findings that are exploitable by someone who has done nothing but clone the repo. Rotate the Tigris keys today; the history scrub can follow. For `AppConnect`, even if the app-signed-token design takes a week, ship the three existence checks (`cafe-full-02` fix (a)-(c)) this week; they are ten lines and they turn "guess a nanoid" into "know a real nanoid". *What would break:* the history rewrite breaks every existing clone and conflicts with `AGENTS.md:104-105`, so it needs a human decision and an announcement. The `AppConnect` checks break any widget currently connecting with a made-up app id, which is a feature.

**2. Make the handler layer correct once instead of 20 times.**
`cafe-full-06` (authorization → internal error) and `cafe-full-09` (20x auth preamble) are the same problem viewed from two angles, and they should be one change: an `AuthedCtx` extension trait on `RequestContext`, typed `CustomError` returns from the `ensure_*` helpers, and a `Handler<M>` wrapper holding `Arc<AppCtx>` instead of 24 bespoke structs. Do `cafe-full-07`'s input validation in the same pass, since it wants the same hook. Rough plan: `service/app/member.rs` and `service/session.rs` first (change the return types), then the 20 handlers mechanically, then delete the now-unused `.internal()` imports. *What would break:* clients that treat every non-200 as retryable will start seeing 403s they must not retry. That is the correct behaviour and it should be called out in a release note. This is the single change that most improves the repo's value as a reference.

**3. Treat the persistence story as a whole, not as three unrelated items.**
`cafe-full-03` (0.9.1 bump with no migration), `cafe-full-04` (migration crate covers 4 of 6 tables), `cafe-full-14` (delete cascades nothing) and `cafe-full-10` (missing index) are one theme: the data layer's operational contract is documented but not implemented. Sequence: fix the migration crate to cover all six tables and drive it from one list; use it to do the 0.9.1 rewrite pass against a copy of the production prefix and record the result in the Precedent section of `docs/worktable-schema-and-deploy.md`; then, with a working migration path in hand, the `ChatSession.user_pub_id` index and the `DeleteApp` cascade become routine instead of scary. *What would break:* nothing, if the rewrite pass runs against a copy first. Everything, if it does not.

**4. Send the WorkTable feedback upstream as issues, not as comments here.**
`cafe-full-16` is the most valuable output of this review for the wider org: this repo is the only real consumer of WorkTable at any scale, and it has silently absorbed the cost of no transactions (a hand-rolled 2PC in `message_store.rs`), index-changes-are-migrations (a full table scan in `session.rs`), and no `load_from` (three copies of `Tables::new`). File items 1, 2, 3 and 6 as WorkTable issues with the file:line evidence from this doc attached. *What would break:* nothing here; the payoff is that the next version of this repo is 200 lines shorter.

**5. Put a floor under the repo: tests on the four authorization decisions, and `cargo audit` in CI.**
`cafe-full-20` names four specific branches, all synchronous, all testable against in-memory `WorkTable::default()` tables. They cover every authorization decision the service makes. Pair that with wiring `cargo audit` into `ci.yml` (the invocation already exists in `mcp_verify.sh:62`) and fixing the `fly.dev.toml` / `Dockerfile.ci` / `prod.yml` dangling references at `ci.yml:31,138` so that CI is describing the pipeline that actually runs. *What would break:* CI may go red on the existing `quick-xml` advisories, which is a decision to make deliberately (`--ignore` with a dated comment) rather than by omission.

## What I did not cover

- **Runtime verification of anything.** I did not build, run, or connect to the service. Every finding is from reading source. In particular `cafe-full-03` (whether 0.9.0-beta-written snapshots actually load under 0.9.1) and `cafe-full-13` (the exact Telegram failure shape) need someone to actually run them.
- **Whether the committed Tigris credentials are still live.** I did not attempt to use them, and nobody should from a review. Assume live until Tigris says otherwise.
- **`src/codegen/model.rs` (2,235 lines).** Treated as generated and trusted; I verified only that it is byte-identical to `generated/model.rs` and that `EnumErrorCode` and the role constants match the RON. Bugs inside endpoint-gen's output are out of scope.
- **endpoint-libs and honey_id-types internals** beyond the specific questions I needed answered (MCP role filtering, `get_conn_id`, `WsServerConfig` defaults, honey callback roles, CORS headers, absence of rate limiting and disconnect hooks). I did not audit either crate.
- **`src/acme.rs` beyond lines 1-120** and the `cert-provider` fork's contents. The ACME/TLS path deserves its own pass, especially given the personal-fork dependency noted in `cafe-full-21`.
- **The Doppler secret path end to end.** I read `doppler_source.rs` and confirmed it runs before the tokio runtime is built (so the nested `block_on` at `:43-53` is safe, not a blocking-in-async bug), but I did not check what keys the production Doppler config actually holds, nor whether `CAFE__` prefix stripping (`:65`) collides with any legitimate key name.
- **Frontend/widget clients.** None are in this repo; the `AppConnect` blast radius assessment assumes there are some and I could not enumerate them.
- **Whether `fly.toml`'s committed values match the deployed app.** Unverifiable from here given `cafe-full-18`.

## Quick-start for the follow-up agent

**Read in this order:**
1. `AGENTS.md`: the working agreement; `docs/worktable-schema-and-deploy.md` is its most load-bearing pointer and is directly implicated in `cafe-full-03`.
2. `src/app.rs`: `AppCtx` is the whole dependency graph in 40 lines, and `run()` is the startup order (bootstrap → bots → purge task → MCP → listen).
3. `src/handlers/app/auth.rs`: 49 lines, and the root of the most serious finding. Read it next to `src/service/session.rs:172-207`.
4. `src/handlers/app_admin/create_app.rs`: the representative handler; once you have read one you have read all 24.
5. `src/service/message_store.rs`: the most interesting logic in the repo (dual-backend routing plus the hand-rolled 2PC) and the source of two `cafe-full-16` items.
6. `config/schema_lists/**/*.ron`: the endpoint surface, roles, and MCP tool descriptions, all in one place. Start with `030_app_admin/030_app_admin_api.ron` (10 of the 26 endpoints).

**Commands:**
```bash
export PATH=/opt/homebrew/bin:$PATH          # cargo is not on the default PATH here;
                                             # ~/.zshenv references a missing ~/.cargo/env
cargo check                                  # ~fast, warm target/ present
cargo check --features s3-sync               # CI parity path (CI adds acme,cert-s3-sync)
cargo clippy --all-targets                   # AGENTS.md:32 says run after every change
cargo test                                   # passes trivially: zero tests exist
cargo audit                                  # 6 advisories, see cafe-full-21
./scripts/utils/regenerate_endpoints.sh      # needs endpoint-gen >= 1.10.1 on PATH;
                                             # rewrites src/codegen/model.rs AND docs/
grep -rn "persist: *true" src/               # the 6 tables a schema change can break
grep -c 'name = "endpoint-libs"' Cargo.lock  # must be 1 (AGENTS.md:22)
```

**Surprises about the layout:**
- **Two role systems that must not be confused.** `UserRole` (0-6, connection-level, gates endpoints, derived from memberships by `recompute_user_role_from_memberships`) and `AppMemberRole` (Owner/Admin/Support, per-app, checked in handlers). Endpoint `roles:` in the RON gate the *former*; the `ensure_*` helpers gate the latter. Both are needed and neither is sufficient.
- **`UserRole::HoneyAuth = 6` is load-bearing across crates.** `honey_id-types 2.0.0` hardcodes `const ROLES: &[u32] = &[6]` for the three `Receive*` callbacks (`src/types/generated.rs:1800,1853,1918`). Reordering variants in `config/enums.ron` would silently re-gate them. Nothing enforces this; it is currently correct.
- **Two auth registration paths with different consequences.** `add_auth_endpoint` (Init, ApiKeyConnect, AppConnect) runs at WebSocket handshake from the `Sec-WebSocket-Protocol` header and does **not** create an MCP tool. `add_handler` does. This is the distinction `cafe-full-15` is about.
- **`generated/` is gitignored but `src/codegen/model.rs` is committed**; `regenerate_endpoints.sh:5` copies one to the other. Never hand-edit `src/codegen/`.
- **`AppConnectionRegistry` / `UserConnectionRegistry` exist only because endpoint-libs cannot attach context to a connection.** Both carry a `TODO` saying so. Treat them as scaffolding, not as design.
- **`migration/` is a workspace member that depends on the main crate** with `features = ["s3-sync"]`, and it re-declares the `worktable!` schemas at both versions. Changing a schema means editing it in two places.
- **The working tree was clean at `feef288`** when reviewed, but this repo has had uncommitted work before. Check `git status` before touching anything.
