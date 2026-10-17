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
        let registry = ProviderRegistry::new(providers).unwrap();
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

    async fn deliver(&self, mut call: Call) -> Call {
        let event = recv(&mut call.worker_rx).await;
        let DaemonWorkerEvent::ExecuteTool {
            message_id,
            call_id,
            chat_id,
            tool_name,
            arguments,
            ..
        } = event
        else {
            panic!("Expected ExecuteTool, got {event:?}");
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
                execute: true,
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
