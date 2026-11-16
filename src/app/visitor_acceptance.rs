use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
#[cfg(not(feature = "s3-sync"))]
use std::process::{Child, Command};
use std::thread::{self, JoinHandle};
use std::time::Instant;

#[cfg(not(feature = "s3-sync"))]
use endpoint_libs::libs::log::{FileLoggingConfig, LogLevel, LoggingConfig, setup_logging};
use futures::channel::oneshot;
#[cfg(not(feature = "s3-sync"))]
use honey_id_types::handlers::convenience_utils::token_management::TokenStorage;
#[cfg(not(feature = "s3-sync"))]
use honey_id_types::handlers::convenience_utils::user_management::{
    CreateUserInfo, DeleteUserInfo, UserStorage,
};
use serde_json::{Value, json};
#[cfg(not(feature = "s3-sync"))]
use worktable::prelude::SelectQueryExecutor;

use super::*;
#[cfg(not(feature = "s3-sync"))]
use crate::codegen::model::EnumErrorCode;
use crate::db::schema::app_config::AppConfigRow;
#[cfg(not(feature = "s3-sync"))]
use crate::db::schema::chat_session::ChatSessionRow;
#[cfg(not(feature = "s3-sync"))]
use crate::db::schema::visitor_credential::{VisitorCredentialWorkTable, reservation};
#[cfg(not(feature = "s3-sync"))]
use crate::id_types::NanoId;
use crate::id_types::PackedNanoId;

#[cfg(feature = "s3-sync")]
mod s3;

const APP: &str = "VisitorTestApp01";
#[cfg(not(feature = "s3-sync"))]
const OTHER_APP: &str = "VisitorTestApp02";
#[cfg(not(feature = "s3-sync"))]
const INACTIVE_APP: &str = "VisitorTestApp03";
#[cfg(not(feature = "s3-sync"))]
const LEGACY: &str = "LegacyVisitor001";
#[cfg(not(feature = "s3-sync"))]
const ACCOUNT: &str = "AccountFixture01";
#[cfg(not(feature = "s3-sync"))]
const DELETED_ACCOUNT: &str = "DeletedAccount01";
#[cfg(not(feature = "s3-sync"))]
const INIT_TOKEN: &str = "InitFixtureTok01";
#[cfg(not(feature = "s3-sync"))]
const LEGACY_SESSION: &str = "LegacySession001";
#[cfg(not(feature = "s3-sync"))]
const ACCOUNT_SESSION: &str = "AccountSession01";
#[cfg(not(feature = "s3-sync"))]
const FORBIDDEN: u32 = EnumErrorCode::Forbidden as u32;

fn packed(value: &str) -> PackedNanoId {
    PackedNanoId::pack(&value.parse().unwrap()).unwrap()
}

struct TestServer {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    #[cfg(not(feature = "s3-sync"))]
    fn start(path: PathBuf, log: Arc<LogService>, issued_id: Option<NanoId>) -> Self {
        let mut config = Config::default();
        config.database.path = path;
        Self::start_with_config(config, log, issued_id)
    }

    fn start_with_config(
        mut config: Config,
        log: Arc<LogService>,
        #[cfg(not(feature = "s3-sync"))] issued_id: Option<NanoId>,
        #[cfg(feature = "s3-sync")] s3_agent: ureq::Agent,
    ) -> Self {
        let address = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap();
        config.server.address = address.to_string();
        let (stop, stopped) = oneshot::channel();
        let thread = thread::spawn(move || {
            let (server, db) = nagoya::block_on(async {
                #[cfg(not(feature = "s3-sync"))]
                let app = App::init(config.clone(), log).await.unwrap();
                #[cfg(feature = "s3-sync")]
                let app = {
                    let db =
                        Tables::new_with_s3_agent(config.database.clone(), &config.s3, s3_agent)
                            .await
                            .unwrap_or_else(|_| panic!("could not open fixture tables"));
                    App::init_with_tables(config.clone(), log, Arc::new(db))
                        .await
                        .unwrap_or_else(|_| panic!("could not initialize fixture app"))
                };
                #[cfg(not(feature = "s3-sync"))]
                if let Some(user_pub_id) = issued_id {
                    assert!(
                        app.ctx
                            .user_service
                            .create_or_update_user(CreateUserInfo {
                                username: "collision fixture".into(),
                                user_pub_id,
                                app_pub_id: None,
                            })
                            .await
                            .is_err(),
                        "an issued visitor ID must not become a local account"
                    );
                    assert_eq!(
                        app.ctx
                            .db
                            .chat_session_table
                            .select_all()
                            .execute()
                            .unwrap()
                            .len(),
                        4
                    );
                }
                #[cfg(not(feature = "s3-sync"))]
                app.ctx
                    .token_storage
                    .store_token(
                        UserPublicId::from(ACCOUNT.parse::<NanoId>().unwrap()),
                        INIT_TOKEN.parse().unwrap(),
                    )
                    .await
                    .unwrap();
                let mut server = WebsocketServer::new(config.server.into());
                crate::admission::install(&mut server);
                app.register_handlers(&mut server).await;
                (server, app.ctx.db.clone())
            });
            server
                .listen_until(async {
                    let _ = stopped.await;
                })
                .unwrap();
            // Flush the chat fixture for restart, without a test-only credential flush.
            nagoya::block_on(async {
                db.chat_session_table.wait_for_ops().await.unwrap();
                db.support_message_table.wait_for_ops().await.unwrap();
            });
        });
        Self {
            address,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    fn shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }

    fn connect(&self, offer: &str, pipelined: Option<Value>) -> (BufReader<TcpStream>, String) {
        // Remote hydration and a provider write rejection can take longer
        // than the disk-only fixture.
        let (startup, io) = if cfg!(feature = "s3-sync") {
            (90, 90)
        } else {
            (10, 15)
        };
        let deadline = Instant::now() + Duration::from_secs(startup);
        let mut stream = loop {
            match TcpStream::connect(self.address) {
                Ok(stream) => break stream,
                Err(err) if Instant::now() >= deadline => panic!("loopback listener: {err}"),
                Err(_) => thread::sleep(Duration::from_millis(20)),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(io)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(io)))
            .unwrap();
        let mut request = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: {offer}\r\n\r\n",
            self.address
        ).into_bytes();
        if let Some(frame) = pipelined {
            request.extend(masked_frame(&frame));
        }
        stream.write_all(&request).unwrap();
        let mut stream = BufReader::new(stream);
        let mut head = String::new();
        loop {
            let mut line = String::new();
            assert_ne!(stream.read_line(&mut line).unwrap(), 0);
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        (stream, head)
    }

    fn auth(&self, offer: &str, method: u32) -> (BufReader<TcpStream>, Value) {
        let (mut stream, head) = self.connect(offer, None);
        assert!(head.starts_with("HTTP/1.1 101 "));
        let selected = format!(
            "sec-websocket-protocol: {}\r\n",
            offer.split(',').next().unwrap()
        );
        assert!(head.to_ascii_lowercase().contains(&selected));
        let reply = response(&mut stream).expect("authentication frame");
        assert_eq!(reply["method"].as_u64(), Some(u64::from(method)));
        (stream, reply)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn masked_frame(value: &Value) -> Vec<u8> {
    let payload = serde_json::to_vec(value).unwrap();
    let mut frame = vec![0x81];
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else {
        frame.push(0x80 | 126);
        frame.extend(u16::try_from(payload.len()).unwrap().to_be_bytes());
    }
    let mask = [11, 23, 37, 41];
    frame.extend(mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(i, byte)| byte ^ mask[i % 4]),
    );
    frame
}

fn response(stream: &mut BufReader<TcpStream>) -> Option<Value> {
    let mut first = [0; 1];
    if stream.read(&mut first).unwrap() == 0 || first[0] == 0x88 {
        return None;
    }
    assert!(
        first[0] == 0x81 || first[0] == 0x82,
        "expected a JSON data frame"
    );
    stream.read_exact(&mut first).unwrap();
    let length = match first[0] {
        126 => {
            let mut length = [0; 2];
            stream.read_exact(&mut length).unwrap();
            u16::from_be_bytes(length).into()
        }
        length @ 0..=125 => usize::from(length),
        _ => panic!("unexpected frame length"),
    };
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload).unwrap();
    Some(serde_json::from_slice(&payload).expect("valid response JSON"))
}

fn rpc(stream: &mut BufReader<TcpStream>, method: u32, params: Value) -> Value {
    let frame = masked_frame(&json!({"method": method, "seq": method, "params": params}));
    stream.get_mut().write_all(&frame).unwrap();
    let reply = response(stream).expect("RPC response");
    assert_eq!(reply["method"].as_u64(), Some(u64::from(method)));
    assert_eq!(reply["seq"].as_u64(), Some(u64::from(method)));
    reply
}

fn expect_code(reply: &Value, code: u32) {
    assert_eq!(reply["code"].as_u64(), Some(u64::from(code)));
}

fn success(reply: &Value) -> &Value {
    assert!(reply.get("code").is_none(), "expected a success response");
    &reply["params"]
}

#[cfg(not(feature = "s3-sync"))]
fn account_info(id: &str) -> CreateUserInfo {
    CreateUserInfo {
        username: "account fixture".into(),
        user_pub_id: id.parse().unwrap(),
        app_pub_id: None,
    }
}

fn acceptance_root() -> PathBuf {
    PathBuf::from(std::env::var_os("VISITOR_ACCEPTANCE_DIR").expect("owned acceptance directory"))
}

async fn seed_app(db: &Tables, app: &str, active: bool) {
    db.app_config_table
        .insert(AppConfigRow {
            id: db.app_config_table.get_next_pk().into(),
            public_id: packed(app),
            tg_bot_token: String::new(),
            app_name: None,
            active,
            message_persistence_enabled: true,
            created_at: 1,
        })
        .await
        .unwrap();
}

#[cfg(not(feature = "s3-sync"))]
#[test]
#[ignore = "requires an owned visitor acceptance directory"]
fn seed_visitor_migration_fixture() {
    let path = acceptance_root().join("db");
    std::fs::create_dir(&path).unwrap();
    nagoya::block_on(seed_legacy_store(path));
}

#[cfg(not(feature = "s3-sync"))]
async fn seed_legacy_store(path: PathBuf) {
    let db = Arc::new(
        Tables::new(crate::config::DatabaseConfig { path })
            .await
            .unwrap(),
    );
    for (app, active) in [(APP, true), (OTHER_APP, true), (INACTIVE_APP, false)] {
        seed_app(&db, app, active).await;
    }
    UserService::new(db.clone())
        .create_or_update_user(account_info(ACCOUNT))
        .await
        .unwrap();
    for (owner, session) in [(LEGACY, LEGACY_SESSION), (ACCOUNT, ACCOUNT_SESSION)] {
        db.chat_session_table
            .insert(ChatSessionRow {
                id: db.chat_session_table.get_next_pk().into(),
                session_id: packed(session),
                app_public_id: packed(APP),
                user_pub_id: packed(owner),
                created_at: 1,
                closed_at: None,
            })
            .await
            .unwrap();
    }
    db.wait_for_ops().await.unwrap();
    // Model a previous attempt that committed some reservations before exiting.
    db.visitor_credential_table
        .insert(reservation(packed(LEGACY)))
        .await
        .unwrap();
    db.wait_for_visitor_ops().await.unwrap();
    assert!(db.require_visitor_migration().await.is_err());
}

#[cfg(not(feature = "s3-sync"))]
async fn check_migrated_store(path: PathBuf) {
    let db = Arc::new(
        Tables::new(crate::config::DatabaseConfig { path })
            .await
            .unwrap(),
    );
    db.require_visitor_migration().await.unwrap();
    assert_eq!(
        db.chat_session_table.select_all().execute().unwrap().len(),
        2
    );
    for owner in [LEGACY, ACCOUNT] {
        let row = db.visitor_credential_table.select(packed(owner)).unwrap();
        assert!(row.app_public_id.is_none() && row.secret_digest.is_none());
    }
    let users = UserService::new(db.clone());
    assert!(
        users
            .create_or_update_user(account_info(LEGACY))
            .await
            .is_err()
    );
    users
        .create_or_update_user(account_info(ACCOUNT))
        .await
        .unwrap();
    users
        .create_or_update_user(account_info(DELETED_ACCOUNT))
        .await
        .unwrap();
    users
        .delete_user(DeleteUserInfo {
            user_pub_id: DELETED_ACCOUNT.parse().unwrap(),
            app_pub_id: None,
        })
        .await
        .unwrap();
    assert!(
        db.visitor_credential_table
            .select(packed(DELETED_ACCOUNT))
            .is_some()
    );
    assert!(
        users
            .create_or_update_user(account_info(DELETED_ACCOUNT))
            .await
            .is_err()
    );
    db.wait_for_ops().await.unwrap();
}

#[cfg(not(feature = "s3-sync"))]
struct IssuerProcess(Child);

#[cfg(not(feature = "s3-sync"))]
impl Drop for IssuerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(not(feature = "s3-sync"))]
#[test]
#[ignore = "started by the visitor acceptance test in a child process"]
fn run_visitor_issuer() {
    let address = std::env::var("VISITOR_ISSUER_ADDRESS").expect("issuer address");
    let (server, _log_guards) = nagoya::block_on(async {
        let logs = setup_logging(LoggingConfig {
            level: LogLevel::Debug,
            file_config: None,
            otel_config: Default::default(),
        })
        .unwrap();
        let log = Arc::new(LogService::new(logs.reload_handle, LogLevel::Debug));
        let mut config = Config::default();
        config.database.path = acceptance_root().join("db");
        config.server.address = address;
        let app = App::init(config.clone(), log).await.unwrap();
        let mut server = WebsocketServer::new(config.server.into());
        crate::admission::install(&mut server);
        app.register_handlers(&mut server).await;
        (server, logs.log_guards)
    });
    server.listen().unwrap();
}

#[cfg(not(feature = "s3-sync"))]
fn issue_before_process_kill(offer: &str) -> String {
    let address = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap();
    let mut issuer = IssuerProcess(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "app::visitor_acceptance::run_visitor_issuer",
                "--ignored",
                "--exact",
                "--nocapture",
            ])
            .env("VISITOR_ISSUER_ADDRESS", address.to_string())
            .spawn()
            .unwrap(),
    );
    let server = TestServer {
        address,
        stop: None,
        thread: None,
    };
    let (_socket, reply) = server.auth(offer, 20000);
    issuer.0.kill().unwrap();
    assert!(!issuer.0.wait().unwrap().success());
    let issued = success(&reply);
    let id = issued["userPublicId"].as_str().unwrap();
    let secret = issued["visitorSecret"].as_str().unwrap();
    format!("{offer},2{id},3{secret}")
}

#[cfg(not(feature = "s3-sync"))]
#[test]
#[ignore = "requires an owned visitor acceptance directory and a loopback listener"]
fn visitor_credential_loopback_acceptance() {
    let root = acceptance_root();
    let path = root.join("db");
    let _ = nago_rustls::rustls::crypto::ring::default_provider().install_default();
    let logs = setup_logging(LoggingConfig {
        level: LogLevel::Debug,
        file_config: Some(FileLoggingConfig {
            path: root.clone(),
            file_prefix: Some("visitor-wire".into()),
            rotation: None,
        }),
        otel_config: Default::default(),
    })
    .unwrap();
    let log_guards = logs.log_guards;
    let log = Arc::new(LogService::new(logs.reload_handle, LogLevel::Debug));
    nagoya::block_on(check_migrated_store(path.clone()));

    let new_offer = format!("0appconnect,1{APP}");
    let mut server = TestServer::start(path.clone(), log.clone(), None);
    let (mut socket, reply) = server.auth(&new_offer, 20000);
    let issued = success(&reply);
    let visitor = issued["userPublicId"].as_str().unwrap();
    let secret = issued["visitorSecret"].as_str().unwrap();
    assert_eq!(visitor.len(), 16);
    assert!(visitor != LEGACY && visitor != ACCOUNT);
    assert!(secret.len() == 64 && secret.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let resume = format!("{new_offer},2{visitor},3{secret}");

    expect_code(
        &rpc(&mut socket, 20001, json!({"userPubId": LEGACY})),
        FORBIDDEN,
    );
    let created = rpc(&mut socket, 20001, json!({"userPubId": visitor}));
    let session = success(&created)["sessionId"].as_str().unwrap();
    let contact = rpc(
        &mut socket,
        20009,
        json!({
            "name": "Visitor",
            "email": "visitor@example.invalid",
            "message": "contact fixture"
        }),
    );
    let contact_session = success(&contact)["sessionId"].as_str().unwrap();
    drop(socket);
    server.shutdown();

    let mut server = TestServer::start(path.clone(), log.clone(), Some(visitor.parse().unwrap()));
    let (mut socket, reply) = server.auth(&resume, 20000);
    let resumed = success(&reply);
    assert_eq!(resumed["userPublicId"], visitor);
    assert!(resumed.get("visitorSecret").is_some_and(Value::is_null));

    let list = rpc(&mut socket, 20006, json!({}));
    let sessions = success(&list)["data"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().any(|row| row["sessionId"] == session));
    assert!(
        sessions
            .iter()
            .any(|row| row["sessionId"] == contact_session)
    );
    let messages = rpc(&mut socket, 20003, json!({"sessionId": contact_session}));
    let messages = success(&messages)["data"].as_array().unwrap();
    assert!(messages.iter().any(|row| {
        row["content"]
            .as_str()
            .is_some_and(|text| text.contains("contact fixture"))
    }));
    drop(socket);

    let wrong_prefix = if secret.starts_with('a') { 'b' } else { 'a' };
    let wrong = format!("{wrong_prefix}{}", &secret[1..]);
    for offer in [
        format!("{new_offer},2{visitor}"),
        format!("{new_offer},3{secret}"),
        format!("{new_offer},2{visitor},3{wrong}"),
        format!("{new_offer},2{LEGACY},3{secret}"),
        format!("{new_offer},2{ACCOUNT},3{secret}"),
        format!("0appconnect,1{OTHER_APP},2{visitor},3{secret}"),
    ] {
        let (_, reply) = server.auth(&offer, 20000);
        expect_code(&reply, FORBIDDEN);
    }
    for app in [INACTIVE_APP, "UnknownDesk00001"] {
        let (_, reply) = server.auth(&format!("0appconnect,1{app}"), 20000);
        expect_code(&reply, EnumErrorCode::NotFound as u32);
    }

    for offer in [
        format!("3{secret},{new_offer},2{visitor}"),
        format!("3{secret}"),
        secret.to_owned(),
        format!("{resume},3{secret}"),
        format!("{new_offer},2{visitor},3invalid"),
        format!("{new_offer}\r\nSec-WebSocket-Protocol: 2{visitor},3{secret}"),
    ] {
        let (_, head) = server.connect(&offer, None);
        assert!(head.starts_with("HTTP/1.1 400 "));
        assert!(!head.contains(secret));
    }

    let (mut rejected, head) = server.connect(
        &format!("{new_offer},2{visitor},3{wrong}"),
        Some(json!({"method": 20001, "seq": 7, "params": {"userPubId": visitor}})),
    );
    assert!(head.starts_with("HTTP/1.1 101 "));
    expect_code(&response(&mut rejected).unwrap(), FORBIDDEN);
    assert!(
        response(&mut rejected).is_none(),
        "rejected socket ran a pipelined RPC"
    );

    let (mut other, reply) = server.auth(&new_offer, 20000);
    assert_ne!(success(&reply)["userPublicId"], visitor);
    for (method, params) in [
        (
            20002,
            json!({"sessionId": session, "content": "forged message"}),
        ),
        (20003, json!({"sessionId": session})),
        (20004, json!({"sessionId": session})),
        (20005, json!({"sessionId": session})),
    ] {
        expect_code(&rpc(&mut other, method, params), FORBIDDEN);
    }

    let (mut account, reply) = server.auth(&format!("0init,1{INIT_TOKEN}"), 10000);
    assert_eq!(success(&reply)["userId"], ACCOUNT);
    let list = rpc(&mut account, 20006, json!({}));
    let rows = success(&list)["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["sessionId"], ACCOUNT_SESSION);
    drop(account);
    drop(other);
    drop(rejected);
    server.shutdown();

    let crash_resume = issue_before_process_kill(&new_offer);
    let mut server = TestServer::start(path.clone(), log, None);
    let (socket, reply) = server.auth(&crash_resume, 20000);
    assert!(success(&reply)["visitorSecret"].is_null());
    drop(socket);

    // Make the local fsync barrier fail after startup without using a real store.
    let credentials = path.join(VisitorCredentialWorkTable::name_snake_case());
    std::fs::rename(&credentials, path.join("faulted-visitor-table")).unwrap();
    let (mut failed, reply) = server.auth(&new_offer, 20000);
    expect_code(&reply, EnumErrorCode::InternalError as u32);
    assert!(reply["params"].get("visitorSecret").is_none());
    assert!(response(&mut failed).is_none());
    drop(failed);
    server.shutdown();
    drop(log_guards);
    let logs = std::fs::read_to_string(root.join("visitor-wire.log")).unwrap();
    assert!(
        !logs.contains(secret) && !logs.contains(&wrong),
        "a synthetic visitor proof appeared in the server log"
    );
}
