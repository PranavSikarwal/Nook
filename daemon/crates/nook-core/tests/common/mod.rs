#![allow(dead_code)]

use nook_core::config::Config;
use nook_core::policy_assets::PolicyAssets;
use nook_core::protocol::{DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest};
use nook_core::providers::{Provider, ProviderRegistry};
use nook_core::server::AppState;
use nook_core::supervisor::WorkerSupervisor;
use nook_core::tool_approval::{
    handle_approval_decision, handle_authorize_tool, AuthorizeContext, AuthorizeToolCall,
    APPROVAL_TIMEOUT,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

pub fn policy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../policy")
}

pub fn repo_policy() -> Arc<PolicyAssets> {
    Arc::new(PolicyAssets::load(&policy_dir()).unwrap())
}

fn fake_worker_command() -> Vec<String> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_worker.py");
    vec!["python3".into(), script.to_string_lossy().into_owned()]
}

pub struct Harness {
    pub state: Arc<AppState>,
    pub panel_tx: mpsc::Sender<DaemonMessage>,
    pub panel_rx: mpsc::Receiver<DaemonMessage>,
}

pub struct Call {
    pub client_id: Uuid,
    pub worker_id: Uuid,
    pub chat_id: Uuid,
    pub worker_rx: mpsc::Receiver<DaemonWorkerEvent>,
}

impl Harness {
    pub async fn new(providers: Vec<Arc<dyn Provider>>) -> Self {
        Self::build(repo_policy(), providers, APPROVAL_TIMEOUT).await
    }

    pub async fn build(
        policy: Arc<PolicyAssets>,
        providers: Vec<Arc<dyn Provider>>,
        approval_timeout: Duration,
    ) -> Self {
        Self::build_with_pool(policy, providers, approval_timeout, None).await
    }

    pub async fn build_with_pool(
        policy: Arc<PolicyAssets>,
        providers: Vec<Arc<dyn Provider>>,
        approval_timeout: Duration,
        pool: Option<sqlx::PgPool>,
    ) -> Self {
        let registry = ProviderRegistry::new(providers).unwrap();
        Self::assemble(
            policy,
            registry,
            approval_timeout,
            pool,
            Config::default(),
            PathBuf::from("/tmp/nook-unused-config.toml"),
        )
        .await
    }

    /// Starts from a config and builds the tools from it, as the Daemon does.
    pub async fn with_config(config: Config, config_path: PathBuf) -> Self {
        Self::with_config_and_notes(config, config_path, None).await
    }

    pub async fn with_config_and_notes(
        config: Config,
        config_path: PathBuf,
        notes_program: Option<PathBuf>,
    ) -> Self {
        let registry = nook_core::providers::build_registry(&config.tools, notes_program).await;
        Self::assemble(
            repo_policy(),
            registry,
            APPROVAL_TIMEOUT,
            None,
            config,
            config_path,
        )
        .await
    }

    async fn assemble(
        policy: Arc<PolicyAssets>,
        registry: ProviderRegistry,
        approval_timeout: Duration,
        pool: Option<sqlx::PgPool>,
        config: Config,
        config_path: PathBuf,
    ) -> Self {
        let supervisor = WorkerSupervisor::with_tools(
            &Config::default(),
            None,
            Some(fake_worker_command()),
            registry.descriptors().to_vec(),
        )
        .await
        .unwrap();
        let state = Arc::new(AppState {
            config: RwLock::new(config),
            config_path,
            pool: nook_core::server::DbPool::new(pool),
            supervisor: Mutex::new(Some(supervisor)),
            preserve_worker_on_settings_update: false,
            open_requests: Mutex::new(HashMap::new()),
            pending_approvals: Mutex::new(HashMap::new()),
            cancelled_requests: Mutex::new(HashSet::new()),
            approval_timeout,
            providers: nook_core::providers::ProviderHandle::new(registry),
            running_tools: Mutex::new(HashMap::new()),
            policy,
        });
        let (panel_tx, panel_rx) = mpsc::channel(64);
        Self {
            state,
            panel_tx,
            panel_rx,
        }
    }

    async fn supervisor(&self) -> WorkerSupervisor {
        self.state.supervisor.lock().await.clone().unwrap()
    }

    pub async fn run_text(&self, chat_id: Uuid, text: &str) -> Call {
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

    pub async fn execute(&self, chat_id: Uuid, call_id: &str, tool: &str, args: &str) -> Call {
        let call = self
            .run_text(chat_id, &format!("execute:{call_id}|{tool}|{args}"))
            .await;
        self.deliver(call).await
    }

    /// Starts a Worker-side tool call, as the Worker does for web tools.
    pub async fn authorize(&self, chat_id: Uuid, call_id: &str, tool: &str, args: &str) -> Call {
        let call = self
            .run_text(chat_id, &format!("authorize:{call_id}|{tool}|{args}"))
            .await;
        self.deliver_with(call, false).await
    }

    async fn deliver(&self, call: Call) -> Call {
        self.deliver_with(call, true).await
    }

    async fn deliver_with(&self, mut call: Call, execute: bool) -> Call {
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

    pub async fn decide(&self, chat_id: Uuid, call_id: &str, action: &str) {
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

    pub async fn panel(&mut self) -> DaemonMessage {
        tokio::time::timeout(Duration::from_secs(10), self.panel_rx.recv())
            .await
            .expect("Timeout waiting for the Panel")
            .expect("Panel channel closed")
    }

    pub fn assert_panel_silent(&mut self) {
        assert!(self.panel_rx.try_recv().is_err(), "Panel got a message");
    }

    /// Runs a call to its end and returns the Worker's result.
    pub async fn approve_and_run(
        &mut self,
        chat_id: Uuid,
        call_id: &str,
        tool: &str,
        args: &str,
        action: &str,
    ) -> Value {
        let mut call = self.execute(chat_id, call_id, tool, args).await;
        assert!(matches!(
            self.panel().await,
            DaemonMessage::ApprovalRequested { .. }
        ));
        self.decide(chat_id, call_id, action).await;
        self.drain_tool_events().await;
        tool_result(&mut call).await
    }

    /// Reads the tool-started and tool-finished events a run sends to the Panel.
    pub async fn drain_tool_events(&mut self) {
        assert!(matches!(
            self.panel().await,
            DaemonMessage::ToolCallStarted { .. }
        ));
        assert!(matches!(
            self.panel().await,
            DaemonMessage::ToolCallFinished { .. }
        ));
    }

    /// Runs a call and returns the denial result without any prompt.
    pub async fn expect_denied(&mut self, chat_id: Uuid, tool: &str, args: &str) -> String {
        let mut call = self.execute(chat_id, "denied", tool, args).await;
        let result = tool_result(&mut call).await;
        assert_eq!(result["status"], "denied", "{result}");
        self.assert_panel_silent();
        result["output"].as_str().unwrap().to_string()
    }

    /// Cancels a waiting or running request.
    pub async fn cancel(&self, client_id: Uuid) {
        self.state.cancelled_requests.lock().await.insert(client_id);
        nook_core::tool_approval::deny_pending_for_request(&self.state, client_id, "Cancelled")
            .await;
        nook_core::tool_approval::abort_running_tools(&self.state, client_id).await;
    }
}

pub async fn recv(rx: &mut mpsc::Receiver<DaemonWorkerEvent>) -> DaemonWorkerEvent {
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("Timeout waiting for a Worker event")
        .expect("Worker channel closed")
}

pub async fn text_from_worker(call: &mut Call, prefix: &str) -> Value {
    match recv(&mut call.worker_rx).await {
        DaemonWorkerEvent::TextDelta { text, .. } => {
            serde_json::from_str(text.strip_prefix(prefix).unwrap()).unwrap()
        }
        other => panic!("Expected text, got {other:?}"),
    }
}

pub async fn tool_result(call: &mut Call) -> Value {
    text_from_worker(call, "ToolResult: ").await
}

pub async fn assert_worker_silent(call: &mut Call) {
    assert!(
        tokio::time::timeout(Duration::from_millis(300), call.worker_rx.recv())
            .await
            .is_err(),
        "Worker received a message"
    );
}

/// The decision the Worker gets for a call it asked to authorize.
pub async fn authorization_result(call: &mut Call) -> (String, Option<String>) {
    let value = text_from_worker(call, "Result: ").await;
    (
        value["decision"].as_str().unwrap().to_string(),
        value["reason"].as_str().map(str::to_string),
    )
}

pub const CREATE_NOTE: &str = "nook:notes_create_note";

#[cfg(unix)]
pub struct NotesFixture {
    _dir: tempfile::TempDir,
    pub program: PathBuf,
    pub log_file: PathBuf,
}

#[cfg(unix)]
impl NotesFixture {
    pub fn new(response_stdout: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let log_file = dir.path().join("fake_osascript.log");
        let program = dir.path().join("fake_osascript.sh");
        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\0' \"$arg\" >> \"{}\"; done\nprintf '%s' \"{}\"\n",
            log_file.display(),
            response_stdout
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            _dir: dir,
            program,
            log_file,
        }
    }

    pub async fn harness(&self) -> Harness {
        Harness::with_config_and_notes(
            Config::default(),
            PathBuf::from("/tmp/nook-unused-config.toml"),
            Some(self.program.clone()),
        )
        .await
    }

    pub fn logged_args(&self) -> Vec<String> {
        if !self.log_file.exists() {
            return Vec::new();
        }
        let bytes = std::fs::read(&self.log_file).unwrap();
        bytes
            .split(|&b| b == 0)
            .filter(|slice| !slice.is_empty())
            .map(|slice| String::from_utf8_lossy(slice).into_owned())
            .collect()
    }
}

#[cfg(all(test, unix))]
mod atomic_notes_folder_tests {
    use super::*;

    #[tokio::test]
    async fn a_folder_change_while_a_decision_waits_denies_the_call_and_records_no_grant() {
        let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
        let fixture = NotesFixture::new(&format!("{note_id}\n"));
        let mut h = fixture.harness().await;
        let chat = Uuid::new_v4();
        let args = serde_json::json!({
            "title": "Meeting Notes",
            "body": "Discuss project plan"
        })
        .to_string();

        let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;
        let panel_msg = h.panel().await;
        assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

        // Hold state.config.write()
        let mut cfg_guard = h.state.config.write().await;

        let state_clone = h.state.clone();
        let panel_tx = h.panel_tx.clone();
        let decision_task = tokio::spawn(async move {
            nook_core::tool_approval::handle_approval_decision(
                &state_clone,
                Uuid::new_v4(),
                chat,
                "c1".to_string(),
                "allow_for_chat".to_string(),
                &panel_tx,
            )
            .await;
        });

        // Wait until the decision has claimed the pending approval (decision_in_flight is true)
        let mut attempts = 0;
        loop {
            attempts += 1;
            let in_flight = {
                let approvals = h.state.pending_approvals.lock().await;
                approvals
                    .get("c1")
                    .map(|e| e.decision_in_flight)
                    .unwrap_or(false)
            };
            if in_flight || attempts > 100 {
                break;
            }
            tokio::task::yield_now().await;
        }

        // Change the folder in memory under the write lock
        cfg_guard.tools.notes.folder = "Work".to_string();
        let tools = cfg_guard.tools.clone();
        let rebuilt = h.state.providers.current().rebuilt(&tools);
        h.state.providers.replace(rebuilt);
        nook_core::tool_approval::deny_pending_notes_approvals(&h.state).await;
        h.state
            .policy
            .authorizer
            .grant_store()
            .lock()
            .await
            .remove_tools(&[CREATE_NOTE]);

        // Release the write lock
        drop(cfg_guard);

        decision_task.await.unwrap();

        // The call must be denied with "The Notes folder changed"
        let result = tool_result(&mut call).await;
        assert_eq!(result["status"], "denied");
        assert_eq!(result["output"], "The Notes folder changed");

        // No grant was recorded
        let store_arc = h.state.policy.authorizer.grant_store();
        let grant_store = store_arc.lock().await;
        assert!(
            grant_store
                .matching_scopes(chat, CREATE_NOTE, None, None, None)
                .is_empty(),
            "grant must not be recorded on folder mismatch"
        );

        // Fake program never ran
        assert!(fixture.logged_args().is_empty());
    }

    #[tokio::test]
    async fn a_decision_before_a_folder_change_runs_in_the_old_folder() {
        let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
        let fixture = NotesFixture::new(&format!("{note_id}\n"));
        let mut h = fixture.harness().await;
        let chat = Uuid::new_v4();
        let args = serde_json::json!({
            "title": "Old Note",
            "body": "Content in old folder"
        })
        .to_string();

        let mut call = h.execute(chat, "c1", CREATE_NOTE, &args).await;
        let panel_msg = h.panel().await;
        assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

        // Decision finishes first
        h.decide(chat, "c1", "allow_once").await;
        h.drain_tool_events().await;

        // Then folder change runs
        {
            let mut cfg_guard = h.state.config.write().await;
            cfg_guard.tools.notes.folder = "Work".to_string();
            let tools = cfg_guard.tools.clone();
            let rebuilt = h.state.providers.current().rebuilt(&tools);
            h.state.providers.replace(rebuilt);
            nook_core::tool_approval::deny_pending_notes_approvals(&h.state).await;
            h.state
                .policy
                .authorizer
                .grant_store()
                .lock()
                .await
                .remove_tools(&[CREATE_NOTE]);
        }

        let result = tool_result(&mut call).await;
        assert_eq!(result["status"], "ok");
        assert!(
            result["output"]
                .as_str()
                .unwrap()
                .contains("in the Notes folder \"Nook\""),
            "tool result must name the old folder Nook: {}",
            result["output"]
        );

        let logged = fixture.logged_args();
        assert_eq!(logged[2], ":Nook", "fake program must receive old folder");
    }

    #[tokio::test]
    async fn a_folder_change_prevents_a_chat_grant_from_running_in_the_new_folder() {
        let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
        let fixture = NotesFixture::new(&format!("{note_id}\n"));
        let mut h = fixture.harness().await;
        let chat = Uuid::new_v4();
        let args1 = serde_json::json!({
            "title": "First Note",
            "body": "First content"
        })
        .to_string();

        // Run call 1 with allow_for_chat to establish chat grant
        let res1 = h
            .approve_and_run(chat, "c1", CREATE_NOTE, &args1, "allow_for_chat")
            .await;
        assert_eq!(res1["status"], "ok");

        let store_arc = h.state.policy.authorizer.grant_store();
        assert!(
            !store_arc
                .lock()
                .await
                .matching_scopes(chat, "notes_create_note", None, None, None)
                .is_empty(),
            "chat grant should exist"
        );

        // Now test the race: write lock held while call 2 is started
        let mut cfg_guard = h.state.config.write().await;

        let args2 = serde_json::json!({
            "title": "Second Note",
            "body": "Second content"
        })
        .to_string();

        // Spawn call 2 - handle_authorize_tool will take state.config.read() and wait
        let state_clone = h.state.clone();
        let panel_tx = h.panel_tx.clone();
        let client_id = Uuid::new_v4();
        let worker_id = Uuid::new_v4();
        let call_task = tokio::spawn(async move {
            nook_core::tool_approval::handle_authorize_tool(
                &state_clone,
                &nook_core::tool_approval::AuthorizeContext {
                    client_request_id: client_id,
                    worker_request_id: worker_id,
                    chat_id: chat,
                },
                nook_core::tool_approval::AuthorizeToolCall {
                    message_id: Uuid::new_v4(),
                    call_id: "c2".to_string(),
                    chat_id: chat,
                    tool_name: CREATE_NOTE.to_string(),
                    arguments: args2,
                    execute: true,
                },
                &panel_tx,
            )
            .await;
        });

        // Folder change happens under the write lock
        cfg_guard.tools.notes.folder = "Work".to_string();
        let tools = cfg_guard.tools.clone();
        let rebuilt = h.state.providers.current().rebuilt(&tools);
        h.state.providers.replace(rebuilt);
        nook_core::tool_approval::deny_pending_notes_approvals(&h.state).await;
        h.state
            .policy
            .authorizer
            .grant_store()
            .lock()
            .await
            .remove_tools(&["notes_create_note"]);

        // Release write lock
        drop(cfg_guard);

        call_task.await.unwrap();

        // Because chat grant was dropped and folder changed, call 2 MUST NOT run automatically.
        // It must show an approval request on the panel for the new folder.
        let panel_msg = h.panel().await;
        match panel_msg {
            DaemonMessage::ApprovalRequested {
                call_id, tool_name, ..
            } => {
                assert_eq!(call_id, "c2");
                assert_eq!(tool_name, "nook:notes_create_note");
            }
            other => panic!("expected ApprovalRequested for new folder, got {other:?}"),
        }

        // Deny call 2
        h.decide(chat, "c2", "deny").await;
        h.assert_panel_silent();
    }

    #[tokio::test]
    async fn a_folder_change_leaves_an_approval_and_a_grant_of_another_tool_intact() {
        let note_id = "x-coredata://12345678-1234-1234-1234-1234567890ab/ICNote/p1";
        let fixture = NotesFixture::new(&format!("{note_id}\n"));
        let mut h = fixture.harness().await;
        let chat = Uuid::new_v4();

        // Start another tool's call: nook:web_fetch
        let mut other_call = h
            .authorize(
                chat,
                "fetch1",
                "nook:web_fetch",
                r#"{"url":"https://example.com"}"#,
            )
            .await;

        let panel_msg = h.panel().await;
        assert!(matches!(panel_msg, DaemonMessage::ApprovalRequested { .. }));

        // Add a grant for another tool in grant_store
        let other_grant = nook_core::policy::ScopedGrant {
            chat_id: chat,
            tool_name: "web_fetch".to_string(),
            scope: "chat".to_string(),
            host: Some("example.org".to_string()),
            path: None,
            argument_digest: None,
        };
        h.state
            .policy
            .authorizer
            .grant_store()
            .lock()
            .await
            .add_grant(other_grant);

        // Perform the Notes folder change
        {
            let mut cfg_guard = h.state.config.write().await;
            cfg_guard.tools.notes.folder = "Work".to_string();
            let tools = cfg_guard.tools.clone();
            let rebuilt = h.state.providers.current().rebuilt(&tools);
            h.state.providers.replace(rebuilt);
            nook_core::tool_approval::deny_pending_notes_approvals(&h.state).await;
            h.state
                .policy
                .authorizer
                .grant_store()
                .lock()
                .await
                .remove_tools(&["notes_create_note"]);
        }

        // The pending approval for fetch1 must STILL be pending
        {
            let approvals = h.state.pending_approvals.lock().await;
            assert!(
                approvals.contains_key("fetch1"),
                "pending approval for another tool must stay"
            );
        }

        // The grant for web_fetch must STILL be present
        {
            let store_arc = h.state.policy.authorizer.grant_store();
            let store = store_arc.lock().await;
            assert!(
                !store
                    .matching_scopes(chat, "web_fetch", Some("example.org"), None, None)
                    .is_empty(),
                "grant for another tool must stay"
            );
        }

        // Decide allow_once on fetch1 - it must succeed
        h.decide(chat, "fetch1", "allow_once").await;
        let (decision, _) = authorization_result(&mut other_call).await;
        assert_eq!(decision, "allow");
    }
}
