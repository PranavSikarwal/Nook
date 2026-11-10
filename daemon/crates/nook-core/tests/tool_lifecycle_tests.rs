#![cfg(unix)]

use nook_core::config::Config;
use nook_core::policy_assets::PolicyAssets;
use nook_core::protocol::{
    ClientMessage, DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest, ErrorCode, ErrorInfo,
    FinishStatus,
};
use nook_core::server::{
    forward_text_delta, forward_tool_call_event, forward_unless_cancelled, AppState, Server,
};
use nook_core::supervisor::WorkerSupervisor;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

struct Request {
    client_id: Uuid,
    worker_id: Uuid,
    worker_rx: mpsc::Receiver<DaemonWorkerEvent>,
}

struct Harness {
    state: Arc<AppState>,
    panel_tx: mpsc::Sender<DaemonMessage>,
    panel_rx: mpsc::Receiver<DaemonMessage>,
}

async fn recv(rx: &mut mpsc::Receiver<DaemonWorkerEvent>) -> DaemonWorkerEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("Timeout waiting for a Worker event")
        .expect("Worker channel closed")
}

impl Harness {
    async fn new() -> Self {
        let policy_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../policy");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fake_worker.py");
        let supervisor = WorkerSupervisor::new(
            &Config::default(),
            None,
            Some(vec![
                "python3".into(),
                script.to_string_lossy().into_owned(),
            ]),
        )
        .await
        .unwrap();
        let state = Arc::new(AppState {
            config: RwLock::new(Config::default()),
            config_path: PathBuf::from("/tmp/nook-unused-config.toml"),
            pool: nook_core::server::DbPool::default(),
            supervisor: Mutex::new(Some(supervisor)),
            preserve_worker_on_settings_update: false,
            open_requests: Mutex::new(HashMap::new()),
            pending_approvals: Mutex::new(HashMap::new()),
            cancelled_requests: Mutex::new(HashSet::new()),
            approval_timeout: nook_core::tool_approval::APPROVAL_TIMEOUT,
            providers: nook_core::providers::ProviderHandle::new(
                nook_core::providers::ProviderRegistry::empty(),
            ),
            running_tools: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            policy: Arc::new(PolicyAssets::load(&policy_dir).unwrap()),
        });
        let (panel_tx, panel_rx) = mpsc::channel(32);
        Self {
            state,
            panel_tx,
            panel_rx,
        }
    }

    async fn start(&self, text: &str) -> Request {
        let client_id = Uuid::new_v4();
        let worker_id = Uuid::new_v4();
        self.state
            .open_requests
            .lock()
            .await
            .insert(client_id, worker_id);
        let sup = self.state.supervisor.lock().await.clone().unwrap();
        let mut worker_rx = sup
            .send_request(DaemonWorkerRequest::Run {
                request_id: worker_id,
                chat_id: Uuid::new_v4(),
                text: text.to_string(),
                attachments: vec![],
            })
            .await
            .unwrap();
        assert!(matches!(
            recv(&mut worker_rx).await,
            DaemonWorkerEvent::MessageStarted { .. }
        ));
        Request {
            client_id,
            worker_id,
            worker_rx,
        }
    }

    async fn forward(&self, request: &mut Request, chat_id: Uuid) {
        let event = recv(&mut request.worker_rx).await;
        forward_tool_call_event(
            &self.state,
            request.client_id,
            chat_id,
            Uuid::new_v4(),
            event,
            &self.panel_tx,
        )
        .await;
    }

    async fn expect_panel(&mut self) -> DaemonMessage {
        tokio::time::timeout(Duration::from_secs(5), self.panel_rx.recv())
            .await
            .expect("Timeout waiting for the Panel")
            .expect("Panel channel closed")
    }

    fn assert_panel_silent(&mut self) {
        assert!(self.panel_rx.try_recv().is_err(), "Panel got a message");
    }
}

async fn cancel_through_server(state: &Arc<AppState>, target_id: Uuid) {
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_l_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server = Server::bind(&socket_path, shutdown_rx, state.clone())
        .await
        .unwrap();
    let task = tokio::spawn(async move {
        let _ = server.run().await;
    });
    let stream = UnixStream::connect(&socket_path).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let cancel_id = Uuid::new_v4();
    let mut payload = serde_json::to_string(&ClientMessage::Cancel {
        id: cancel_id,
        target_id,
    })
    .unwrap();
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(reader).lines();
    let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<DaemonMessage>(&line).unwrap(),
        DaemonMessage::Cancelled {
            id: cancel_id,
            target_id
        }
    );
    shutdown_tx.send(()).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn tool_events_reach_the_panel_with_the_client_request_id() {
    let mut h = Harness::new().await;
    let mut request = h.start("tools:call_1").await;
    let chat_id = Uuid::new_v4();

    h.forward(&mut request, chat_id).await;
    h.forward(&mut request, chat_id).await;

    match h.expect_panel().await {
        DaemonMessage::ToolCallStarted {
            id,
            chat_id: got_chat,
            call_id,
            name,
            arguments,
            ..
        } => {
            assert_eq!(id, request.client_id);
            assert_ne!(id, request.worker_id);
            assert_eq!(got_chat, chat_id);
            assert_eq!(call_id, "call_1");
            assert_eq!(name, "nook:web_fetch");
            assert_eq!(arguments, r#"{"url":"https://example.com/"}"#);
        }
        other => panic!("Expected ToolCallStarted, got {other:?}"),
    }
    match h.expect_panel().await {
        DaemonMessage::ToolCallFinished {
            id,
            call_id,
            result,
            ..
        } => {
            assert_eq!(id, request.client_id);
            assert_eq!(call_id, "call_1");
            assert_eq!(result, "page text");
        }
        other => panic!("Expected ToolCallFinished, got {other:?}"),
    }
}

#[tokio::test]
async fn tool_event_after_cancel_does_not_reach_the_panel() {
    let mut h = Harness::new().await;
    let mut request = h.start("tools_on_cancel:call_late").await;

    cancel_through_server(&h.state, request.client_id).await;
    h.forward(&mut request, Uuid::new_v4()).await;

    h.assert_panel_silent();
    assert!(matches!(
        recv(&mut request.worker_rx).await,
        DaemonWorkerEvent::MessageFinished {
            status: FinishStatus::Cancelled,
            ..
        }
    ));
}

#[tokio::test]
async fn a_second_request_does_not_receive_events_of_a_cancelled_one() {
    let mut h = Harness::new().await;
    let mut cancelled = h.start("tools_on_cancel:call_old").await;
    cancel_through_server(&h.state, cancelled.client_id).await;
    let mut second = h.start("tools:call_new").await;

    h.forward(&mut cancelled, Uuid::new_v4()).await;
    h.assert_panel_silent();

    h.forward(&mut second, Uuid::new_v4()).await;
    match h.expect_panel().await {
        DaemonMessage::ToolCallStarted { id, call_id, .. } => {
            assert_eq!(id, second.client_id);
            assert_eq!(call_id, "call_new");
        }
        other => panic!("Expected ToolCallStarted, got {other:?}"),
    }
    h.forward(&mut second, Uuid::new_v4()).await;
    match h.expect_panel().await {
        DaemonMessage::ToolCallFinished { id, call_id, .. } => {
            assert_eq!(id, second.client_id);
            assert_eq!(call_id, "call_new");
        }
        other => panic!("Expected ToolCallFinished, got {other:?}"),
    }
    h.assert_panel_silent();
}

#[tokio::test]
async fn text_before_cancel_is_forwarded_and_accumulated() {
    let mut h = Harness::new().await;
    let client_id = Uuid::new_v4();
    let chat_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let mut full_text = String::new();

    forward_text_delta(
        &h.state,
        client_id,
        chat_id,
        message_id,
        "hello".to_string(),
        &mut full_text,
        &h.panel_tx,
    )
    .await;

    assert_eq!(full_text, "hello");
    assert_eq!(
        h.expect_panel().await,
        DaemonMessage::TextDelta {
            id: client_id,
            chat_id,
            message_id,
            text: "hello".to_string(),
        }
    );
}

#[tokio::test]
async fn text_after_cancel_is_accumulated_but_not_forwarded() {
    let mut h = Harness::new().await;
    let client_id = Uuid::new_v4();
    let mut full_text = String::from("before ");
    h.state.cancelled_requests.lock().await.insert(client_id);

    forward_text_delta(
        &h.state,
        client_id,
        Uuid::new_v4(),
        Uuid::new_v4(),
        "after".to_string(),
        &mut full_text,
        &h.panel_tx,
    )
    .await;

    assert_eq!(full_text, "before after");
    h.assert_panel_silent();
}

#[tokio::test]
async fn text_of_another_request_is_forwarded_while_one_is_cancelled() {
    let mut h = Harness::new().await;
    let cancelled = Uuid::new_v4();
    let other = Uuid::new_v4();
    h.state.cancelled_requests.lock().await.insert(cancelled);
    let mut full_text = String::new();

    forward_text_delta(
        &h.state,
        other,
        Uuid::new_v4(),
        Uuid::new_v4(),
        "live".to_string(),
        &mut full_text,
        &h.panel_tx,
    )
    .await;

    assert!(matches!(
        h.expect_panel().await,
        DaemonMessage::TextDelta { id, .. } if id == other
    ));
}

#[tokio::test]
async fn message_started_and_error_after_cancel_are_not_forwarded() {
    let mut h = Harness::new().await;
    let client_id = Uuid::new_v4();
    let chat_id = Uuid::new_v4();
    h.state.cancelled_requests.lock().await.insert(client_id);

    forward_unless_cancelled(
        &h.state,
        client_id,
        DaemonMessage::MessageStarted {
            id: client_id,
            chat_id,
            message_id: Uuid::new_v4(),
        },
        &h.panel_tx,
    )
    .await;
    forward_unless_cancelled(
        &h.state,
        client_id,
        DaemonMessage::Error {
            id: client_id,
            error: ErrorInfo {
                code: ErrorCode::Internal,
                message: "late".to_string(),
                retryable: false,
            },
        },
        &h.panel_tx,
    )
    .await;

    h.assert_panel_silent();
}

#[tokio::test]
async fn message_started_is_forwarded_when_the_request_is_live() {
    let mut h = Harness::new().await;
    let client_id = Uuid::new_v4();
    let message = DaemonMessage::MessageStarted {
        id: client_id,
        chat_id: Uuid::new_v4(),
        message_id: Uuid::new_v4(),
    };

    forward_unless_cancelled(&h.state, client_id, message.clone(), &h.panel_tx).await;

    assert_eq!(h.expect_panel().await, message);
}
