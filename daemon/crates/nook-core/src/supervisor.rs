use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStderr, Command};
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::protocol::{DaemonWorkerEvent, DaemonWorkerRequest, ErrorCode, ErrorInfo};

#[derive(thiserror::Error, Debug)]
pub enum SupervisorError {
    #[error("Worker failed to start: {0}")]
    ProcessStart(#[from] std::io::Error),
    #[error("Worker ready timeout")]
    ReadyTimeout,
    #[error("Worker communication error: {0}")]
    Communication(String),
}

struct Inner {
    active_listeners: HashMap<Uuid, mpsc::Sender<DaemonWorkerEvent>>,
    stdin_tx: Option<mpsc::Sender<String>>,
    is_shutdown: bool,
    shutdown_tx: broadcast::Sender<()>,
}

#[derive(Clone)]
pub struct WorkerSupervisor {
    inner: Arc<Mutex<Inner>>,
}

fn find_worker_directory() -> Option<PathBuf> {
    if let Ok(dir_str) = std::env::var("NOOK_WORKER_DIR") {
        let p = PathBuf::from(dir_str);
        if p.is_dir() {
            return Some(p);
        }
    }

    for candidate in &["worker", "../worker", "../../worker"] {
        let p = PathBuf::from(candidate);
        if p.is_dir() && (p.join("pyproject.toml").is_file() || p.join("src/nook_worker").is_dir())
        {
            return Some(p);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let bundle_worker = exe_dir.parent().unwrap_or(exe_dir).join("Resources/worker");
            if bundle_worker.is_dir() {
                return Some(bundle_worker);
            }
            for relative in &["../../worker", "../../../worker", "../worker"] {
                let candidate = exe_dir.join(relative);
                if candidate.is_dir() && candidate.join("pyproject.toml").is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        let candidates = [
            home_path.join(".local/share/nook/worker"),
            home_path.join("personal/Nook/worker"),
        ];
        for candidate in &candidates {
            if candidate.is_dir() && candidate.join("pyproject.toml").is_file() {
                return Some(candidate.clone());
            }
        }
    }

    None
}

fn find_uv_binary() -> String {
    for env_key in &["NOOK_UV_PATH", "UV_BIN"] {
        if let Ok(val) = std::env::var(env_key) {
            let p = PathBuf::from(val);
            if p.is_file() {
                return p.to_string_lossy().to_string();
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidates = [
            PathBuf::from(&home).join(".local/bin/uv"),
            PathBuf::from(&home).join(".cargo/bin/uv"),
            PathBuf::from(&home).join(".rtk/shims/uv"),
            PathBuf::from("/usr/local/bin/uv"),
        ];
        for candidate in &candidates {
            if candidate.is_file() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    "uv".to_string()
}

fn uses_uv_project(command_args: &[String]) -> bool {
    command_args.iter().any(|arg| arg == "run") && command_args.iter().any(|arg| arg == "--project")
}

fn e2e_log_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("NOOK_E2E_RUN_DIR").map(|dir| PathBuf::from(dir).join(name))
}

fn redact(line: &str, values: &[String]) -> String {
    values.iter().fold(line.to_owned(), |line, value| {
        if value.trim().is_empty() {
            line
        } else {
            line.replace(value, "***")
        }
    })
}

async fn capture_stderr(stderr: ChildStderr, path: PathBuf, redactions: Vec<String>) {
    let Ok(mut file) = tokio::fs::File::create(path).await else {
        return;
    };
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = redact(&line, &redactions);
        if file.write_all(line.as_bytes()).await.is_err() || file.write_all(b"\n").await.is_err() {
            return;
        }
    }
}

fn resolve_worker_command(config: &Config, custom_command: Option<Vec<String>>) -> Vec<String> {
    if let Some(cmd) = custom_command {
        return cmd;
    }
    if let Ok(cmd_str) = std::env::var("NOOK_WORKER_COMMAND") {
        return cmd_str.split_whitespace().map(|s| s.to_string()).collect();
    }
    if let Some(ref override_cmd) = config.worker_command {
        return override_cmd
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
    }

    let worker_dir = find_worker_directory().unwrap_or_else(|| PathBuf::from("worker"));

    #[cfg(windows)]
    let venv_python = worker_dir.join(".venv/Scripts/python.exe");
    #[cfg(not(windows))]
    let venv_python = worker_dir.join(".venv/bin/python");

    if venv_python.is_file() {
        return vec![
            venv_python.to_string_lossy().to_string(),
            "-m".to_string(),
            "nook_worker".to_string(),
        ];
    }

    let uv_bin = find_uv_binary();
    vec![
        uv_bin,
        "run".to_string(),
        "--project".to_string(),
        worker_dir.to_string_lossy().to_string(),
        "python".to_string(),
        "-m".to_string(),
        "nook_worker".to_string(),
    ]
}

impl WorkerSupervisor {
    pub async fn new(
        config: &Config,
        api_key: Option<String>,
        custom_command: Option<Vec<String>>,
    ) -> Result<Self, SupervisorError> {
        let command_args = resolve_worker_command(config, custom_command);

        let mut env_vars = HashMap::new();
        env_vars.insert("NOOK_BASE_URL".to_string(), config.base_url.clone());
        env_vars.insert("NOOK_MODEL".to_string(), config.model.clone());
        env_vars.insert(
            "NOOK_MAX_INPUT_TOKENS".to_string(),
            config.max_input_tokens.to_string(),
        );
        env_vars.insert(
            "NOOK_SUMMARIZE_AT_TOKENS".to_string(),
            config.summarize_at_tokens.to_string(),
        );
        env_vars.insert("NOOK_DATABASE_URL".to_string(), config.database_url.clone());
        if let Some(key) = api_key {
            env_vars.insert("NOOK_API_KEY".to_string(), key);
        }

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let inner = Arc::new(Mutex::new(Inner {
            active_listeners: HashMap::new(),
            stdin_tx: None,
            is_shutdown: false,
            shutdown_tx,
        }));

        let supervisor = Self { inner };

        // Start initial process and wait for ready
        let (first_ready_tx, first_ready_rx) = tokio::sync::oneshot::channel();
        let sup_clone = supervisor.clone();

        tokio::spawn(async move {
            sup_clone
                .supervisor_loop(command_args, env_vars, Some(first_ready_tx), shutdown_rx)
                .await;
        });

        match tokio::time::timeout(Duration::from_secs(60), first_ready_rx).await {
            Ok(Ok(Ok(()))) => Ok(supervisor),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err(SupervisorError::Communication(
                "Supervisor loop terminated prematurely".to_string(),
            )),
            Err(_) => Err(SupervisorError::ReadyTimeout),
        }
    }

    async fn supervisor_loop(
        &self,
        command_args: Vec<String>,
        env_vars: HashMap<String, String>,
        mut initial_ready_tx: Option<tokio::sync::oneshot::Sender<Result<(), SupervisorError>>>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let backoffs = [1, 2, 4, 8, 30];
        let mut backoff_idx = 0;

        loop {
            let ready_at = Arc::new(Mutex::new(None::<std::time::Instant>));
            let spawn_res = tokio::select! {
                res = self.spawn_and_supervise(
                    &command_args,
                    &env_vars,
                    initial_ready_tx.take(),
                    ready_at.clone(),
                ) => res,
                _ = shutdown_rx.recv() => {
                    info!("Supervisor loop received shutdown signal during supervise");
                    self.broadcast_crash().await;
                    return;
                }
            };

            // Broadcast crash or shutdown error to any remaining active listeners
            self.broadcast_crash().await;

            // Check if supervisor is shut down
            {
                let inner = self.inner.lock().await;
                if inner.is_shutdown {
                    info!("Supervisor shut down; exiting loop");
                    return;
                }
            }

            if let Err(e) = spawn_res {
                warn!("Worker process spawn failed: {e}");
            }

            // Reset backoff delay after 60 seconds of healthy running post-ready
            let ran_healthy = if let Some(ready_time) = *ready_at.lock().await {
                ready_time.elapsed() >= Duration::from_secs(60)
            } else {
                false
            };

            if ran_healthy {
                backoff_idx = 0;
            }

            let delay = backoffs[backoff_idx.min(backoffs.len() - 1)];
            info!(delay_secs = delay, "Waiting before restarting worker");

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(delay)) => {},
                _ = shutdown_rx.recv() => {
                    info!("Supervisor loop received shutdown while sleeping");
                    self.broadcast_crash().await;
                    return;
                }
            }
            backoff_idx += 1;
        }
    }

    async fn spawn_and_supervise(
        &self,
        command_args: &[String],
        env_vars: &HashMap<String, String>,
        ready_notifier: Option<tokio::sync::oneshot::Sender<Result<(), SupervisorError>>>,
        ready_at: Arc<Mutex<Option<std::time::Instant>>>,
    ) -> Result<(), SupervisorError> {
        let mut shutdown_rx = {
            let inner = self.inner.lock().await;
            inner.shutdown_tx.subscribe()
        };
        let mut cmd = Command::new(&command_args[0]);
        if command_args.len() > 1 {
            cmd.args(&command_args[1..]);
        }
        cmd.envs(env_vars);
        if uses_uv_project(command_args) {
            cmd.env_remove("VIRTUAL_ENV");
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        let worker_log_path = e2e_log_path("worker-stderr.log");
        if worker_log_path.is_some() {
            cmd.stderr(Stdio::piped());
        } else {
            cmd.stderr(Stdio::inherit());
        }

        let mut child = cmd.spawn()?;
        let worker_stderr_task = match (child.stderr.take(), worker_log_path) {
            (Some(stderr), Some(path)) => {
                let redactions = env_vars.values().cloned().collect();
                Some(tokio::spawn(capture_stderr(stderr, path, redactions)))
            }
            _ => None,
        };
        let mut child_stdin = child.stdin.take().expect("Child stdin not piped");
        let child_stdout = child.stdout.take().expect("Child stdout not piped");

        let mut lines = BufReader::new(child_stdout).lines();

        let ready_line = match tokio::time::timeout(Duration::from_secs(60), lines.next_line())
            .await
        {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => {
                let err = SupervisorError::Communication("Worker exited before ready".to_string());
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::Communication(
                        "Worker exited before ready".to_string(),
                    )));
                }
                return Err(err);
            }
            Ok(Err(e)) => {
                let err = SupervisorError::ProcessStart(e);
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::Communication(
                        "Process start error".to_string(),
                    )));
                }
                return Err(err);
            }
            Err(_) => {
                let _ = child.kill().await;
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::ReadyTimeout));
                }
                return Err(SupervisorError::ReadyTimeout);
            }
        };

        let event: DaemonWorkerEvent = serde_json::from_str(&ready_line)
            .map_err(|e| SupervisorError::Communication(format!("Invalid ready JSON: {e}")))?;

        match event {
            DaemonWorkerEvent::Ready { version } => {
                info!(version = %version, "Worker process ready");
                *ready_at.lock().await = Some(std::time::Instant::now());
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Ok(()));
                }
            }
            other => {
                let err = SupervisorError::Communication(format!(
                    "Expected ready event, received: {other:?}"
                ));
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::Communication(
                        "Unexpected event".to_string(),
                    )));
                }
                return Err(err);
            }
        }

        // Stdin writing channel
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(100);
        {
            let mut inner = self.inner.lock().await;
            inner.stdin_tx = Some(stdin_tx);
        }

        let stdin_task = tokio::spawn(async move {
            while let Some(line) = stdin_rx.recv().await {
                if child_stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if child_stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        // Read stdout events
        loop {
            tokio::select! {
                line_res = lines.next_line() => {
                    match line_res {
                        Ok(Some(line)) => {
                            if let Ok(event) = serde_json::from_str::<DaemonWorkerEvent>(&line) {
                                self.route_event(event).await;
                            }
                        }
                        Ok(None) | Err(_) => {
                            warn!("Worker stdout EOF, exiting supervise loop");
                            break;
                        }
                    }
                }
                _ = child.wait() => {
                    warn!("Worker child process exited");
                    break;
                }
                _ = shutdown_rx.recv() => {
                    info!("Supervisor received shutdown signal during worker execution");
                    let _ = child.kill().await;
                    break;
                }
            }
        }

        stdin_task.abort();
        if let Some(task) = worker_stderr_task {
            let _ = task.await;
        }
        Ok(())
    }

    async fn route_event(&self, event: DaemonWorkerEvent) {
        let req_id = match &event {
            DaemonWorkerEvent::MessageStarted { request_id, .. }
            | DaemonWorkerEvent::TextDelta { request_id, .. }
            | DaemonWorkerEvent::ToolCallStarted { request_id, .. }
            | DaemonWorkerEvent::ToolCallFinished { request_id, .. }
            | DaemonWorkerEvent::ApprovalRequested { request_id, .. }
            | DaemonWorkerEvent::MessageFinished { request_id, .. }
            | DaemonWorkerEvent::TitleReady { request_id, .. }
            | DaemonWorkerEvent::Deleted { request_id, .. }
            | DaemonWorkerEvent::Error { request_id, .. } => *request_id,
            DaemonWorkerEvent::Ready { .. } => return,
        };

        let is_terminal = matches!(
            &event,
            DaemonWorkerEvent::MessageFinished { .. }
                | DaemonWorkerEvent::TitleReady { .. }
                | DaemonWorkerEvent::Deleted { .. }
                | DaemonWorkerEvent::Error { .. }
        );

        let sender = {
            let mut inner = self.inner.lock().await;
            if is_terminal {
                inner.active_listeners.remove(&req_id)
            } else {
                inner.active_listeners.get(&req_id).cloned()
            }
        };

        if let Some(tx) = sender {
            let _ = tx.send(event).await;
        }
    }

    async fn broadcast_crash(&self) {
        let mut inner = self.inner.lock().await;
        inner.stdin_tx = None;
        let listeners = std::mem::take(&mut inner.active_listeners);

        for (req_id, tx) in listeners {
            let err_event = DaemonWorkerEvent::Error {
                request_id: req_id,
                error: ErrorInfo {
                    code: ErrorCode::WorkerCrashed,
                    message: "Worker process crashed".to_string(),
                    retryable: true,
                },
            };
            let _ = tx.send(err_event).await;
        }
    }

    pub async fn send_request(
        &self,
        request: DaemonWorkerRequest,
    ) -> Result<mpsc::Receiver<DaemonWorkerEvent>, SupervisorError> {
        let (tx, rx) = mpsc::channel(100);

        let req_id = match &request {
            DaemonWorkerRequest::Run { request_id, .. }
            | DaemonWorkerRequest::Title { request_id, .. }
            | DaemonWorkerRequest::DeleteChat { request_id, .. } => Some(*request_id),
            // A decision shares the running reply's request_id. Registering a
            // listener for it would replace the reply's sender.
            DaemonWorkerRequest::ApprovalDecision { .. }
            | DaemonWorkerRequest::Cancel { .. }
            | DaemonWorkerRequest::Shutdown => None,
        };

        let mut payload = serde_json::to_string(&request)
            .map_err(|e| SupervisorError::Communication(e.to_string()))?;
        payload.push('\n');

        let stdin_tx = {
            let inner = self.inner.lock().await;
            if inner.is_shutdown {
                return Err(SupervisorError::Communication(
                    "Worker supervisor is shut down".to_string(),
                ));
            }
            match &inner.stdin_tx {
                Some(tx) => tx.clone(),
                None => {
                    return Err(SupervisorError::Communication(
                        "Worker stdin not available".to_string(),
                    ));
                }
            }
        };

        stdin_tx
            .send(payload)
            .await
            .map_err(|e| SupervisorError::Communication(e.to_string()))?;

        if let Some(id) = req_id {
            let mut inner = self.inner.lock().await;
            inner.active_listeners.insert(id, tx);
        }

        Ok(rx)
    }

    pub async fn shutdown(&self) {
        let req = DaemonWorkerRequest::Shutdown;
        let mut payload = serde_json::to_string(&req).unwrap_or_default();
        payload.push('\n');

        let (stdin_tx, shutdown_tx) = {
            let mut inner = self.inner.lock().await;
            inner.is_shutdown = true;
            (inner.stdin_tx.clone(), inner.shutdown_tx.clone())
        };

        let _ = shutdown_tx.send(());

        if let Some(ref stdin) = stdin_tx {
            let _ = stdin.send(payload).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::uses_uv_project;

    #[test]
    fn uv_project_launch_ignores_active_virtual_environment() {
        let uv_project_command = vec![
            "uv".to_string(),
            "run".to_string(),
            "--project".to_string(),
            "worker".to_string(),
        ];
        let custom_command = vec!["python".to_string(), "worker.py".to_string()];

        assert!(uses_uv_project(&uv_project_command));
        assert!(!uses_uv_project(&custom_command));
    }
}
