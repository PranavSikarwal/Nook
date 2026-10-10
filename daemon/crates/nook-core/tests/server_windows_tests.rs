#![cfg(windows)]

use nook_core::config::Config;
use nook_core::protocol::{ClientMessage, DaemonMessage};
use nook_core::server::{windows_pipe_name, AppState, Server};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

#[tokio::test]
async fn windows_named_pipe_round_trips_json_ping() {
    let temp_dir = tempfile::tempdir().unwrap();
    let state = Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path: temp_dir.path().join("config.toml"),
        pool: None,
        supervisor: Mutex::new(None),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let server = Server::bind(&Config::default_socket_path(), shutdown_rx, state)
        .await
        .unwrap();
    let server_task = tokio::spawn(server.run());

    let mut client = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ClientOptions::new().open(windows_pipe_name()) {
                Ok(client) => break client,
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("named-pipe server did not become available");

    let request_id = Uuid::new_v4();
    let mut request = serde_json::to_string(&ClientMessage::Ping { id: request_id }).unwrap();
    request.push('\n');
    client.write_all(request.as_bytes()).await.unwrap();

    let (reader, _writer) = tokio::io::split(client);
    let mut lines = BufReader::new(reader).lines();
    let line = tokio::time::timeout(Duration::from_secs(2), lines.next_line())
        .await
        .expect("timed out waiting for daemon response")
        .unwrap()
        .expect("daemon closed the named pipe");
    let response: DaemonMessage = serde_json::from_str(&line).unwrap();
    assert_eq!(response, DaemonMessage::Pong { id: request_id });

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
