use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
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
}

#[derive(Clone)]
pub struct WorkerSupervisor {
    inner: Arc<Mutex<Inner>>,
}

impl WorkerSupervisor {
    pub async fn new(
        config: &Config,
        api_key: Option<String>,
        custom_command: Option<Vec<String>>,
    ) -> Result<Self, SupervisorError> {
        let command_args = if let Some(cmd) = custom_command {
            cmd
        } else if let Some(ref override_cmd) = config.worker_command {
            override_cmd.split_whitespace().map(|s| s.to_string()).collect()
        } else {
            let worker_dir = if std::path::Path::new("worker").exists() {
                "worker".to_string()
            } else if std::path::Path::new("../worker").exists() {
                "../worker".to_string()
            } else {
                "worker".to_string()
            };
            vec![
                "uv".to_string(),
                "run".to_string(),
                "--project".to_string(),
                worker_dir,
                "python".to_string(),
                "-m".to_string(),
                "nook_worker".to_string(),
            ]
        };

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

        let inner = Arc::new(Mutex::new(Inner {
            active_listeners: HashMap::new(),
            stdin_tx: None,
        }));

        let supervisor = Self { inner };

        // Start initial process and wait for ready
        let (first_ready_tx, first_ready_rx) = tokio::sync::oneshot::channel();
        let sup_clone = supervisor.clone();

        tokio::spawn(async move {
            sup_clone.supervisor_loop(command_args, env_vars, Some(first_ready_tx)).await;
        });

        match tokio::time::timeout(Duration::from_secs(60), first_ready_rx).await {
            Ok(Ok(Ok(()))) => Ok(supervisor),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err(SupervisorError::Communication("Supervisor loop terminated prematurely".to_string())),
            Err(_) => Err(SupervisorError::ReadyTimeout),
        }
    }

    async fn supervisor_loop(
        &self,
        command_args: Vec<String>,
        env_vars: HashMap<String, String>,
        mut initial_ready_tx: Option<tokio::sync::oneshot::Sender<Result<(), SupervisorError>>>,
    ) {
        let backoffs = [1, 2, 4, 8, 30];
        let mut backoff_idx = 0;

        loop {
            let spawn_res = self.spawn_and_supervise(&command_args, &env_vars, initial_ready_tx.take()).await;

            // Broadcast crash to any active listeners
            self.broadcast_crash().await;

            if let Err(e) = spawn_res {
                warn!("Worker process spawn failed: {e}");
            }

            let delay = backoffs[backoff_idx.min(backoffs.len() - 1)];
            info!(delay_secs = delay, "Waiting before restarting worker");
            tokio::time::sleep(Duration::from_secs(delay)).await;
            backoff_idx += 1;
        }
    }

    async fn spawn_and_supervise(
        &self,
        command_args: &[String],
        env_vars: &HashMap<String, String>,
        ready_notifier: Option<tokio::sync::oneshot::Sender<Result<(), SupervisorError>>>,
    ) -> Result<(), SupervisorError> {
        let mut cmd = Command::new(&command_args[0]);
        if command_args.len() > 1 {
            cmd.args(&command_args[1..]);
        }
        cmd.envs(env_vars);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        let mut child = cmd.spawn()?;
        let mut child_stdin = child.stdin.take().expect("Child stdin not piped");
        let child_stdout = child.stdout.take().expect("Child stdout not piped");

        let mut lines = BufReader::new(child_stdout).lines();

        let ready_line = match tokio::time::timeout(Duration::from_secs(60), lines.next_line()).await {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => {
                let err = SupervisorError::Communication("Worker exited before ready".to_string());
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::Communication("Worker exited before ready".to_string())));
                }
                return Err(err);
            }
            Ok(Err(e)) => {
                let err = SupervisorError::ProcessStart(e);
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::Communication("Process start error".to_string())));
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
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Ok(()));
                }
            }
            other => {
                let err = SupervisorError::Communication(format!("Expected ready event, received: {other:?}"));
                if let Some(tx) = ready_notifier {
                    let _ = tx.send(Err(SupervisorError::Communication("Unexpected event".to_string())));
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
            }
        }

        stdin_task.abort();
        Ok(())
    }

    async fn route_event(&self, event: DaemonWorkerEvent) {
        let req_id = match &event {
            DaemonWorkerEvent::MessageStarted { request_id, .. }
            | DaemonWorkerEvent::TextDelta { request_id, .. }
            | DaemonWorkerEvent::ToolCallStarted { request_id, .. }
            | DaemonWorkerEvent::ToolCallFinished { request_id, .. }
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

        let mut inner = self.inner.lock().await;
        if let Some(tx) = inner.active_listeners.get(&req_id) {
            let _ = tx.send(event).await;
        }

        if is_terminal {
            inner.active_listeners.remove(&req_id);
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
            DaemonWorkerRequest::Cancel { .. } | DaemonWorkerRequest::Shutdown => None,
        };

        let mut payload = serde_json::to_string(&request)
            .map_err(|e| SupervisorError::Communication(e.to_string()))?;
        payload.push('\n');

        let mut inner = self.inner.lock().await;
        if let Some(id) = req_id {
            inner.active_listeners.insert(id, tx);
        }

        if let Some(ref stdin_tx) = inner.stdin_tx {
            stdin_tx
                .send(payload)
                .await
                .map_err(|e| SupervisorError::Communication(e.to_string()))?;
            Ok(rx)
        } else {
            Err(SupervisorError::Communication(
                "Worker stdin not available".to_string(),
            ))
        }
    }

    pub async fn shutdown(&self) {
        let req = DaemonWorkerRequest::Shutdown;
        let mut payload = serde_json::to_string(&req).unwrap_or_default();
        payload.push('\n');

        let inner = self.inner.lock().await;
        if let Some(ref stdin_tx) = inner.stdin_tx {
            let _ = stdin_tx.send(payload).await;
        }
    }
}
