use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use nook_core::config::Config;
use nook_core::protocol::{ClientMessage, DaemonMessage};
use nook_core::server::{AppState, Server};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

#[tokio::test]
async fn test_server_ping_pong_and_permissions() {
    let socket_path = PathBuf::from(format!("/tmp/nk_p_{}.sock", &Uuid::new_v4().to_string()[..8]));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        open_requests: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    let server = Server::bind(&socket_path, shutdown_rx, state).await.unwrap();

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
    assert!(!socket_path.exists(), "Socket file should be removed on shutdown");
}

#[tokio::test]
async fn test_server_returns_error_for_unknown_type_with_id() {
    let socket_path = PathBuf::from(format!("/tmp/nk_e_{}.sock", &Uuid::new_v4().to_string()[..8]));
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        open_requests: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    let server = Server::bind(&socket_path, shutdown_rx, state).await.unwrap();
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
