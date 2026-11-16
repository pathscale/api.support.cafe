use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use endpoint_libs::libs::log::{FileLoggingConfig, LogLevel, LoggingConfig, setup_logging};
use nago_rustls::rustls;
use serde_json::{Value, json};
use worktable::prelude::SelectQueryExecutor;

use super::{APP, TestServer, acceptance_root, expect_code, response, rpc, seed_app, success};
use crate::codegen::model::EnumErrorCode;
use crate::config::{Config, S3Config};
use crate::db::schema::visitor_credential::VisitorCredentialWorkTable;
use crate::db::tables::Tables;
use crate::service::log::LogService;

const CONTACT: &str = "provider visitor qualification fixture";

#[derive(serde::Deserialize)]
struct ProviderFile {
    s3: FixtureStore,
}

#[derive(serde::Deserialize)]
struct FixtureStore {
    endpoint: String,
    bucket_name: String,
    prefix: String,
    access_key: String,
    secret_key: String,
}

fn provider_config(var: &str) -> Config {
    let path = PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("set {var}")));
    // Read only the S3 section. The fixture uses local defaults for all other services.
    let input: ProviderFile = ::config::Config::builder()
        .add_source(::config::File::from(path))
        .build()
        .and_then(|config| config.try_deserialize())
        .unwrap_or_else(|_| panic!("invalid provider configuration in {var}"));
    let s3 = input.s3;
    assert!(!s3.access_key.is_empty() && !s3.secret_key.is_empty());
    assert!(
        !s3.bucket_name.is_empty()
            && s3.bucket_name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
            }),
        "expected an explicit synthetic S3 bucket name"
    );
    Config {
        s3: S3Config {
            endpoint: s3.endpoint,
            bucket_name: s3.bucket_name,
            prefix: s3.prefix,
            access_key: Some(s3.access_key),
            secret_key: Some(s3.secret_key),
            ..S3Config::default()
        },
        ..Config::default()
    }
}

#[derive(PartialEq, Eq)]
enum Operation {
    ReadManifest,
    WriteManifest,
    WriteChunk,
    Other,
}

struct RequestOutcome {
    operation: Operation,
    status: Option<u16>,
}

struct ProviderTrace {
    manifest_path: String,
    chunk_prefix: String,
    outcomes: Arc<Mutex<Vec<RequestOutcome>>>,
}

impl ureq::Middleware for ProviderTrace {
    fn handle(
        &self,
        request: ureq::Request,
        next: ureq::MiddlewareNext<'_>,
    ) -> Result<ureq::Response, ureq::Error> {
        let operation = match url::Url::parse(request.url()) {
            Ok(url) if url.path() == self.manifest_path.as_str() => match request.method() {
                "GET" => Operation::ReadManifest,
                "PUT" => Operation::WriteManifest,
                _ => Operation::Other,
            },
            Ok(url)
                if request.method() == "PUT"
                    && url.path().starts_with(self.chunk_prefix.as_str()) =>
            {
                Operation::WriteChunk
            }
            _ => Operation::Other,
        };
        let result = next.handle(request);
        let status = match &result {
            Ok(response) => Some(response.status()),
            Err(ureq::Error::Status(status, _)) => Some(*status),
            Err(ureq::Error::Transport(_)) => None,
        };
        // Retain method and status only. The request URL contains a signature.
        self.outcomes
            .lock()
            .unwrap()
            .push(RequestOutcome { operation, status });
        result
    }
}

fn provider_agent(tls: Arc<rustls::ClientConfig>, trace: ProviderTrace) -> ureq::Agent {
    ureq::builder()
        .https_only(true)
        .redirects(0)
        .try_proxy_from_env(false)
        .timeout(Duration::from_secs(30))
        .tls_config(tls)
        .middleware(trace)
        .build()
}

fn check_history(server: &TestServer, resume: &str, visitor: &str, session: &str) {
    let (mut widget, reply) = server.auth(resume, 20000);
    let resumed = success(&reply);
    assert!(resumed["userPublicId"].as_str() == Some(visitor));
    assert!(resumed.get("visitorSecret").is_some_and(Value::is_null));
    let list = rpc(&mut widget, 20006, json!({}));
    let sessions = success(&list)["data"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0]["sessionId"].as_str() == Some(session));
    let messages = rpc(&mut widget, 20003, json!({ "sessionId": session }));
    let messages = success(&messages)["data"].as_array().unwrap();
    assert!(messages.iter().any(|row| {
        row["content"]
            .as_str()
            .is_some_and(|text| text.contains(CONTACT))
    }));
}

#[test]
#[ignore = "requires a real isolated S3 prefix and a manifest PUT denial for the fault principal"]
fn visitor_credential_s3_provider_acceptance() {
    assert!(
        std::env::var_os("RUST_LOG").is_none(),
        "unset RUST_LOG to keep signed requests out of logs"
    );
    let mut writer = provider_config("VISITOR_ACCEPTANCE_S3_CONFIG");
    let mut fault = provider_config("VISITOR_ACCEPTANCE_S3_FAULT_CONFIG");
    assert!(writer.s3.access_key != fault.s3.access_key);
    let endpoint =
        url::Url::parse(&writer.s3.endpoint).unwrap_or_else(|_| panic!("invalid S3 endpoint"));
    assert!(
        endpoint.scheme() == "https"
            && endpoint.host().is_some()
            && endpoint.path() == "/"
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
            && endpoint.query().is_none()
            && endpoint.fragment().is_none(),
        "expected a bare HTTPS provider endpoint"
    );
    assert!(
        writer.s3.endpoint == fault.s3.endpoint
            && writer.s3.bucket_name == fault.s3.bucket_name
            && writer.s3.prefix == fault.s3.prefix,
        "both configurations must use the same provider destination"
    );
    assert!(
        writer.s3.prefix.starts_with("visitor-acceptance-")
            && writer.s3.prefix.len() > "visitor-acceptance-".len()
            && writer
                .s3
                .prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "use a unique, empty visitor-acceptance-* prefix in a synthetic bucket"
    );

    let owner = acceptance_root();
    assert!(
        owner.join("OWNER.md").is_file(),
        "fixture directory needs an owner"
    );
    let root = tempfile::Builder::new()
        .prefix("s3-provider-")
        .tempdir_in(owner)
        .expect("create owned fixture directory");
    writer.database.path = root.path().join("writer");
    fault.database.path = root.path().join("fault-cold");
    let logs = setup_logging(LoggingConfig {
        level: LogLevel::Info,
        file_config: Some(FileLoggingConfig {
            path: root.path().to_path_buf(),
            file_prefix: Some("fixture".into()),
            rotation: None,
        }),
        otel_config: Default::default(),
    })
    .unwrap();
    let log_guards = logs.log_guards;
    let log = Arc::new(LogService::new(logs.reload_handle, LogLevel::Info));

    let tls = Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(ps_rustcrypto::provider()))
            .with_safe_default_protocol_versions()
            .expect("configure ChaCha TLS protocol versions")
            .with_root_certificates(rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            })
            .with_no_client_auth(),
    );
    let credential_path = format!(
        "/{}/{}/{}",
        writer.s3.bucket_name,
        writer.s3.prefix,
        VisitorCredentialWorkTable::name_snake_case(),
    );
    let writer_trace = Arc::new(Mutex::new(Vec::new()));
    let fault_trace = Arc::new(Mutex::new(Vec::new()));
    let writer_agent = provider_agent(
        tls.clone(),
        ProviderTrace {
            manifest_path: format!("{credential_path}/manifest.v1"),
            chunk_prefix: format!("{credential_path}/chunks/"),
            outcomes: writer_trace.clone(),
        },
    );
    let fault_agent = provider_agent(
        tls,
        ProviderTrace {
            manifest_path: format!("{credential_path}/manifest.v1"),
            chunk_prefix: format!("{credential_path}/chunks/"),
            outcomes: fault_trace.clone(),
        },
    );

    nagoya::block_on(async {
        let db =
            Tables::new_with_s3_agent(writer.database.clone(), &writer.s3, writer_agent.clone())
                .await
                .unwrap_or_else(|_| panic!("could not open writer fixture tables"));
        assert!(
            db.app_config_table
                .select_all()
                .limit(1)
                .execute()
                .unwrap()
                .is_empty()
        );
        assert!(
            db.chat_session_table
                .select_all()
                .limit(1)
                .execute()
                .unwrap()
                .is_empty()
        );
        assert!(
            db.visitor_credential_table
                .select_all()
                .limit(1)
                .execute()
                .unwrap()
                .is_empty()
        );
        seed_app(&db, APP, true).await;
        db.migrate_visitor_credentials()
            .await
            .unwrap_or_else(|_| panic!("could not persist fixture migration marker"));
        db.wait_for_ops()
            .await
            .unwrap_or_else(|_| panic!("could not persist fixture app"));
    });

    let mut server =
        TestServer::start_with_config(writer.clone(), log.clone(), writer_agent.clone());
    writer_trace.lock().unwrap().clear();
    let new_offer = format!("0appconnect,1{APP}");
    let (mut widget, reply) = server.auth(&new_offer, 20000);
    let issued = success(&reply);
    let visitor = issued["userPublicId"].as_str().unwrap();
    let proof = issued["visitorSecret"].as_str().unwrap();
    assert!(proof.len() == 64 && proof.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(
        writer_trace.lock().unwrap().iter().any(|outcome| {
            outcome.operation == Operation::WriteManifest
                && matches!(outcome.status, Some(200..=299))
        }),
        "issuance must publish the credential manifest before returning a secret"
    );
    let resume = format!("{new_offer},2{visitor},3{proof}");
    let contact = rpc(
        &mut widget,
        20009,
        json!({"name": "Visitor", "email": "visitor@example.invalid", "message": CONTACT}),
    );
    let session = success(&contact)["sessionId"].as_str().unwrap();
    drop(widget);
    server.shutdown();

    assert!(!fault.database.path.exists());
    let mut server = TestServer::start_with_config(fault.clone(), log.clone(), fault_agent);
    check_history(&server, &resume, visitor, session);
    assert!(
        fault_trace.lock().unwrap().iter().any(|outcome| {
            outcome.operation == Operation::ReadManifest && outcome.status == Some(200)
        }),
        "fault principal must be able to read the committed manifest"
    );
    fault_trace.lock().unwrap().clear();
    let (mut failed, reply) = server.auth(&new_offer, 20000);
    expect_code(&reply, EnumErrorCode::InternalError as u32);
    assert!(reply.get("params") == Some(&json!({"kind": "InternalError"})));
    assert!(response(&mut failed).is_none());
    drop(failed);
    server.shutdown();
    {
        let outcomes = fault_trace.lock().unwrap();
        let denied = |outcome: &RequestOutcome| {
            outcome.operation == Operation::WriteManifest && outcome.status == Some(403)
        };
        assert!(
            outcomes.iter().any(denied),
            "provider must deny the manifest PUT"
        );
        assert!(
            outcomes.iter().any(|outcome| {
                outcome.operation == Operation::WriteChunk
                    && matches!(outcome.status, Some(200..=299))
            }),
            "provider must accept the chunk PUT before denying publication"
        );
        assert!(
            outcomes.iter().all(|outcome| {
                if outcome.operation == Operation::WriteManifest {
                    denied(outcome)
                } else {
                    matches!(outcome.status, Some(200..=299))
                }
            }),
            "the manifest PUT must be the only failed provider request"
        );
    }

    writer.database.path = root.path().join("verify-cold");
    assert!(!writer.database.path.exists());
    writer_trace.lock().unwrap().clear();
    let mut server = TestServer::start_with_config(writer.clone(), log, writer_agent.clone());
    check_history(&server, &resume, visitor, session);
    server.shutdown();
    assert!(
        writer_trace.lock().unwrap().iter().any(|outcome| {
            outcome.operation == Operation::ReadManifest && outcome.status == Some(200)
        }),
        "cold open must read the committed provider manifest"
    );

    writer.database.path = root.path().join("count-cold");
    assert!(!writer.database.path.exists());
    nagoya::block_on(async {
        let db = Tables::new_with_s3_agent(writer.database.clone(), &writer.s3, writer_agent)
            .await
            .unwrap_or_else(|_| panic!("could not verify cold credential table"));
        let rows = db.visitor_credential_table.select_all().execute().unwrap();
        assert_eq!(
            rows.len(),
            2,
            "only the marker and the first visitor may persist"
        );
    });

    drop(log_guards);
    let logs = std::fs::read_to_string(root.path().join("fixture.log")).unwrap();
    assert!(!logs.contains(proof), "visitor proof was logged");
    assert!(
        !logs.to_ascii_lowercase().contains("x-amz-signature"),
        "signed provider URL was logged"
    );
    for s3 in [&writer.s3, &fault.s3] {
        for value in [&s3.access_key, &s3.secret_key].into_iter().flatten() {
            assert!(
                !logs.contains(value.as_str()),
                "provider credential was logged"
            );
        }
    }
    root.close().expect("remove local fixture caches and logs");
}
