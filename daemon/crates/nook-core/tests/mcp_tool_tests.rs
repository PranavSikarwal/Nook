#![cfg(unix)]

mod common;

use common::{tool_result, Harness};
use nook_core::config::McpServerConfig;
use nook_core::protocol::DaemonMessage;
use nook_core::protocol::ToolSourceState;
use nook_core::providers::mcp::{McpProvider, StartupLimits};
use nook_core::providers::Provider;
use nook_core::sandbox::Sandbox;
use nook_core::secrets::SecretStore;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

const ECHO: &str = "mcp:fake:echo";

fn example_binary() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join("fake_mcp_server")
}

fn server(name: &str, mode: &str) -> McpServerConfig {
    let binary = example_binary();
    McpServerConfig {
        name: name.to_string(),
        command: binary.display().to_string(),
        args: vec![mode.to_string()],
        env: BTreeMap::new(),
        env_secrets: BTreeMap::new(),
        network: false,
        read_paths: vec![binary.parent().unwrap().display().to_string()],
        write_paths: vec![],
        timeout_seconds: 60,
    }
}

fn docs_server(record_dir: &std::path::Path) -> McpServerConfig {
    let mut config = server("google-docs", "docs");
    config.network = true;
    config.write_paths = vec![record_dir.display().to_string()];
    config.env.insert(
        "FAKE_DOCS_RECORD_DIR".to_string(),
        record_dir.display().to_string(),
    );
    config
}

fn inserted_log_path(record_dir: &std::path::Path) -> PathBuf {
    record_dir.join("inserted.log")
}

type LogBuffer = Arc<std::sync::Mutex<Vec<u8>>>;

/// Writes this thread's tracing output to a buffer until the guard drops.
fn capture_log() -> (LogBuffer, tracing::subscriber::DefaultGuard) {
    struct LogCapture(LogBuffer);
    impl std::io::Write for LogCapture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buffer: LogBuffer = Arc::default();
    let writer = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || LogCapture(writer.clone()))
        .finish();
    (buffer, tracing::subscriber::set_default(subscriber))
}

fn log_text(buffer: &LogBuffer) -> String {
    String::from_utf8_lossy(&buffer.lock().unwrap()).to_string()
}

async fn sandbox() -> Option<Sandbox> {
    match Sandbox::detect().await {
        Ok(sandbox) => Some(sandbox),
        Err(error) => {
            assert!(
                std::env::var_os("NOOK_REQUIRE_SANDBOX").is_none(),
                "The sandbox is required but unavailable: {error}"
            );
            eprintln!("Skipping MCP test: {error}");
            None
        }
    }
}

fn quick() -> StartupLimits {
    StartupLimits {
        initialize: Duration::from_secs(2),
        list_tools: Duration::from_secs(2),
        restart_interval: Duration::from_secs(3600),
        secret_timeout: Duration::from_secs(2),
    }
}

async fn provider(configs: &[McpServerConfig], limits: StartupLimits) -> Option<McpProvider> {
    let sandbox = sandbox().await?;
    McpProvider::start(
        configs,
        &sandbox,
        limits,
        nook_core::secrets::SecretContext::default(),
    )
    .await
}

async fn harness(configs: &[McpServerConfig]) -> Option<Harness> {
    harness_with(configs, quick()).await
}

async fn harness_with(configs: &[McpServerConfig], limits: StartupLimits) -> Option<Harness> {
    let provider = provider(configs, limits).await?;
    Some(Harness::new(vec![Arc::new(provider)]).await)
}

fn names(h: &Harness) -> Vec<String> {
    let mut names: Vec<String> = h
        .state
        .providers
        .descriptors()
        .iter()
        .map(|d| d.name.clone())
        .collect();
    names.sort();
    names
}

async fn run(h: &mut Harness, chat: Uuid, call: &str, tool: &str, args: Value) -> Value {
    h.approve_and_run(chat, call, tool, &args.to_string(), "allow_once")
        .await
}

#[tokio::test]
async fn only_valid_tools_are_offered_with_clean_names() {
    let Some(h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    assert_eq!(
        names(&h),
        [
            "mcp__fake__big",
            "mcp__fake__die",
            "mcp__fake__dup_name",
            "mcp__fake__echo",
            "mcp__fake__env",
            "mcp__fake__fail",
            "mcp__fake__mixed",
            "mcp__fake__plain",
            "mcp__fake__read_file",
            "mcp__fake__slow",
            "mcp__fake__structured",
        ]
    );
    let echo = h.state.providers.descriptor(ECHO).unwrap();
    assert_eq!(echo.description, "Echo the text back. (MCP server: fake)");
    let plain = h.state.providers.descriptor("mcp:fake:plain").unwrap();
    assert_eq!(plain.description, "plain (MCP server: fake)");
    assert!(h.state.providers.contains("mcp:fake:dup.name"));
    assert!(!h.state.providers.contains("mcp:fake:dup_name"));
    for skipped in ["bad_schema", "no_schema", "has:colon"] {
        assert!(!h.state.providers.contains(&format!("mcp:fake:{skipped}")));
    }
}

#[tokio::test]
async fn a_docs_style_tool_is_listed_under_the_hyphenated_server_name() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(h) = harness(&[docs_server(&record_path)]).await else {
        return;
    };
    let mut names: Vec<_> = h
        .state
        .providers
        .descriptors()
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "mcp__google-docs__checkEnv",
            "mcp__google-docs__connect",
            "mcp__google-docs__echoEnv",
            "mcp__google-docs__insertText",
            "mcp__google-docs__readGoogleDoc",
        ]
    );
    let doc = h
        .state
        .providers
        .descriptor("mcp:google-docs:readGoogleDoc")
        .unwrap();
    assert!(
        doc.description.ends_with("(MCP server: google-docs)"),
        "expected description to end with (MCP server: google-docs), got: {}",
        doc.description
    );
}

#[tokio::test]
async fn a_docs_style_call_asks_first_and_shows_the_server_tool_and_arguments() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(mut h) = harness(&[docs_server(&record_path)]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let args = json!({
        "documentId": "doc-123",
        "text": "hello world",
    });
    let _call = h
        .execute(chat, "c1", "mcp:google-docs:insertText", &args.to_string())
        .await;

    let DaemonMessage::ApprovalRequested {
        resource_summary,
        actions,
        tool_name,
        ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };

    assert_eq!(tool_name, "mcp:google-docs:insertText");
    assert_eq!(
        resource_summary,
        "Call insertText on MCP server google-docs with {\"documentId\":\"doc-123\",\"text\":\"hello world\"}"
    );
    assert!(resource_summary.contains("google-docs"));
    assert!(resource_summary.contains("insertText"));
    assert!(resource_summary.contains("doc-123") && resource_summary.contains("hello world"));
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        ["allow_once", "allow_for_chat", "always_allow", "deny"]
    );

    let pending = h.state.pending_approvals.lock().await;
    let entry = pending.get("c1").expect("pending approval for c1");
    assert_eq!(
        entry.request.facts.get("server").map(String::as_str),
        Some("google-docs")
    );
    assert_eq!(
        entry.request.facts.get("tool").map(String::as_str),
        Some("insertText")
    );
}

#[tokio::test]
async fn an_approved_docs_write_reaches_the_server() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(mut h) = harness(&[docs_server(&record_path)]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let args = json!({
        "documentId": "doc-123",
        "text": "first line of text",
    });
    let result = run(&mut h, chat, "c1", "mcp:google-docs:insertText", args).await;

    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "ok");

    let log_file = inserted_log_path(&record_path);
    assert!(log_file.exists(), "inserted.log should exist after write");
    let content = std::fs::read_to_string(&log_file).unwrap();
    assert_eq!(content.trim(), "first line of text");
}

#[tokio::test]
async fn a_denied_docs_write_never_reaches_the_server() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(mut h) = harness(&[docs_server(&record_path)]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let args = json!({
        "documentId": "doc-123",
        "text": "should not be written",
    });
    let mut call = h
        .execute(chat, "c1", "mcp:google-docs:insertText", &args.to_string())
        .await;
    h.panel().await;

    h.decide(chat, "c1", "deny").await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Denied by the user");
    h.assert_panel_silent();

    let log_file = inserted_log_path(&record_path);
    assert!(
        !log_file.exists(),
        "inserted.log must not exist after deny: {}",
        log_file.display()
    );
}

#[tokio::test]
async fn a_docs_server_with_network_true_can_connect_out() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(mut h) = harness(&[docs_server(&record_path)]).await else {
        return;
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let chat = Uuid::new_v4();

    let result = run(
        &mut h,
        chat,
        "c1",
        "mcp:google-docs:connect",
        json!({ "port": port }),
    )
    .await;

    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "connected");
}

struct MissingSecretStore;

impl nook_core::secrets::SecretStore for MissingSecretStore {
    fn get(&self, _name: &str) -> Result<String, nook_core::secrets::SecretError> {
        Err(nook_core::secrets::SecretError::Missing)
    }
    fn set(&self, _name: &str, _value: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
    fn delete(&self, _name: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
}

struct ErrorSecretStore(nook_core::secrets::SecretError);

impl nook_core::secrets::SecretStore for ErrorSecretStore {
    fn get(&self, _name: &str) -> Result<String, nook_core::secrets::SecretError> {
        Err(self.0)
    }
    fn set(&self, _name: &str, _value: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
    fn delete(&self, _name: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
}

struct BlockingSecretStore;

impl nook_core::secrets::SecretStore for BlockingSecretStore {
    fn get(&self, _name: &str) -> Result<String, nook_core::secrets::SecretError> {
        // Warning: a read that never returns would hang the test runtime on drop, which waits for blocking threads.
        std::thread::sleep(Duration::from_secs(5));
        Err(nook_core::secrets::SecretError::Unavailable)
    }
    fn set(&self, _name: &str, _value: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
    fn delete(&self, _name: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
}

struct FailingSecretStore;

impl nook_core::secrets::SecretStore for FailingSecretStore {
    fn get(&self, _name: &str) -> Result<String, nook_core::secrets::SecretError> {
        Err(nook_core::secrets::SecretError::Unreadable)
    }
    fn set(&self, _name: &str, _value: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
    fn delete(&self, _name: &str) -> Result<(), nook_core::secrets::SecretError> {
        Ok(())
    }
}

#[tokio::test]
async fn a_secret_from_the_store_reaches_the_server_as_an_env_value() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let mut config = docs_server(&record_path);
    config.env.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_ID".to_string(),
        "fake-client-id".to_string(),
    );
    config.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "google-docs-client-secret".to_string(),
    );
    let store = Arc::new(nook_core::secrets::MemorySecretStore::default());
    store
        .set("google-docs-client-secret", "super-secret-1234")
        .unwrap();
    let secrets = nook_core::secrets::SecretContext {
        store: store.clone(),
        mask: Arc::default(),
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let provider = McpProvider::start(&[config], &sandbox, quick(), secrets)
        .await
        .expect("server starts");
    let mut h = Harness::new(vec![Arc::new(provider)]).await;
    let chat = Uuid::new_v4();

    let res1 = run(
        &mut h,
        chat,
        "c1",
        "mcp:google-docs:checkEnv",
        json!({ "name": "GOOGLE_DRIVE_MCP_CLIENT_SECRET", "expected": "super-secret-1234" }),
    )
    .await;
    assert_eq!(res1["status"], "ok");
    assert_eq!(res1["output"], "match");

    let res2 = run(
        &mut h,
        chat,
        "c2",
        "mcp:google-docs:checkEnv",
        json!({ "name": "GOOGLE_DRIVE_MCP_CLIENT_ID", "expected": "fake-client-id" }),
    )
    .await;
    assert_eq!(res2["status"], "ok");
    assert_eq!(res2["output"], "match");
}

#[tokio::test]
async fn a_missing_secret_keeps_the_server_off_and_names_the_secret() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let mut config = docs_server(&record_path);
    config.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "google-docs-client-secret".to_string(),
    );
    let secrets = nook_core::secrets::SecretContext {
        store: Arc::new(MissingSecretStore),
        mask: Arc::default(),
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) =
        McpProvider::start_reporting(&[config], &sandbox, quick(), secrets).await;
    assert!(provider.is_none());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].state, ToolSourceState::Off);
    let reason = failed[0].reason.as_ref().expect("failure reason");
    assert!(
        reason.contains("google-docs-client-secret"),
        "expected reason to name secret: {reason}"
    );
    assert!(
        reason.contains("nookctl secret set google-docs-client-secret"),
        "expected reason to contain nookctl secret set: {reason}"
    );
}

#[tokio::test]
async fn a_startup_error_that_holds_a_secret_is_scrubbed_in_the_report() {
    let mut config = server("leaky", "leak_init_error");
    config
        .env_secrets
        .insert("LEAK_VALUE".to_string(), "leak-secret-name".to_string());
    let store = Arc::new(nook_core::secrets::MemorySecretStore::default());
    store.set("leak-secret-name", "leaked-token-5678").unwrap();
    let secrets = nook_core::secrets::SecretContext {
        store: store.clone(),
        mask: Arc::default(),
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) =
        McpProvider::start_reporting(&[config], &sandbox, quick(), secrets).await;
    assert!(provider.is_none());
    assert_eq!(failed.len(), 1);
    let reason = failed[0].reason.as_ref().expect("failure reason");
    assert!(
        reason.contains("***"),
        "expected the secret to be masked in the reason: {reason}"
    );
    assert!(
        !reason.contains("leaked-token-5678"),
        "expected the reason not to hold the secret: {reason}"
    );
}

#[tokio::test]
async fn a_store_that_never_answers_keeps_the_server_off_after_the_timeout() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let mut stuck = docs_server(&record_path);
    stuck.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "stuck-secret".to_string(),
    );
    let good = server("good", "normal");
    let secrets = nook_core::secrets::SecretContext {
        store: Arc::new(BlockingSecretStore),
        mask: Arc::default(),
    };
    let limits = StartupLimits {
        secret_timeout: Duration::from_millis(50),
        ..quick()
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) =
        McpProvider::start_reporting(&[stuck, good], &sandbox, limits, secrets).await;
    let p = provider.expect("good server should start");
    let ready = p.sources();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].source_id, "mcp:good");
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].source_id, "mcp:google-docs");
    let reason = failed[0].reason.as_ref().unwrap();
    assert!(reason.contains("stuck-secret"));
    assert!(reason.contains("timed out"));
}

#[tokio::test]
async fn a_store_error_keeps_the_server_off_with_the_fixed_text() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(sandbox) = sandbox().await else {
        return;
    };
    for error in [
        nook_core::secrets::SecretError::Unavailable,
        nook_core::secrets::SecretError::Unreadable,
    ] {
        let mut config = docs_server(&record_path);
        config.env_secrets.insert(
            "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
            "google-docs-client-secret".to_string(),
        );
        let secrets = nook_core::secrets::SecretContext {
            store: Arc::new(ErrorSecretStore(error)),
            mask: Arc::default(),
        };
        let (provider, failed) =
            McpProvider::start_reporting(&[config], &sandbox, quick(), secrets).await;
        assert!(provider.is_none());
        assert_eq!(failed.len(), 1);
        let reason = failed[0].reason.as_ref().unwrap();
        assert!(
            reason.contains("google-docs-client-secret"),
            "expected reason to name secret: {reason}"
        );
        assert!(
            reason.contains(&error.to_string()),
            "expected reason to contain fixed text '{}': {reason}",
            error
        );
    }

    let mut failing_config = docs_server(&record_path);
    failing_config.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "google-docs-client-secret".to_string(),
    );
    let secrets = nook_core::secrets::SecretContext {
        store: Arc::new(FailingSecretStore),
        mask: Arc::default(),
    };
    let (provider, failed) =
        McpProvider::start_reporting(&[failing_config], &sandbox, quick(), secrets).await;
    assert!(provider.is_none());
    assert_eq!(failed.len(), 1);
    let reason = failed[0].reason.as_ref().unwrap();
    assert!(reason.contains("google-docs-client-secret"));
    assert!(reason.contains("Secret cannot be read"));
}

#[tokio::test]
async fn a_restart_with_a_missing_secret_logs_a_fixed_text_and_the_call_fails() {
    let limits = StartupLimits {
        restart_interval: Duration::ZERO,
        ..quick()
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (log, _guard) = capture_log();
    let store = Arc::new(nook_core::secrets::MemorySecretStore::default());
    store.set("restart-secret", "initial-value").unwrap();
    let secrets = nook_core::secrets::SecretContext {
        store: store.clone(),
        mask: Arc::default(),
    };
    let mut s = server("fake", "normal");
    s.env_secrets
        .insert("RESTART_VAR".to_string(), "restart-secret".to_string());
    let provider = McpProvider::start(&[s], &sandbox, limits, secrets)
        .await
        .expect("server starts");
    let mut h = Harness::new(vec![Arc::new(provider)]).await;

    run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    store.delete("restart-secret").unwrap();

    let result = run(&mut h, Uuid::new_v4(), "c2", ECHO, json!({ "text": "hi" })).await;
    assert_eq!(result["status"], "error");
    assert_eq!(result["output"], "Error: the MCP server stopped");

    let logged = log_text(&log);
    assert!(
        logged.contains("The secret \"restart-secret\" is not in the secret store."),
        "expected the fixed missing-secret text in the log: {logged}"
    );
    assert!(
        !logged.contains("initial-value"),
        "captured log must not contain the secret value: {logged}"
    );
}

#[tokio::test]
async fn an_empty_stored_secret_keeps_the_server_off_and_names_the_secret() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let mut config = docs_server(&record_path);
    config.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "google-docs-client-secret".to_string(),
    );
    let store = Arc::new(nook_core::secrets::MemorySecretStore::default());
    store.set("google-docs-client-secret", "").unwrap();
    let secrets = nook_core::secrets::SecretContext {
        store,
        mask: Arc::default(),
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) =
        McpProvider::start_reporting(&[config], &sandbox, quick(), secrets).await;
    assert!(provider.is_none());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].state, ToolSourceState::Off);
    let reason = failed[0].reason.as_ref().expect("failure reason");
    assert!(
        reason.contains("google-docs-client-secret"),
        "expected reason to name secret: {reason}"
    );
    assert!(
        reason.contains("nookctl secret set google-docs-client-secret"),
        "expected reason to contain nookctl secret set: {reason}"
    );
}

#[tokio::test]
async fn a_secret_printed_by_a_server_is_scrubbed_from_the_result() {
    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let mut config = docs_server(&record_path);
    config.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "google-docs-client-secret".to_string(),
    );
    let store = Arc::new(nook_core::secrets::MemorySecretStore::default());
    store
        .set("google-docs-client-secret", "super-secret-1234")
        .unwrap();
    let secrets = nook_core::secrets::SecretContext {
        store: store.clone(),
        mask: Arc::default(),
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let provider = McpProvider::start(&[config], &sandbox, quick(), secrets)
        .await
        .expect("server starts");
    let mut h = Harness::new(vec![Arc::new(provider)]).await;
    let chat = Uuid::new_v4();

    let result = run(
        &mut h,
        chat,
        "c1",
        "mcp:google-docs:echoEnv",
        json!({ "name": "GOOGLE_DRIVE_MCP_CLIENT_SECRET" }),
    )
    .await;
    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "***");
}

#[tokio::test]
async fn a_call_asks_first_and_returns_the_server_text() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let _call = h
        .execute(chat, "c1", ECHO, &json!({ "text": "hi" }).to_string())
        .await;

    let DaemonMessage::ApprovalRequested {
        resource_summary,
        actions,
        tool_name,
        ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };

    assert_eq!(tool_name, ECHO);
    assert_eq!(
        resource_summary,
        "Call echo on MCP server fake with {\"text\":\"hi\"}"
    );
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        ["allow_once", "allow_for_chat", "always_allow", "deny"]
    );
}

#[tokio::test]
async fn an_approved_call_returns_the_answer() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", ECHO, json!({ "text": "hi" })).await;

    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "echo: hi");
}

#[tokio::test]
async fn a_denied_call_never_reaches_the_server() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let mut call = h
        .execute(chat, "c1", ECHO, &json!({ "text": "hi" }).to_string())
        .await;
    h.panel().await;

    h.decide(chat, "c1", "deny").await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Denied by the user");
    h.assert_panel_silent();
}

#[tokio::test]
async fn a_chat_grant_covers_one_tool_in_one_chat() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let granted = h
        .approve_and_run(
            chat,
            "c1",
            ECHO,
            &json!({ "text": "a" }).to_string(),
            "allow_for_chat",
        )
        .await;
    assert_eq!(granted["status"], "ok");

    let mut again = h
        .execute(chat, "c2", ECHO, &json!({ "text": "b" }).to_string())
        .await;
    h.drain_tool_events().await;
    assert_eq!(tool_result(&mut again).await["output"], "echo: b");

    let _other_tool = h.execute(chat, "c3", "mcp:fake:fail", "{}").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    let _other_chat = h
        .execute(
            Uuid::new_v4(),
            "c4",
            ECHO,
            &json!({ "text": "c" }).to_string(),
        )
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn arguments_that_break_the_server_schema_are_refused_before_any_prompt() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();

    for arguments in [
        "{}",
        r#"{"text":5}"#,
        r#"{"text":"a","extra":1}"#,
        "[]",
        "not json",
    ] {
        assert_eq!(
            h.expect_denied(chat, ECHO, arguments).await,
            "Invalid arguments",
            "{arguments}"
        );
    }
}

#[tokio::test]
async fn an_error_from_the_server_comes_back_as_an_error_result() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:fail", json!({})).await;

    assert_eq!(result["status"], "error");
    assert_eq!(result["output"], "Error: something broke");
}

#[tokio::test]
async fn content_and_structured_answers_become_text() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let mixed = run(&mut h, chat, "c1", "mcp:fake:mixed", json!({})).await;
    assert_eq!(
        mixed["output"],
        "first\n[image omitted]\nresource text\n[resource omitted]\n[resource link: file:///c]\nlast"
    );

    let structured = run(&mut h, chat, "c2", "mcp:fake:structured", json!({})).await;
    assert_eq!(structured["output"], "{\"answer\":42}");
}

#[tokio::test]
async fn a_huge_answer_is_cut() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:big", json!({})).await;

    let output = result["output"].as_str().unwrap();
    assert!(output.len() < 70_000);
    assert!(output.ends_with("[output truncated]"));
}

#[tokio::test]
async fn a_slow_call_times_out_with_a_clear_error() {
    let mut config = server("fake", "normal");
    config.timeout_seconds = 1;
    let Some(mut h) = harness(&[config]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let start = Instant::now();

    let slow = run(&mut h, chat, "c1", "mcp:fake:slow", json!({})).await;

    assert_eq!(slow["status"], "error");
    assert_eq!(
        slow["output"],
        "Error: the MCP server did not answer in time"
    );
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
async fn the_server_cannot_read_a_file_outside_its_read_paths() {
    let secret_dir = tempfile::tempdir().unwrap();
    let secret = secret_dir.path().canonicalize().unwrap().join("secret.txt");
    std::fs::write(&secret, "top secret").unwrap();
    let readable_dir = tempfile::tempdir().unwrap();
    let readable = readable_dir
        .path()
        .canonicalize()
        .unwrap()
        .join("shared.txt");
    std::fs::write(&readable, "shared text").unwrap();
    let mut config = server("fake", "normal");
    config
        .read_paths
        .push(readable_dir.path().display().to_string());
    let Some(mut h) = harness(&[config]).await else {
        return;
    };
    let chat = Uuid::new_v4();

    let hidden = run(
        &mut h,
        chat,
        "c1",
        "mcp:fake:read_file",
        json!({ "path": secret.display().to_string() }),
    )
    .await;
    assert_eq!(hidden["status"], "error");
    assert!(!hidden["output"].as_str().unwrap().contains("top secret"));

    let shared = run(
        &mut h,
        chat,
        "c2",
        "mcp:fake:read_file",
        json!({ "path": readable.display().to_string() }),
    )
    .await;
    assert_eq!(shared["output"], "shared text");
}

#[tokio::test]
async fn the_server_environment_holds_only_fixed_and_configured_variables() {
    std::env::set_var("NOOK_TEST_LEAK", "leaked");
    let mut config = server("fake", "normal");
    config
        .env
        .insert("FAKE_TOKEN".to_string(), "token".to_string());
    let Some(mut h) = harness(&[config]).await else {
        return;
    };

    let result = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:env", json!({})).await;

    let text = result["output"].as_str().unwrap();
    let names: Vec<&str> = text.split(',').collect();
    for expected in ["FAKE_TOKEN", "HOME", "LANG", "PATH", "TMPDIR"] {
        assert!(names.contains(&expected), "{text}");
    }
    assert!(!text.contains("NOOK_TEST_LEAK"), "{text}");
}

#[tokio::test]
async fn a_server_that_stops_makes_later_calls_fail_cleanly() {
    let Some(mut h) = harness(&[server("fake", "crash")]).await else {
        return;
    };
    tokio::time::sleep(Duration::from_millis(300)).await;

    let result = run(&mut h, Uuid::new_v4(), "c1", ECHO, json!({ "text": "hi" })).await;

    assert_eq!(result["status"], "error");
    assert_eq!(result["output"], "Error: the MCP server stopped");
}

#[tokio::test]
async fn a_server_that_died_is_started_again_by_the_next_call() {
    let limits = StartupLimits {
        restart_interval: Duration::ZERO,
        ..quick()
    };
    let Some(mut h) = harness_with(&[server("fake", "normal")], limits).await else {
        return;
    };

    let died = run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    assert_eq!(died["status"], "error");
    assert_eq!(died["output"], "Error: the MCP server stopped");

    let chat = Uuid::new_v4();
    let again = run(&mut h, chat, "c2", ECHO, json!({ "text": "hi" })).await;
    assert_eq!(again["status"], "ok");
    assert_eq!(again["output"], "echo: hi");
}

#[tokio::test]
async fn a_server_that_died_soon_after_a_start_stays_stopped() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };

    run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    let again = run(&mut h, Uuid::new_v4(), "c2", ECHO, json!({ "text": "hi" })).await;

    assert_eq!(again["status"], "error");
    assert_eq!(again["output"], "Error: the MCP server stopped");
}

#[tokio::test]
async fn a_server_that_never_answers_initialize_is_skipped_without_blocking_the_others() {
    let configs = [server("stuck", "hang"), server("fake", "normal")];
    let start = Instant::now();

    let Some(h) = harness(&configs).await else {
        return;
    };

    assert!(start.elapsed() < Duration::from_secs(8));
    assert!(h.state.providers.contains(ECHO));
    assert!(!h.state.providers.contains("mcp:stuck:echo"));
}

#[tokio::test]
async fn a_server_with_an_unknown_protocol_version_is_skipped() {
    assert!(provider(&[server("old", "old")], quick()).await.is_none());
}

#[tokio::test]
async fn servers_that_cannot_start_or_have_bad_settings_are_skipped() {
    let mut missing = server("missing", "normal");
    missing.command = "/definitely/not/here".to_string();
    let mut empty = server("empty", "normal");
    empty.command = "  ".to_string();
    let bad_name = server("has space", "normal");
    let good = server("good", "normal");
    let duplicate = server("good", "normal");

    let Some(h) = harness(&[missing, empty, bad_name, good, duplicate]).await else {
        return;
    };

    assert!(h.state.providers.contains("mcp:good:echo"));
    assert!(names(&h).iter().all(|name| name.starts_with("mcp__good__")));
}

#[tokio::test]
async fn a_server_that_lists_one_name_in_env_and_env_secrets_is_skipped_with_a_report() {
    let mut bad = server("bad", "normal");
    bad.env
        .insert("SHARED_NAME".to_string(), "env_val".to_string());
    bad.env_secrets
        .insert("SHARED_NAME".to_string(), "secret_name".to_string());
    let good = server("good", "normal");
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) = McpProvider::start_reporting(
        &[bad, good],
        &sandbox,
        quick(),
        nook_core::secrets::SecretContext::default(),
    )
    .await;
    let p = provider.expect("good server starts");
    assert!(p
        .descriptors()
        .iter()
        .all(|d| d.name.starts_with("mcp__good__")));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].source_id, "mcp:bad");
    let reason = failed[0].reason.as_ref().unwrap();
    assert!(reason.contains("SHARED_NAME"));
    assert!(!reason.contains("env_val"));
    assert!(!reason.contains("secret_name"));
}

#[tokio::test]
async fn an_env_secrets_entry_with_an_empty_name_is_skipped_with_a_report() {
    let mut bad = server("bad", "normal");
    bad.env_secrets
        .insert("EMPTY_SECRET".to_string(), "".to_string());
    let good = server("good", "normal");
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) = McpProvider::start_reporting(
        &[bad, good],
        &sandbox,
        quick(),
        nook_core::secrets::SecretContext::default(),
    )
    .await;
    let p = provider.expect("good server starts");
    assert!(p
        .descriptors()
        .iter()
        .all(|d| d.name.starts_with("mcp__good__")));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].source_id, "mcp:bad");
    let reason = failed[0].reason.as_ref().unwrap();
    assert!(reason.contains("EMPTY_SECRET"));
}

#[tokio::test]
async fn an_env_secrets_entry_for_the_model_key_is_skipped_with_a_report() {
    let mut bad = server("bad", "normal");
    bad.env_secrets
        .insert("SECRET_VAR".to_string(), "model-api-key".to_string());
    let good = server("good", "normal");
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) = McpProvider::start_reporting(
        &[bad, good],
        &sandbox,
        quick(),
        nook_core::secrets::SecretContext::default(),
    )
    .await;
    let p = provider.expect("good server starts");
    assert!(p
        .descriptors()
        .iter()
        .all(|d| d.name.starts_with("mcp__good__")));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].source_id, "mcp:bad");
    let reason = failed[0].reason.as_ref().unwrap();
    assert!(reason.contains("SECRET_VAR"));
    assert!(!reason.contains("model-api-key-val"));
}

#[tokio::test]
async fn a_library_variable_name_in_env_is_skipped_with_a_report() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    for var in ["LD_PRELOAD", "DYLD_INSERT_LIBRARIES"] {
        let mut bad = server("bad", "normal");
        bad.env
            .insert(var.to_string(), "/lib/exploit.so".to_string());
        let good = server("good", "normal");
        let (provider, failed) = McpProvider::start_reporting(
            &[bad, good],
            &sandbox,
            quick(),
            nook_core::secrets::SecretContext::default(),
        )
        .await;
        let p = provider.expect("good server starts");
        assert!(p
            .descriptors()
            .iter()
            .all(|d| d.name.starts_with("mcp__good__")));
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].source_id, "mcp:bad");
        let reason = failed[0].reason.as_ref().unwrap();
        assert!(reason.contains(var));
        assert!(!reason.contains("/lib/exploit.so"));
    }
}

#[tokio::test]
async fn a_library_variable_name_in_env_secrets_is_skipped_with_a_report() {
    let mut bad = server("bad", "normal");
    bad.env_secrets
        .insert("LD_LIBRARY_PATH".to_string(), "my-secret".to_string());
    let good = server("good", "normal");
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) = McpProvider::start_reporting(
        &[bad, good],
        &sandbox,
        quick(),
        nook_core::secrets::SecretContext::default(),
    )
    .await;
    let p = provider.expect("good server starts");
    assert!(p
        .descriptors()
        .iter()
        .all(|d| d.name.starts_with("mcp__good__")));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].source_id, "mcp:bad");
    let reason = failed[0].reason.as_ref().unwrap();
    assert!(reason.contains("LD_LIBRARY_PATH"));
    assert!(!reason.contains("my-secret"));
}

#[tokio::test]
async fn a_name_that_only_looks_like_a_library_variable_is_accepted() {
    let mut server_ldap = server("ldap", "normal");
    server_ldap
        .env
        .insert("LDAP_URL".to_string(), "ldap://localhost".to_string());
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let (provider, failed) = McpProvider::start_reporting(
        &[server_ldap],
        &sandbox,
        quick(),
        nook_core::secrets::SecretContext::default(),
    )
    .await;
    let p = provider.expect("ldap server starts");
    assert!(p
        .descriptors()
        .iter()
        .any(|d| d.name.starts_with("mcp__ldap__")));
    assert!(failed.is_empty());
}

#[tokio::test]
async fn two_servers_keep_their_tools_apart() {
    let Some(h) = harness(&[server("one", "normal"), server("two", "normal")]).await else {
        return;
    };

    assert!(h.state.providers.contains("mcp:one:echo"));
    assert!(h.state.providers.contains("mcp:two:echo"));
    let names = names(&h);
    assert!(names.contains(&"mcp__one__echo".to_string()));
    assert!(names.contains(&"mcp__two__echo".to_string()));
}

#[tokio::test]
async fn a_chatty_server_and_a_paged_list_work() {
    let Some(h) = harness(&[server("chatty", "chatty")]).await else {
        return;
    };
    assert!(h.state.providers.contains("mcp:chatty:echo"));

    let Some(paged) = harness(&[server("paged", "paged")]).await else {
        return;
    };
    assert_eq!(names(&paged), ["mcp__paged__echo", "mcp__paged__fail"]);
}

#[tokio::test]
async fn cancelling_aborts_a_running_call() {
    let Some(mut h) = harness(&[server("fake", "normal")]).await else {
        return;
    };
    let chat = Uuid::new_v4();
    let mut call = h.execute(chat, "c1", "mcp:fake:slow", "{}").await;
    h.panel().await;
    h.decide(chat, "c1", "allow_once").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallStarted { .. }
    ));

    nook_core::tool_approval::abort_running_tools(&h.state, call.client_id).await;

    common::assert_worker_silent(&mut call).await;
    assert!(h.state.running_tools.lock().await.is_empty());
}

#[tokio::test]
async fn the_policy_resolves_every_mcp_tool_to_the_shared_entry() {
    let policy = common::repo_policy();

    let spec = policy.catalog.resolve("mcp:anything:at_all").unwrap();

    assert_eq!(spec.display_name, "MCP tool");
    assert_eq!(spec.approval.tier, "ask");
    assert!(policy.catalog.resolve("nook:nothing").is_none());
}

#[tokio::test]
async fn sources_list_each_server_as_ready_or_off_with_a_reason() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let mut broken = server("broken", "normal");
    broken.command = "/nonexistent/server".to_string();
    let mut empty_command = server("blank", "normal");
    empty_command.command = " ".to_string();
    let configs = [
        server("fake", "normal"),
        broken,
        empty_command,
        server("fake", "normal"),
        server("bad name", "normal"),
    ];

    let (provider, failed) = McpProvider::start_reporting(
        &configs,
        &sandbox,
        quick(),
        nook_core::secrets::SecretContext::default(),
    )
    .await;

    let ready = provider.expect("one server runs").sources();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].source_id, "mcp:fake");
    assert_eq!(ready[0].state, ToolSourceState::Ready);
    assert!(ready[0].tools >= 2);
    let mut reasons: Vec<(String, String)> = failed
        .into_iter()
        .map(|s| {
            assert_eq!(s.state, ToolSourceState::Off);
            (s.source_id, s.reason.unwrap())
        })
        .collect();
    reasons.sort();
    assert_eq!(reasons.len(), 4);
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:blank" && r == "The command is empty"));
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:broken" && !r.is_empty()));
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:fake" && r.contains("name")));
    assert!(reasons
        .iter()
        .any(|(id, r)| id == "mcp:bad name" && r.contains("not valid")));
}

#[tokio::test]
async fn a_stopped_server_shows_as_stopped_until_a_call_starts_it() {
    let limits = StartupLimits {
        restart_interval: Duration::ZERO,
        ..quick()
    };
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let provider = McpProvider::start(
        &[server("fake", "normal")],
        &sandbox,
        limits,
        nook_core::secrets::SecretContext::default(),
    )
    .await
    .unwrap();
    let mut h = Harness::new(vec![Arc::new(provider)]).await;

    run(&mut h, Uuid::new_v4(), "c1", "mcp:fake:die", json!({})).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let stopped = h.state.providers.sources();
    assert_eq!(stopped[0].state, ToolSourceState::Stopped);
    assert!(stopped[0].reason.is_some());

    run(&mut h, Uuid::new_v4(), "c2", ECHO, json!({ "text": "hi" })).await;
    assert_eq!(h.state.providers.sources()[0].state, ToolSourceState::Ready);
}

#[tokio::test]
async fn a_store_error_that_holds_the_secret_bytes_never_reaches_a_report_or_the_log() {
    use keyring_core::api::CredentialStoreApi;
    use nook_core::secrets::{OsSecretStore, SecretContext};

    let record_dir = tempfile::tempdir().unwrap();
    let record_path = record_dir.path().canonicalize().unwrap();
    let Some(sandbox) = sandbox().await else {
        return;
    };

    let secret_bytes = b"super-confidential-credential-bytes";
    let mock = keyring_core::mock::Store::new().unwrap();
    let entry = mock
        .build("Nook MCP", "google-docs-client-secret", None)
        .unwrap();
    let cred: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    cred.set_error(keyring_core::Error::BadEncoding(secret_bytes.to_vec()));

    let store = Arc::new(OsSecretStore::with_store(mock));
    let secrets = SecretContext {
        store,
        mask: Arc::default(),
    };

    let mut config = docs_server(&record_path);
    config.env_secrets.insert(
        "GOOGLE_DRIVE_MCP_CLIENT_SECRET".to_string(),
        "google-docs-client-secret".to_string(),
    );

    let (log_buffer, _guard) = capture_log();

    let (provider, failed) =
        McpProvider::start_reporting(&[config], &sandbox, quick(), secrets).await;

    assert!(provider.is_none());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].state, ToolSourceState::Off);
    let reason = failed[0].reason.as_ref().expect("failure reason");
    let secret_text = "super-confidential-credential-bytes";
    assert!(
        !reason.contains(secret_text),
        "failure report must not contain secret bytes: {reason}"
    );

    let logged = log_text(&log_buffer);
    assert!(
        !logged.contains(secret_text),
        "captured log must not contain secret bytes: {logged}"
    );
}
