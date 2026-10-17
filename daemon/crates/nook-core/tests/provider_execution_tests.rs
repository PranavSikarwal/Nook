#![cfg(unix)]

use async_trait::async_trait;
use nook_core::config::Config;
use nook_core::policy_assets::PolicyAssets;
use nook_core::protocol::{DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest, ToolDescriptor};
use nook_core::providers::{
    CallContext, Provider, ProviderError, ProviderRegistry, ToolFacts, ToolOutput,
};
use nook_core::server::AppState;
use nook_core::supervisor::WorkerSupervisor;
use nook_core::tool_approval::{
    abort_running_tools, handle_approval_decision, handle_authorize_tool, AuthorizeContext,
    AuthorizeToolCall, APPROVAL_TIMEOUT,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

const ECHO: &str = "nook:echo_test";
const FETCH: &str = "nook:web_fetch";

const ECHO_YAML: &str = r#"
  nook:echo_test:
    display_name: "Echo"
    description: "Echo a message for tests."
    approval:
      tier: ask
      actions:
        - id: allow_once
          label: "Allow once"
          description: "Allow this call once."
        - id: allow_for_chat
          label: "Allow for this chat"
          description: "Allow this tool for the rest of this chat."
        - id: deny
          label: "Deny"
          description: "Reject this call."
"#;

const ECHO_SCHEMA: &str = r#"
  action "echo_test" appliesTo {
    principal: [User],
    resource: [Tool],
    context: {
      chat_id: String,
      approval_tier: String,
      grant_scope: String,
      argument_digest: String,
    }
  };
"#;

const ECHO_POLICY: &str = r#"
permit (principal, action == Nook::Action::"echo_test", resource)
when { context.grant_scope == "once" || context.grant_scope == "chat" };
"#;

struct EchoProvider {
    started: AtomicUsize,
    dropped: Arc<AtomicBool>,
}

struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl Provider for EchoProvider {
    fn descriptors(&self) -> Vec<ToolDescriptor> {
        vec![ToolDescriptor {
            id: ECHO.to_string(),
            name: "echo_test".to_string(),
            description: "Echo a message.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "message": { "type": "string" } },
                "required": ["message"],
                "additionalProperties": false
            }),
        }]
    }

    fn facts(&self, _tool_id: &str, arguments: &Value) -> Result<ToolFacts, ProviderError> {
        Ok(ToolFacts {
            summary: format!("Echo {}", arguments["message"].as_str().unwrap_or_default()),
            ..ToolFacts::default()
        })
    }

    async fn call(&self, _tool_id: &str, arguments: Value, _context: CallContext) -> ToolOutput {
        self.started.fetch_add(1, Ordering::SeqCst);
        let message = arguments["message"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        match message.as_str() {
            "slow" => {
                let _flag = DropFlag(self.dropped.clone());
                tokio::time::sleep(Duration::from_secs(60)).await;
                ToolOutput::ok("late")
            }
            "fail" => ToolOutput::error("Error: the echo failed"),
            "big" => ToolOutput::ok("x".repeat(200_000)),
            other => ToolOutput::ok(format!("echo: {other}")),
        }
    }
}

fn policy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../policy")
}

fn policy_with_echo() -> Arc<PolicyAssets> {
    let read = |relative: &str| std::fs::read_to_string(policy_dir().join(relative)).unwrap();
    let tools = format!("{}{}", read("tools.yaml"), ECHO_YAML);
    let schema = read("cedar/schema.cedarschema");
    let schema = schema.trim_end().trim_end_matches('}').to_string() + ECHO_SCHEMA + "}\n";
    let policies = format!("{}{}", read("cedar/policies.cedar"), ECHO_POLICY);
    Arc::new(PolicyAssets::from_sources(&tools, &schema, &policies).unwrap())
}

fn fake_worker_command() -> Vec<String> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_worker.py");
    vec!["python3".into(), script.to_string_lossy().into_owned()]
}

struct Harness {
    state: Arc<AppState>,
    provider: Arc<EchoProvider>,
    panel_tx: mpsc::Sender<DaemonMessage>,
    panel_rx: mpsc::Receiver<DaemonMessage>,
}

struct Call {
    client_id: Uuid,
    worker_id: Uuid,
    chat_id: Uuid,
    worker_rx: mpsc::Receiver<DaemonWorkerEvent>,
}

impl Harness {
    async fn new() -> Self {
        Self::with_timeout(APPROVAL_TIMEOUT).await
    }

    async fn with_timeout(approval_timeout: Duration) -> Self {
        let provider = Arc::new(EchoProvider {
            started: AtomicUsize::new(0),
            dropped: Arc::new(AtomicBool::new(false)),
        });
        let registry = ProviderRegistry::new(vec![provider.clone()]).unwrap();
        let supervisor = WorkerSupervisor::with_tools(
            &Config::default(),
            None,
            Some(fake_worker_command()),
            registry.descriptors().to_vec(),
        )
        .await
        .unwrap();
        let state = Arc::new(AppState {
            config: RwLock::new(Config::default()),
            config_path: PathBuf::from("/tmp/nook-unused-config.toml"),
            pool: None,
            supervisor: Mutex::new(Some(supervisor)),
            preserve_worker_on_settings_update: false,
            open_requests: Mutex::new(HashMap::new()),
            pending_approvals: Mutex::new(HashMap::new()),
            cancelled_requests: Mutex::new(HashSet::new()),
            approval_timeout,
            providers: Arc::new(registry),
            running_tools: Mutex::new(HashMap::new()),
            policy: policy_with_echo(),
        });
        let (panel_tx, panel_rx) = mpsc::channel(32);
        Self {
            state,
            provider,
            panel_tx,
            panel_rx,
        }
    }

    async fn supervisor(&self) -> WorkerSupervisor {
        self.state.supervisor.lock().await.clone().unwrap()
    }

    async fn run_text(&self, chat_id: Uuid, text: &str) -> Call {
        let worker_id = Uuid::new_v4();
        let mut worker_rx = self
            .supervisor()
            .await
            .send_request(DaemonWorkerRequest::Run {
                request_id: worker_id,
                chat_id,
                text: text.to_string(),
                attachments: vec![],
            })
            .await
            .unwrap();
        assert!(matches!(
            recv(&mut worker_rx).await,
            DaemonWorkerEvent::MessageStarted { .. }
        ));
        Call {
            client_id: Uuid::new_v4(),
            worker_id,
            chat_id,
            worker_rx,
        }
    }

    async fn execute(&self, chat_id: Uuid, call_id: &str, tool: &str, args: &str) -> Call {
        let call = self
            .run_text(chat_id, &format!("execute:{call_id}|{tool}|{args}"))
            .await;
        self.deliver(call, true).await
    }

    async fn authorize(&self, chat_id: Uuid, call_id: &str, tool: &str, args: &str) -> Call {
        let call = self
            .run_text(chat_id, &format!("authorize:{call_id}|{tool}|{args}"))
            .await;
        self.deliver(call, false).await
    }

    async fn deliver(&self, mut call: Call, execute: bool) -> Call {
        let event = recv(&mut call.worker_rx).await;
        let (message_id, call_id, chat_id, tool_name, arguments) = match event {
            DaemonWorkerEvent::ExecuteTool {
                message_id,
                call_id,
                chat_id,
                tool_name,
                arguments,
                ..
            }
            | DaemonWorkerEvent::AuthorizeTool {
                message_id,
                call_id,
                chat_id,
                tool_name,
                arguments,
                ..
            } => (message_id, call_id, chat_id, tool_name, arguments),
            other => panic!("Expected a tool event, got {other:?}"),
        };
        handle_authorize_tool(
            &self.state,
            &AuthorizeContext {
                client_request_id: call.client_id,
                worker_request_id: call.worker_id,
                chat_id: call.chat_id,
            },
            AuthorizeToolCall {
                message_id,
                call_id,
                chat_id,
                tool_name,
                arguments,
                execute,
            },
            &self.panel_tx,
        )
        .await;
        call
    }

    async fn decide(&self, chat_id: Uuid, call_id: &str, action: &str) {
        handle_approval_decision(
            &self.state,
            Uuid::new_v4(),
            chat_id,
            call_id.to_string(),
            action.to_string(),
            &self.panel_tx,
        )
        .await;
    }

    async fn panel(&mut self) -> DaemonMessage {
        tokio::time::timeout(Duration::from_secs(5), self.panel_rx.recv())
            .await
            .expect("Timeout waiting for the Panel")
            .expect("Panel channel closed")
    }

    fn assert_panel_silent(&mut self) {
        assert!(self.panel_rx.try_recv().is_err(), "Panel got a message");
    }
}

async fn recv(rx: &mut mpsc::Receiver<DaemonWorkerEvent>) -> DaemonWorkerEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("Timeout waiting for a Worker event")
        .expect("Worker channel closed")
}

async fn text_from_worker(call: &mut Call, prefix: &str) -> Value {
    match recv(&mut call.worker_rx).await {
        DaemonWorkerEvent::TextDelta { text, .. } => {
            serde_json::from_str(text.strip_prefix(prefix).unwrap()).unwrap()
        }
        other => panic!("Expected text, got {other:?}"),
    }
}

async fn tool_result(call: &mut Call) -> Value {
    text_from_worker(call, "ToolResult: ").await
}

async fn assert_worker_silent(call: &mut Call) {
    assert!(
        tokio::time::timeout(Duration::from_millis(300), call.worker_rx.recv())
            .await
            .is_err(),
        "Worker received a message"
    );
}

#[tokio::test]
async fn the_worker_receives_the_tool_catalog_after_ready() {
    let h = Harness::new().await;
    let mut call = h.run_text(Uuid::new_v4(), "catalog?").await;

    let catalog = text_from_worker(&mut call, "Catalog: ").await;

    assert_eq!(catalog[0]["id"], ECHO);
    assert_eq!(catalog[0]["name"], "echo_test");
    assert_eq!(catalog[0]["input_schema"]["type"], "object");
}

#[tokio::test]
async fn an_approved_call_runs_in_the_daemon_and_returns_its_output() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h.execute(chat_id, "c1", ECHO, r#"{"message":"hi"}"#).await;

    let DaemonMessage::ApprovalRequested {
        resource_summary, ..
    } = h.panel().await
    else {
        panic!("Expected an approval request");
    };
    assert_eq!(resource_summary, "Echo hi");
    h.decide(chat_id, "c1", "allow_once").await;

    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallStarted { ref call_id, ref name, .. } if call_id == "c1" && name == ECHO
    ));
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallFinished { ref call_id, ref result, .. } if call_id == "c1" && result == "echo: hi"
    ));
    let result = tool_result(&mut call).await;
    assert_eq!(result["call_id"], "c1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["output"], "echo: hi");
}

#[tokio::test]
async fn a_denied_call_never_reaches_the_provider() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h.execute(chat_id, "c1", ECHO, r#"{"message":"hi"}"#).await;
    h.panel().await;

    h.decide(chat_id, "c1", "deny").await;

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Denied by the user");
    assert_eq!(h.provider.started.load(Ordering::SeqCst), 0);
    h.assert_panel_silent();
}

#[tokio::test]
async fn a_chat_grant_lets_the_next_call_run_without_a_prompt() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut first = h.execute(chat_id, "c1", ECHO, r#"{"message":"a"}"#).await;
    h.panel().await;
    h.decide(chat_id, "c1", "allow_for_chat").await;
    assert_eq!(tool_result(&mut first).await["status"], "ok");
    h.panel().await;
    h.panel().await;

    let mut second = h.execute(chat_id, "c2", ECHO, r#"{"message":"b"}"#).await;

    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallStarted { .. }
    ));
    assert_eq!(tool_result(&mut second).await["output"], "echo: b");

    let other_chat = Uuid::new_v4();
    let _third = h
        .execute(other_chat, "c3", ECHO, r#"{"message":"c"}"#)
        .await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallFinished { .. }
    ));
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
}

#[tokio::test]
async fn a_provider_error_comes_back_as_an_error_result() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .execute(chat_id, "c1", ECHO, r#"{"message":"fail"}"#)
        .await;
    h.panel().await;
    h.decide(chat_id, "c1", "allow_once").await;

    let result = tool_result(&mut call).await;

    assert_eq!(result["status"], "error");
    assert_eq!(result["output"], "Error: the echo failed");
}

#[tokio::test]
async fn output_is_cut_at_the_size_limit() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h.execute(chat_id, "c1", ECHO, r#"{"message":"big"}"#).await;
    h.panel().await;
    h.decide(chat_id, "c1", "allow_once").await;
    h.panel().await;
    let DaemonMessage::ToolCallFinished { result, .. } = h.panel().await else {
        panic!("Expected a finished tool call");
    };
    assert_eq!(result.chars().count(), 2000);

    let output = tool_result(&mut call).await["output"]
        .as_str()
        .unwrap()
        .to_string();

    assert!(output.len() < 70_000);
    assert!(output.ends_with("[output truncated]"));
}

#[tokio::test]
async fn invalid_arguments_are_denied_before_any_prompt() {
    let mut h = Harness::new().await;
    for args in [
        "{}",
        r#"{"message":5}"#,
        r#"{"message":"a","extra":1}"#,
        "[",
    ] {
        let mut call = h.execute(Uuid::new_v4(), "c1", ECHO, args).await;
        let result = tool_result(&mut call).await;
        assert_eq!(result["status"], "denied", "{args}");
        assert_eq!(result["output"], "Invalid arguments", "{args}");
    }
    h.assert_panel_silent();
    assert_eq!(h.provider.started.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_worker_cannot_authorize_a_daemon_tool_to_run_it_itself() {
    let mut h = Harness::new().await;
    let mut call = h
        .authorize(Uuid::new_v4(), "c1", ECHO, r#"{"message":"hi"}"#)
        .await;

    let result = text_from_worker(&mut call, "Result: ").await;

    assert_eq!(result["decision"], "deny");
    assert_eq!(result["reason"], "Tool runs in the Daemon");
    h.assert_panel_silent();
}

#[tokio::test]
async fn the_worker_cannot_ask_the_daemon_to_run_a_worker_tool() {
    let mut h = Harness::new().await;
    let mut call = h
        .execute(
            Uuid::new_v4(),
            "c1",
            FETCH,
            r#"{"url":"https://example.com/"}"#,
        )
        .await;

    let result = tool_result(&mut call).await;

    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Tool runs in the Worker");
    h.assert_panel_silent();
}

#[tokio::test]
async fn an_unknown_tool_is_denied() {
    let mut h = Harness::new().await;
    let mut call = h.execute(Uuid::new_v4(), "c1", "nook:nope", "{}").await;

    let result = tool_result(&mut call).await;

    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Unknown tool");
    h.assert_panel_silent();
}

#[tokio::test]
async fn an_unanswered_approval_expires_as_a_denied_result() {
    let mut h = Harness::with_timeout(Duration::from_millis(100)).await;
    let mut call = h
        .execute(Uuid::new_v4(), "c1", ECHO, r#"{"message":"hi"}"#)
        .await;
    h.panel().await;

    let result = tool_result(&mut call).await;

    assert_eq!(result["status"], "denied");
    assert_eq!(result["output"], "Approval timed out");
    assert_eq!(h.provider.started.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancelling_aborts_a_running_call_and_sends_no_result() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .execute(chat_id, "c1", ECHO, r#"{"message":"slow"}"#)
        .await;
    h.panel().await;
    h.decide(chat_id, "c1", "allow_once").await;
    assert!(matches!(
        h.panel().await,
        DaemonMessage::ToolCallStarted { .. }
    ));
    for _ in 0..50 {
        if h.provider.started.load(Ordering::SeqCst) == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    abort_running_tools(&h.state, call.client_id).await;

    for _ in 0..50 {
        if h.provider.dropped.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(h.provider.dropped.load(Ordering::SeqCst));
    assert_worker_silent(&mut call).await;
    assert!(h.state.running_tools.lock().await.is_empty());
}

#[tokio::test]
async fn a_call_approved_after_a_cancel_does_not_run() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h.execute(chat_id, "c1", ECHO, r#"{"message":"hi"}"#).await;
    h.panel().await;
    h.state
        .cancelled_requests
        .lock()
        .await
        .insert(call.client_id);

    h.decide(chat_id, "c1", "allow_once").await;

    assert_worker_silent(&mut call).await;
    assert_eq!(h.provider.started.load(Ordering::SeqCst), 0);
}
