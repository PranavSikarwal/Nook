#![cfg(unix)]

use nook_core::config::Config;
use nook_core::policy_assets::PolicyAssets;
use nook_core::protocol::{DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest};
use nook_core::server::AppState;
use nook_core::supervisor::WorkerSupervisor;
use nook_core::tool_approval::{
    expire_pending_approval, handle_approval_decision, handle_authorize_tool, AuthorizeContext,
    AuthorizeToolCall, APPROVAL_TIMEOUT,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

const FETCH: &str = "nook:web_fetch";
const SEARCH: &str = "nook:web_search";

fn policy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../policy")
}

fn modified_policy(edit: impl FnOnce(String) -> String) -> Arc<PolicyAssets> {
    let read = |relative: &str| std::fs::read_to_string(policy_dir().join(relative)).unwrap();
    let tools = edit(read("tools.yaml"));
    Arc::new(
        PolicyAssets::from_sources(
            &tools,
            &read("cedar/schema.cedarschema"),
            &read("cedar/policies.cedar"),
        )
        .unwrap(),
    )
}

fn fake_worker_command() -> Vec<String> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_worker.py");
    vec!["python3".into(), script.to_string_lossy().into_owned()]
}

struct Harness {
    state: Arc<AppState>,
    panel_tx: mpsc::Sender<DaemonMessage>,
    panel_rx: mpsc::Receiver<DaemonMessage>,
}

struct Call {
    client_id: Uuid,
    worker_id: Uuid,
    call_id: String,
    chat_id: Uuid,
    worker_rx: mpsc::Receiver<DaemonWorkerEvent>,
}

impl Harness {
    async fn new() -> Self {
        let policy = Arc::new(PolicyAssets::load(&policy_dir()).unwrap());
        Self::with_policy(policy).await
    }

    async fn with_edited_catalog(edit: impl FnOnce(String) -> String) -> Self {
        Self::with_policy(modified_policy(edit)).await
    }

    async fn with_timeout(approval_timeout: Duration) -> Self {
        let policy = Arc::new(PolicyAssets::load(&policy_dir()).unwrap());
        Self::build(policy, approval_timeout).await
    }

    async fn with_policy(policy: Arc<PolicyAssets>) -> Self {
        Self::build(policy, APPROVAL_TIMEOUT).await
    }

    async fn build(policy: Arc<PolicyAssets>, approval_timeout: Duration) -> Self {
        let supervisor =
            WorkerSupervisor::new(&Config::default(), None, Some(fake_worker_command()))
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
            policy,
        });
        let (panel_tx, panel_rx) = mpsc::channel(32);
        Self {
            state,
            panel_tx,
            panel_rx,
        }
    }

    async fn supervisor(&self) -> WorkerSupervisor {
        self.state.supervisor.lock().await.clone().unwrap()
    }

    async fn start_call(&self, chat_id: Uuid, call_id: &str, tool: &str, args: &str) -> Call {
        let call = self.run_call(chat_id, call_id, tool, args).await;
        self.deliver_authorize_event(call).await
    }

    async fn run_call(&self, chat_id: Uuid, call_id: &str, tool: &str, args: &str) -> Call {
        let worker_id = Uuid::new_v4();
        let mut worker_rx = self
            .supervisor()
            .await
            .send_request(DaemonWorkerRequest::Run {
                request_id: worker_id,
                chat_id,
                text: format!("authorize:{call_id}|{tool}|{args}"),
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
            call_id: call_id.to_string(),
            chat_id,
            worker_rx,
        }
    }

    async fn deliver_authorize_event(&self, mut call: Call) -> Call {
        let event = recv(&mut call.worker_rx).await;
        let DaemonWorkerEvent::AuthorizeTool {
            message_id,
            call_id,
            chat_id,
            tool_name,
            arguments,
            ..
        } = event
        else {
            panic!("Expected AuthorizeTool, got {event:?}");
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
            },
            &self.panel_tx,
        )
        .await;
        call
    }

    async fn decide_in_chat(&self, chat_id: Uuid, call_id: &str, action: &str) {
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

    async fn expect_prompt(&mut self) -> DaemonMessage {
        tokio::time::timeout(Duration::from_secs(5), self.panel_rx.recv())
            .await
            .expect("Timeout waiting for the Panel prompt")
            .expect("Panel channel closed")
    }

    fn close_panel(&mut self) {
        let (tx, _rx) = mpsc::channel(32);
        self.panel_tx = tx;
    }

    async fn pending_count(&self) -> usize {
        self.state.pending_approvals.lock().await.len()
    }

    fn assert_no_panel_message(&mut self) {
        assert!(self.panel_rx.try_recv().is_err(), "Panel got a message");
    }

    async fn expect_error(&mut self) -> String {
        match self.expect_prompt().await {
            DaemonMessage::Error { error, .. } => error.message,
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    async fn grant_count_for(&self, chat_id: Uuid, tool: &str, host: Option<&str>) -> usize {
        self.state
            .policy
            .authorizer
            .grant_store()
            .lock()
            .await
            .matching_scopes(chat_id, tool, host, None)
            .len()
    }
}

async fn recv(rx: &mut mpsc::Receiver<DaemonWorkerEvent>) -> DaemonWorkerEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("Timeout waiting for a Worker event")
        .expect("Worker channel closed")
}

async fn worker_result(call: &mut Call) -> (String, Option<String>) {
    let (call_id, decision, reason) = worker_result_with_id(call).await;
    assert_eq!(call_id, call.call_id);
    (decision, reason)
}

async fn worker_result_with_id(call: &mut Call) -> (String, String, Option<String>) {
    match recv(&mut call.worker_rx).await {
        DaemonWorkerEvent::TextDelta {
            request_id, text, ..
        } => {
            assert_eq!(request_id, call.worker_id);
            let value: Value =
                serde_json::from_str(text.strip_prefix("Result: ").unwrap()).unwrap();
            (
                value["call_id"].as_str().unwrap().to_string(),
                value["decision"].as_str().unwrap().to_string(),
                value["reason"].as_str().map(str::to_string),
            )
        }
        other => panic!("Expected an authorization result, got {other:?}"),
    }
}

async fn assert_worker_silent(call: &mut Call) {
    assert!(
        tokio::time::timeout(Duration::from_millis(300), call.worker_rx.recv())
            .await
            .is_err(),
        "Worker received a result"
    );
}

fn fetch_args(url: &str) -> String {
    serde_json::json!({ "url": url }).to_string()
}

#[tokio::test]
async fn web_search_under_allow_tier_is_allowed_without_a_prompt() {
    let mut h = Harness::new().await;
    let mut call = h
        .start_call(Uuid::new_v4(), "c1", SEARCH, r#"{"query":"rust"}"#)
        .await;
    let (decision, _) = worker_result(&mut call).await;
    assert_eq!(decision, "allow");
    h.assert_no_panel_message();
}

#[tokio::test]
async fn invalid_arguments_are_denied_without_a_prompt() {
    let mut h = Harness::new().await;
    for (tool, args) in [
        (FETCH, "{"),
        (FETCH, "{}"),
        (FETCH, r#"{"url":"file:///etc/passwd"}"#),
    ] {
        let mut call = h.start_call(Uuid::new_v4(), "c1", tool, args).await;
        let (decision, reason) = worker_result(&mut call).await;
        assert_eq!(decision, "deny");
        assert_eq!(reason.as_deref(), Some("Invalid arguments"));
    }
    h.assert_no_panel_message();
}

#[tokio::test]
async fn unknown_tool_is_denied_without_a_prompt() {
    let mut h = Harness::new().await;
    let mut call = h.start_call(Uuid::new_v4(), "c1", "nook:nope", "{}").await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("Unknown tool"));
    h.assert_no_panel_message();
}

#[tokio::test]
async fn deny_tier_is_denied_even_with_a_persistent_grant() {
    let mut h =
        Harness::with_edited_catalog(|yaml| yaml.replacen("tier: allow", "tier: deny", 1)).await;
    let chat_id = Uuid::new_v4();
    let search = nook_core::authorization::AuthorizationRequest::normalize(
        &h.state.policy.catalog,
        chat_id,
        "c0",
        SEARCH,
        r#"{"query":"rust"}"#,
    )
    .unwrap();
    h.state
        .policy
        .authorizer
        .grant_store()
        .lock()
        .await
        .add_grant(nook_core::policy::ScopedGrant::for_request(&search, "persistent").unwrap());
    let mut call = h
        .start_call(chat_id, "c1", SEARCH, r#"{"query":"rust"}"#)
        .await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert!(reason.is_some());
    h.assert_no_panel_message();
}

#[tokio::test]
async fn authorization_for_another_chat_is_denied() {
    let mut h = Harness::new().await;
    let mut call = h
        .run_call(Uuid::new_v4(), "c1", SEARCH, r#"{"query":"rust"}"#)
        .await;
    call.chat_id = Uuid::new_v4();
    let mut call = h.deliver_authorize_event(call).await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(
        reason.as_deref(),
        Some("Request does not match the running chat")
    );
    h.assert_no_panel_message();
}

#[tokio::test]
async fn fetch_asks_the_panel_with_daemon_built_fields() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let url = "https://Docs.Example.com/a";
    let mut call = h.start_call(chat_id, "c1", FETCH, &fetch_args(url)).await;
    match h.expect_prompt().await {
        DaemonMessage::ApprovalRequested {
            id,
            chat_id: prompt_chat,
            call_id,
            tool_name,
            arguments,
            explanation,
            resource_summary,
            actions,
            ..
        } => {
            assert_eq!(id, call.client_id);
            assert_eq!(prompt_chat, chat_id);
            assert_eq!(call_id, "c1");
            assert_eq!(tool_name, FETCH);
            assert_eq!(arguments, fetch_args(url));
            assert_eq!(
                explanation,
                h.state.policy.catalog.tool(FETCH).unwrap().description
            );
            assert_eq!(
                resource_summary,
                "https://Docs.Example.com/a (docs.example.com)"
            );
            let catalog_actions = &h.state.policy.catalog.tool(FETCH).unwrap().approval.actions;
            assert_eq!(&actions, catalog_actions);
            let ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();
            assert_eq!(ids, ["allow_once", "allow_for_chat_host", "deny"]);
        }
        other => panic!("Expected ApprovalRequested, got {other:?}"),
    }
    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn deny_action_denies_the_call() {
    let mut h = Harness::new().await;
    let mut call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/"),
        )
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(call.chat_id, "c1", "deny").await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert!(reason.is_some());
}

#[tokio::test]
async fn allow_once_allows_the_call_and_echoes_request_and_call_ids() {
    let mut h = Harness::new().await;
    let mut call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(call.chat_id, "c1", "allow_once").await;
    let (call_id, decision, _) = worker_result_with_id(&mut call).await;
    assert_eq!(call_id, "c1");
    assert_eq!(decision, "allow");
}

#[tokio::test]
async fn once_grant_is_consumed_and_does_not_cover_a_second_identical_call() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let url = fetch_args("https://a.example/x");
    let mut first = h.start_call(chat_id, "c1", FETCH, &url).await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");
    assert_eq!(
        h.grant_count_for(chat_id, "web_fetch", Some("a.example"))
            .await,
        0
    );

    let mut second = h.start_call(chat_id, "c2", FETCH, &url).await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut second).await;
}

#[tokio::test]
async fn once_grant_does_not_authorize_another_digest() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut first = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");

    let mut other = h
        .start_call(chat_id, "c2", FETCH, &fetch_args("https://a.example/y"))
        .await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut other).await;
}

#[tokio::test]
async fn host_grant_covers_the_host_in_that_chat_only() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut first = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_for_chat_host").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");

    let mut same_host = h
        .start_call(
            chat_id,
            "c2",
            FETCH,
            &fetch_args("https://A.example/other?q=1"),
        )
        .await;
    assert_eq!(worker_result(&mut same_host).await.0, "allow");
    h.assert_no_panel_message();

    let mut other_host = h
        .start_call(chat_id, "c3", FETCH, &fetch_args("https://b.example/x"))
        .await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut other_host).await;

    let mut other_chat = h
        .start_call(
            Uuid::new_v4(),
            "c4",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut other_chat).await;
}

fn ask_for_search(yaml: String) -> String {
    let actions = concat!(
        "tier: ask_once_per_host\n",
        "      actions:\n",
        "        - id: allow_once\n",
        "          label: \"Allow once\"\n",
        "          description: \"Allow this exact request one time.\"\n",
        "        - id: allow_for_chat\n",
        "          label: \"Allow for this chat\"\n",
        "          description: \"Allow this tool for the rest of this chat.\"\n",
        "        - id: always_allow\n",
        "          label: \"Always allow\"\n",
        "          description: \"Allow this tool in every chat until you remove the grant.\"\n",
    );
    yaml.replacen("tier: allow\n", actions, 1)
}

#[tokio::test]
async fn chat_grant_covers_only_its_chat() {
    let mut h = Harness::with_edited_catalog(ask_for_search).await;
    let chat_id = Uuid::new_v4();
    let args = r#"{"query":"rust"}"#;
    let mut first = h.start_call(chat_id, "c1", SEARCH, args).await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_for_chat").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");

    let mut same_chat = h
        .start_call(chat_id, "c2", SEARCH, r#"{"query":"other"}"#)
        .await;
    assert_eq!(worker_result(&mut same_chat).await.0, "allow");
    h.assert_no_panel_message();

    let mut other_chat = h.start_call(Uuid::new_v4(), "c3", SEARCH, args).await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut other_chat).await;
}

#[tokio::test]
async fn always_allow_covers_every_chat() {
    let mut h = Harness::with_edited_catalog(ask_for_search).await;
    let chat_id = Uuid::new_v4();
    let args = r#"{"query":"rust"}"#;
    let mut first = h.start_call(chat_id, "c1", SEARCH, args).await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "always_allow").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");

    let mut other_chat = h.start_call(Uuid::new_v4(), "c2", SEARCH, args).await;
    assert_eq!(worker_result(&mut other_chat).await.0, "allow");
    h.assert_no_panel_message();
}

#[tokio::test]
async fn cedar_still_denies_after_an_approval_it_does_not_accept() {
    let mut h = Harness::with_edited_catalog(|yaml| {
        yaml.replacen("id: allow_for_chat_host", "id: allow_for_chat", 1)
    })
    .await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_for_chat").await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert!(reason.is_some());
    assert_eq!(
        h.grant_count_for(chat_id, "web_fetch", Some("a.example"))
            .await,
        0
    );
}

#[tokio::test]
async fn yaml_cannot_permit_a_persistent_grant_that_cedar_refuses_for_fetch() {
    let mut h = Harness::with_edited_catalog(|yaml| {
        yaml.replacen("id: allow_for_chat_host", "id: always_allow", 1)
    })
    .await;
    let chat_id = Uuid::new_v4();
    let url = "https://a.example/x";
    let mut call = h.start_call(chat_id, "c1", FETCH, &fetch_args(url)).await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "always_allow").await;
    assert_eq!(worker_result(&mut call).await.0, "deny");

    let mut again = h.start_call(chat_id, "c2", FETCH, &fetch_args(url)).await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut again).await;
}

#[tokio::test]
async fn changed_label_in_tools_yaml_reaches_the_approval_message() {
    let mut h = Harness::with_edited_catalog(|yaml| {
        yaml.replace("label: \"Allow once\"", "label: \"Approve a single time\"")
    })
    .await;
    let chat_id = Uuid::new_v4();
    let _call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    match h.expect_prompt().await {
        DaemonMessage::ApprovalRequested { actions, .. } => {
            assert_eq!(actions[0].id, "allow_once");
            assert_eq!(actions[0].label, "Approve a single time");
            assert_eq!(actions[0].description, "Allow this exact request one time.");
        }
        other => panic!("Expected ApprovalRequested, got {other:?}"),
    }
}

#[tokio::test]
async fn unoffered_action_is_rejected_and_the_call_stays_pending() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "always_allow").await;
    assert!(h.expect_error().await.contains("not offered"));
    assert_worker_silent(&mut call).await;
    assert!(h.state.pending_approvals.lock().await.contains_key("c1"));

    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert_eq!(worker_result(&mut call).await.0, "allow");
}

#[tokio::test]
async fn unknown_call_id_is_rejected() {
    let mut h = Harness::new().await;
    h.decide_in_chat(Uuid::new_v4(), "nope", "allow_once").await;
    assert!(h.expect_error().await.contains("Unknown or invalid"));
}

#[tokio::test]
async fn decision_from_another_chat_is_rejected_and_keeps_the_call_pending() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(Uuid::new_v4(), "c1", "allow_once").await;
    assert!(h.expect_error().await.contains("does not belong"));
    assert_worker_silent(&mut call).await;
    assert!(h.state.pending_approvals.lock().await.contains_key("c1"));
}

#[tokio::test]
async fn a_decision_cannot_be_applied_twice() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert_eq!(worker_result(&mut call).await.0, "allow");
    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert!(h.expect_error().await.contains("Unknown or invalid"));
    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn duplicate_pending_call_id_is_denied() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut first = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    let mut second = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://b.example/x"))
        .await;
    let (decision, reason) = worker_result(&mut second).await;
    assert_eq!(decision, "deny");
    assert!(reason.unwrap().contains("already pending"));
    assert_worker_silent(&mut first).await;
}

#[tokio::test]
async fn grants_survive_a_worker_restart() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let url = fetch_args("https://a.example/x");
    let mut first = h.start_call(chat_id, "c1", FETCH, &url).await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_for_chat_host").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");

    let supervisor = h.supervisor().await;
    let crash_id = Uuid::new_v4();
    let mut crash_rx = supervisor
        .send_request(DaemonWorkerRequest::Run {
            request_id: crash_id,
            chat_id,
            text: "crash".to_string(),
            attachments: vec![],
        })
        .await
        .unwrap();
    assert!(matches!(
        recv(&mut crash_rx).await,
        DaemonWorkerEvent::Error { .. }
    ));

    let worker_id = Uuid::new_v4();
    let mut worker_rx = loop {
        match supervisor
            .send_request(DaemonWorkerRequest::Run {
                request_id: worker_id,
                chat_id,
                text: format!("authorize:c2|{FETCH}|{}", fetch_args("https://a.example/y")),
                attachments: vec![],
            })
            .await
        {
            Ok(rx) => break rx,
            Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    };
    assert!(matches!(
        recv(&mut worker_rx).await,
        DaemonWorkerEvent::MessageStarted { .. }
    ));
    let call = Call {
        client_id: Uuid::new_v4(),
        worker_id,
        call_id: "c2".to_string(),
        chat_id,
        worker_rx,
    };
    let mut call = h.deliver_authorize_event(call).await;
    assert_eq!(worker_result(&mut call).await.0, "allow");
    h.assert_no_panel_message();
}

#[tokio::test]
async fn cancelling_a_request_drops_its_pending_calls_and_denies_them() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;

    nook_core::tool_approval::deny_pending_for_request(&h.state, call.client_id, "Cancelled").await;

    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("Cancelled"));
    assert!(h.state.pending_approvals.lock().await.is_empty());

    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert!(h.expect_error().await.contains("Unknown or invalid"));
    assert_worker_silent(&mut call).await;
    assert_eq!(
        h.grant_count_for(chat_id, "web_fetch", Some("a.example"))
            .await,
        0
    );
}

#[tokio::test]
async fn cancelling_one_request_leaves_other_pending_calls_alone() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut first = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    let mut second = h
        .start_call(chat_id, "c2", FETCH, &fetch_args("https://b.example/x"))
        .await;
    h.expect_prompt().await;

    nook_core::tool_approval::deny_pending_for_request(&h.state, first.client_id, "Cancelled")
        .await;
    assert_eq!(worker_result(&mut first).await.0, "deny");
    assert_worker_silent(&mut second).await;
    assert!(h.state.pending_approvals.lock().await.contains_key("c2"));
}

async fn connect(
    state: &Arc<AppState>,
) -> (
    tokio::net::unix::OwnedWriteHalf,
    tokio::io::Lines<tokio::io::BufReader<tokio::net::unix::OwnedReadHalf>>,
    tokio::task::JoinHandle<()>,
    tokio::sync::broadcast::Sender<()>,
) {
    use tokio::io::AsyncBufReadExt;
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_t_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server = nook_core::server::Server::bind(&socket_path, shutdown_rx, state.clone())
        .await
        .unwrap();
    let task = tokio::spawn(async move {
        let _ = server.run().await;
    });
    let stream = tokio::net::UnixStream::connect(&socket_path).await.unwrap();
    let (reader, writer) = stream.into_split();
    (
        writer,
        tokio::io::BufReader::new(reader).lines(),
        task,
        shutdown_tx,
    )
}

async fn send_client(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    message: &nook_core::protocol::ClientMessage,
) {
    use tokio::io::AsyncWriteExt;
    let mut payload = serde_json::to_string(message).unwrap();
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.unwrap();
}

async fn next_message(
    lines: &mut tokio::io::Lines<tokio::io::BufReader<tokio::net::unix::OwnedReadHalf>>,
) -> DaemonMessage {
    let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
        .await
        .expect("Timeout waiting for a Daemon reply")
        .unwrap()
        .expect("Expected a reply line");
    serde_json::from_str(&line).unwrap()
}

#[tokio::test]
async fn client_cancel_denies_the_waiting_call_and_a_late_decision_fails() {
    use nook_core::protocol::ClientMessage;
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;

    let (mut writer, mut lines, task, shutdown_tx) = connect(&h.state).await;
    let cancel_id = Uuid::new_v4();
    send_client(
        &mut writer,
        &ClientMessage::Cancel {
            id: cancel_id,
            target_id: call.client_id,
        },
    )
    .await;
    assert_eq!(
        next_message(&mut lines).await,
        DaemonMessage::Cancelled {
            id: cancel_id,
            target_id: call.client_id
        }
    );
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("Cancelled"));

    let late_id = Uuid::new_v4();
    send_client(
        &mut writer,
        &ClientMessage::ApprovalDecision {
            id: late_id,
            chat_id,
            call_id: "c1".to_string(),
            action: "allow_once".to_string(),
        },
    )
    .await;
    match next_message(&mut lines).await {
        DaemonMessage::Error { id, error } => {
            assert_eq!(id, late_id);
            assert!(error.message.contains("Unknown or invalid"));
        }
        other => panic!("Expected Error, got {other:?}"),
    }
    assert_worker_silent(&mut call).await;
    shutdown_tx.send(()).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn deleting_a_chat_removes_its_grants() {
    use nook_core::protocol::ClientMessage;
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let url = fetch_args("https://a.example/x");
    let mut first = h.start_call(chat_id, "c1", FETCH, &url).await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_for_chat_host").await;
    assert_eq!(worker_result(&mut first).await.0, "allow");
    let mut covered = h.start_call(chat_id, "c2", FETCH, &url).await;
    assert_eq!(worker_result(&mut covered).await.0, "allow");

    let (mut writer, mut lines, task, shutdown_tx) = connect(&h.state).await;
    let delete_id = Uuid::new_v4();
    send_client(
        &mut writer,
        &ClientMessage::DeleteChat {
            id: delete_id,
            chat_id,
        },
    )
    .await;
    assert_eq!(
        next_message(&mut lines).await,
        DaemonMessage::Deleted {
            id: delete_id,
            chat_id
        }
    );

    let mut after = h.start_call(chat_id, "c3", FETCH, &url).await;
    assert!(matches!(
        h.expect_prompt().await,
        DaemonMessage::ApprovalRequested { .. }
    ));
    assert_worker_silent(&mut after).await;
    shutdown_tx.send(()).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn closed_panel_denies_the_call_and_drops_the_pending_entry() {
    let mut h = Harness::new().await;
    h.close_panel();
    let mut call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("Approval could not be shown"));
    assert_eq!(h.pending_count().await, 0);
}

#[test]
fn approval_timeout_is_five_minutes() {
    assert_eq!(APPROVAL_TIMEOUT, Duration::from_secs(300));
}

#[tokio::test]
async fn an_expired_approval_denies_the_call_and_a_late_decision_records_no_grant() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    expire_pending_approval(&h.state, "c1", call.worker_id, Duration::from_millis(20)).await;
    let (decision, reason) = worker_result(&mut call).await;
    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("Approval timed out"));
    assert_eq!(h.pending_count().await, 0);

    h.decide_in_chat(chat_id, "c1", "allow_for_chat_host").await;
    assert!(h.expect_error().await.contains("Unknown or invalid"));
    assert_eq!(
        h.grant_count_for(chat_id, "web_fetch", Some("a.example"))
            .await,
        0
    );
    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn expiry_leaves_a_decision_that_is_being_forwarded() {
    let mut h = Harness::new().await;
    let mut call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    h.expect_prompt().await;
    h.state
        .pending_approvals
        .lock()
        .await
        .get_mut("c1")
        .unwrap()
        .decision_in_flight = true;
    expire_pending_approval(&h.state, "c1", call.worker_id, Duration::from_millis(20)).await;
    assert_eq!(h.pending_count().await, 1);
    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn expiry_does_nothing_after_the_user_decided() {
    let mut h = Harness::new().await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert_eq!(worker_result(&mut call).await.0, "allow");
    expire_pending_approval(&h.state, "c1", call.worker_id, Duration::from_millis(20)).await;
    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn expiry_does_not_touch_a_newer_call_that_reuses_the_call_id() {
    let mut h = Harness::new().await;
    let mut call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    h.expect_prompt().await;
    expire_pending_approval(&h.state, "c1", Uuid::new_v4(), Duration::from_millis(20)).await;
    assert_eq!(h.pending_count().await, 1);
    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn an_unanswered_approval_expires_by_itself() {
    let mut h = Harness::with_timeout(Duration::from_millis(100)).await;
    let mut call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    h.expect_prompt().await;

    let (decision, reason) = worker_result(&mut call).await;

    assert_eq!(decision, "deny");
    assert_eq!(reason.as_deref(), Some("Approval timed out"));
    assert_eq!(h.pending_count().await, 0);
}

#[tokio::test]
async fn a_decision_before_the_timeout_is_not_followed_by_a_second_result() {
    let mut h = Harness::with_timeout(Duration::from_millis(150)).await;
    let chat_id = Uuid::new_v4();
    let mut call = h
        .start_call(chat_id, "c1", FETCH, &fetch_args("https://a.example/x"))
        .await;
    h.expect_prompt().await;
    h.decide_in_chat(chat_id, "c1", "allow_once").await;
    assert_eq!(worker_result(&mut call).await.0, "allow");

    tokio::time::sleep(Duration::from_millis(400)).await;

    assert_worker_silent(&mut call).await;
}

#[tokio::test]
async fn the_expiry_timer_does_not_keep_the_daemon_state_alive() {
    let mut h = Harness::new().await;
    let _call = h
        .start_call(
            Uuid::new_v4(),
            "c1",
            FETCH,
            &fetch_args("https://a.example/x"),
        )
        .await;
    h.expect_prompt().await;

    assert_eq!(Arc::strong_count(&h.state), 1);
}
