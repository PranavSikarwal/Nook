#![cfg(unix)]

use nook_core::config::Config;
use nook_core::protocol::{ClientMessage, DaemonMessage};
use nook_core::server::{AppState, PendingApprovalEntry, Server};
use nook_core::supervisor::WorkerSupervisor;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

#[tokio::test]
async fn test_server_ping_pong_and_permissions() {
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_p_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    let server = Server::bind(&socket_path, shutdown_rx, state)
        .await
        .unwrap();

    // Verify file mode 0600
    let metadata = std::fs::metadata(&socket_path).unwrap();
    let permissions = metadata.permissions();
    assert_eq!(
        permissions.mode() & 0o777,
        0o600,
        "Socket permissions should be 0600"
    );

    let server_task = tokio::spawn(server.run());

    // Connect as client
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let ping_id = Uuid::new_v4();
    let ping_req = ClientMessage::Ping { id: ping_id };
    let mut payload = serde_json::to_string(&ping_req).unwrap();
    payload.push('\n');

    stream.write_all(payload.as_bytes()).await.unwrap();

    let (reader, _client_writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let reply_line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("Timeout waiting for reply")
        .unwrap()
        .expect("Expected a reply line");

    let reply: DaemonMessage = serde_json::from_str(&reply_line).unwrap();
    assert_eq!(reply, DaemonMessage::Pong { id: ping_id });

    // Send shutdown signal
    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();

    // Verify socket file was cleaned up
    assert!(
        !socket_path.exists(),
        "Socket file should be removed on shutdown"
    );
}

#[tokio::test]
async fn test_server_returns_error_for_unknown_type_with_id() {
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_e_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    let server = Server::bind(&socket_path, shutdown_rx, state)
        .await
        .unwrap();
    let server_task = tokio::spawn(server.run());

    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let req_id = Uuid::new_v4();
    let invalid_payload = format!(r#"{{"type":"unknown_type","id":"{req_id}"}}"#);

    let mut payload = invalid_payload;
    payload.push('\n');

    stream.write_all(payload.as_bytes()).await.unwrap();

    let (reader, _client_writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let reply_line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("Timeout waiting for reply")
        .unwrap()
        .expect("Expected a reply line");

    let reply: DaemonMessage = serde_json::from_str(&reply_line).unwrap();
    match reply {
        DaemonMessage::Error { id, error } => {
            assert_eq!(id, req_id);
            assert_eq!(error.code, nook_core::protocol::ErrorCode::InvalidRequest);
        }
        other => panic!("Expected Error response, got {other:?}"),
    }

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[test]
fn test_attachment_validation_rules() {
    use nook_core::protocol::{Attachment, AttachmentKind};
    use nook_core::server::validate_attachment;

    let valid_img = Attachment {
        id: Uuid::new_v4(),
        kind: AttachmentKind::Image,
        name: "test.png".to_string(),
        mime: "image/png".to_string(),
        size_bytes: 1024,
        path: "/path/to/test.png".to_string(),
    };
    assert!(validate_attachment(&valid_img).is_ok());

    let oversized_img = Attachment {
        id: Uuid::new_v4(),
        kind: AttachmentKind::Image,
        name: "big.png".to_string(),
        mime: "image/png".to_string(),
        size_bytes: 10 * 1024 * 1024 + 1,
        path: "/path/to/big.png".to_string(),
    };
    assert!(validate_attachment(&oversized_img).is_err());

    let invalid_mime_img = Attachment {
        id: Uuid::new_v4(),
        kind: AttachmentKind::Image,
        name: "test.exe".to_string(),
        mime: "application/x-msdownload".to_string(),
        size_bytes: 1024,
        path: "/path/to/test.exe".to_string(),
    };
    assert!(validate_attachment(&invalid_mime_img).is_err());

    let valid_pdf = Attachment {
        id: Uuid::new_v4(),
        kind: AttachmentKind::Pdf,
        name: "doc.pdf".to_string(),
        mime: "application/pdf".to_string(),
        size_bytes: 5000,
        path: "/path/to/doc.pdf".to_string(),
    };
    assert!(validate_attachment(&valid_pdf).is_ok());

    let invalid_pdf_mime = Attachment {
        id: Uuid::new_v4(),
        kind: AttachmentKind::Pdf,
        name: "fake.pdf".to_string(),
        mime: "text/plain".to_string(),
        size_bytes: 5000,
        path: "/path/to/fake.pdf".to_string(),
    };
    assert!(validate_attachment(&invalid_pdf_mime).is_err());
}

#[tokio::test]
async fn test_server_approval_decision_rejects_unknown_call_id() {
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_a_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    let server = Server::bind(&socket_path, shutdown_rx, state)
        .await
        .unwrap();
    let server_task = tokio::spawn(server.run());

    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let req_id = Uuid::new_v4();
    let decision = ClientMessage::ApprovalDecision {
        id: req_id,
        chat_id: Uuid::new_v4(),
        call_id: "unknown_call_123".to_string(),
        action: "allow_once".to_string(),
    };

    let mut payload = serde_json::to_string(&decision).unwrap();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).await.unwrap();

    let (reader, _client_writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let reply_line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("Timeout waiting for reply")
        .unwrap()
        .expect("Expected a reply line");

    let reply: DaemonMessage = serde_json::from_str(&reply_line).unwrap();
    match reply {
        DaemonMessage::Error { id, error } => {
            assert_eq!(id, req_id);
            assert_eq!(error.code, nook_core::protocol::ErrorCode::InvalidRequest);
            assert!(error.message.contains("Unknown or invalid"));
        }
        other => panic!("Expected Error response, got {other:?}"),
    }

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_server_approval_forward_failure_keeps_pending_entry() {
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_f_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let fake_worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_worker.py");
    let supervisor = WorkerSupervisor::new(
        &Config::default(),
        None,
        Some(vec![
            "python3".into(),
            fake_worker.to_string_lossy().into_owned(),
        ]),
    )
    .await
    .unwrap();
    supervisor.shutdown().await;

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(Some(supervisor)),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });
    let chat_id = Uuid::new_v4();
    let call_id = "call_forward_failure".to_string();
    state.pending_approvals.lock().await.insert(
        call_id.clone(),
        PendingApprovalEntry {
            chat_id,
            client_request_id: Uuid::new_v4(),
            worker_request_id: Uuid::new_v4(),
            decision_in_flight: false,
        },
    );

    let server = Server::bind(&socket_path, shutdown_rx, state.clone())
        .await
        .unwrap();
    let server_task = tokio::spawn(server.run());
    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let decision_id = Uuid::new_v4();
    let decision = ClientMessage::ApprovalDecision {
        id: decision_id,
        chat_id,
        call_id: call_id.clone(),
        action: "allow_once".to_string(),
    };
    let mut payload = serde_json::to_string(&decision).unwrap();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).await.unwrap();

    let (reader, _client_writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let reply_line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("Timeout waiting for reply")
        .unwrap()
        .expect("Expected a Worker forwarding error");
    let reply: DaemonMessage = serde_json::from_str(&reply_line).unwrap();
    match reply {
        DaemonMessage::Error { id, error } => {
            assert_eq!(id, decision_id);
            assert!(error.retryable);
            assert!(error.message.contains("Failed to forward"));
        }
        other => panic!("Expected Error response, got {other:?}"),
    }

    assert!(state.pending_approvals.lock().await.contains_key(&call_id));
    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn test_server_approval_decision_rejects_in_flight_duplicate() {
    let chat_id = Uuid::new_v4();
    let call_id = "call_in_flight".to_string();
    let worker_request_id = Uuid::new_v4();
    let mut pending_approvals = HashMap::new();
    pending_approvals.insert(
        call_id.clone(),
        PendingApprovalEntry {
            chat_id,
            client_request_id: Uuid::new_v4(),
            worker_request_id,
            decision_in_flight: false,
        },
    );
    let approvals = Arc::new(Mutex::new(pending_approvals));
    let claim_barrier = Arc::new(tokio::sync::Barrier::new(3));

    let first_approvals = approvals.clone();
    let first_barrier = claim_barrier.clone();
    let first_call_id = call_id.clone();
    let first_claim = async move {
        first_barrier.wait().await;
        let mut approvals = first_approvals.lock().await;
        nook_core::server::claim_pending_approval(&mut approvals, &first_call_id, chat_id)
    };

    let second_approvals = approvals.clone();
    let second_barrier = claim_barrier.clone();
    let second_call_id = call_id.clone();
    let second_claim = async move {
        second_barrier.wait().await;
        let mut approvals = second_approvals.lock().await;
        nook_core::server::claim_pending_approval(&mut approvals, &second_call_id, chat_id)
    };

    let claims = tokio::join!(first_claim, second_claim, claim_barrier.wait());

    let results = [claims.0, claims.1];
    assert_eq!(results.iter().filter(|claim| claim.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|claim| claim.is_err()).count(), 1);
    assert!(
        approvals
            .lock()
            .await
            .get(&call_id)
            .unwrap()
            .decision_in_flight
    );
    assert!(results.contains(&Ok(worker_request_id)));
    assert!(results.contains(&Err("Approval decision is already being forwarded")));
}

#[tokio::test]
async fn test_server_approval_decision_mismatched_chat_is_nondestructive() {
    let socket_path = PathBuf::from(format!(
        "/tmp/nk_m_{}.sock",
        &Uuid::new_v4().to_string()[..8]
    ));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    let legitimate_chat_id = Uuid::new_v4();
    let client_req_id = Uuid::new_v4();
    let worker_req_id = Uuid::new_v4();
    let test_call_id = "call_safe_123".to_string();

    {
        let mut approvals = state.pending_approvals.lock().await;
        approvals.insert(
            test_call_id.clone(),
            nook_core::server::PendingApprovalEntry {
                chat_id: legitimate_chat_id,
                client_request_id: client_req_id,
                worker_request_id: worker_req_id,
                decision_in_flight: false,
            },
        );
    }

    let server = Server::bind(&socket_path, shutdown_rx, state.clone())
        .await
        .unwrap();
    let server_task = tokio::spawn(server.run());

    let mut stream = UnixStream::connect(&socket_path).await.unwrap();
    let malicious_decision = ClientMessage::ApprovalDecision {
        id: Uuid::new_v4(),
        chat_id: Uuid::new_v4(), // Different chat ID
        call_id: test_call_id.clone(),
        action: "deny".to_string(),
    };

    let mut payload = serde_json::to_string(&malicious_decision).unwrap();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).await.unwrap();

    let (reader, _client_writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let reply_line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("Timeout waiting for reply")
        .unwrap()
        .expect("Expected a reply line");

    let reply: DaemonMessage = serde_json::from_str(&reply_line).unwrap();
    assert!(matches!(reply, DaemonMessage::Error { .. }));

    // Verify pending approval was NOT consumed or removed
    {
        let approvals = state.pending_approvals.lock().await;
        assert!(approvals.contains_key(&test_call_id));
        let entry = approvals.get(&test_call_id).unwrap();
        assert_eq!(entry.chat_id, legitimate_chat_id);
    }

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
