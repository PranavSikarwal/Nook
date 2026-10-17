#![cfg(unix)]

use nook_core::config::Config;
use nook_core::policy_assets::PolicyAssets;
use nook_core::server::{AppState, Server};
use nook_core::tool_approval::APPROVAL_TIMEOUT;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixListener;
use tokio::sync::{broadcast, Mutex, RwLock};

fn state() -> Arc<AppState> {
    let policy_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../policy");
    Arc::new(AppState {
        config: RwLock::new(Config::default()),
        config_path: PathBuf::from("/tmp/nook-unused-config.toml"),
        pool: nook_core::server::DbPool::default(),
        supervisor: Mutex::new(None),
        preserve_worker_on_settings_update: false,
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(HashSet::new()),
        approval_timeout: APPROVAL_TIMEOUT,
        providers: std::sync::Arc::new(nook_core::providers::ProviderRegistry::empty()),
        running_tools: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        policy: Arc::new(PolicyAssets::load(&policy_dir).unwrap()),
    })
}

fn socket_in(label: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("nook-sock-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    (dir.join("daemon.sock"), dir)
}

async fn bind(path: &Path) -> (Server, broadcast::Sender<()>) {
    let (tx, rx) = broadcast::channel(1);
    let server = Server::bind(path, rx, state()).await.unwrap();
    (server, tx)
}

#[tokio::test]
async fn bind_refuses_a_socket_that_another_daemon_listens_on() {
    let (path, dir) = socket_in("live");
    let (first, _tx) = bind(&path).await;
    let task = tokio::spawn(first.run());

    let (_tx2, rx2) = broadcast::channel(1);
    let second = Server::bind(&path, rx2, state()).await;

    assert!(second.is_err());
    assert!(second
        .err()
        .unwrap()
        .to_string()
        .contains("already listening"));
    assert!(path.exists());
    task.abort();
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn bind_replaces_a_stale_socket_file() {
    let (path, dir) = socket_in("stale");
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    assert!(path.exists());

    let (server, _tx) = bind(&path).await;

    assert_eq!(server.socket_path(), path);
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn shutdown_removes_the_socket_it_created() {
    let (path, dir) = socket_in("own");
    let (server, tx) = bind(&path).await;
    let task = tokio::spawn(server.run());

    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert!(!path.exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn shutdown_keeps_a_socket_that_another_listener_took_over() {
    let (path, dir) = socket_in("taken");
    let (server, tx) = bind(&path).await;
    let task = tokio::spawn(server.run());
    std::fs::remove_file(&path).unwrap();
    let _replacement = UnixListener::bind(&path).unwrap();

    tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert!(path.exists());
    std::fs::remove_dir_all(&dir).ok();
}
