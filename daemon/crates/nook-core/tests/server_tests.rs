use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
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
    let temp_dir = tempfile::tempdir().unwrap();
    let socket_path = temp_dir.path().join("test.sock");
    let config_path = temp_dir.path().join("config.toml");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path,
        pool: None,
        supervisor: Mutex::new(None),
        open_requests: Mutex::new(HashMap::new()),
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
