use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
#[cfg(windows)]
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
#[cfg(windows)]
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::config::Config;
use crate::db;
use crate::keychain;
use crate::protocol::{
    Attachment, ClientMessage, DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest, ErrorCode,
    ErrorInfo, FinishStatus, MessageStatus,
};
use crate::supervisor::WorkerSupervisor;

#[derive(Debug, Clone)]
pub struct PendingApprovalEntry {
    pub chat_id: Uuid,
    pub client_request_id: Uuid,
    pub worker_request_id: Uuid,
    pub decision_in_flight: bool,
}

pub fn claim_pending_approval(
    approvals: &mut HashMap<String, PendingApprovalEntry>,
    call_id: &str,
    chat_id: Uuid,
) -> Result<Uuid, &'static str> {
    match approvals.get_mut(call_id) {
        Some(entry) if entry.chat_id != chat_id => {
            Err("Approval call ID does not belong to the specified chat")
        }
        Some(entry) if entry.decision_in_flight => {
            Err("Approval decision is already being forwarded")
        }
        Some(entry) => {
            entry.decision_in_flight = true;
            Ok(entry.worker_request_id)
        }
        None => Err("Unknown or invalid approval call id"),
    }
}

pub struct AppState {
    pub config: RwLock<Config>,
    pub config_path: PathBuf,
    pub pool: Option<PgPool>,
    pub supervisor: Mutex<Option<WorkerSupervisor>>,
    pub preserve_worker_on_settings_update: bool,
    pub open_requests: Mutex<HashMap<Uuid, Uuid>>, // maps client id -> worker request_id
    pub pending_approvals: Mutex<HashMap<String, PendingApprovalEntry>>, // maps call_id -> entry
    pub cancelled_requests: Mutex<HashSet<Uuid>>,
}

pub struct Server {
    socket_path: PathBuf,
    #[cfg(unix)]
    listener: UnixListener,
    #[cfg(windows)]
    pipe_server: NamedPipeServer,
    shutdown_rx: broadcast::Receiver<()>,
    state: Arc<AppState>,
}

#[cfg(windows)]
fn current_user_sid_string() -> std::io::Result<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::null_mut;

    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut required = 0;
    unsafe {
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut required);
    }
    if required == 0 {
        unsafe { CloseHandle(token) };
        return Err(std::io::Error::last_os_error());
    }

    let mut token_user = vec![0u8; required as usize];
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            token_user.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    };
    unsafe { CloseHandle(token) };
    if result == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let user = unsafe { &*(token_user.as_ptr().cast::<TOKEN_USER>()) };
    let mut sid_string = null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_string) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let sid_len = unsafe {
        (0..)
            .take_while(|offset| *sid_string.add(*offset) != 0)
            .count()
    };
    let sid = OsString::from_wide(unsafe { std::slice::from_raw_parts(sid_string, sid_len) })
        .to_string_lossy()
        .into_owned();
    unsafe { LocalFree(sid_string.cast()) };
    Ok(sid)
}

#[cfg(windows)]
pub fn windows_pipe_name() -> std::io::Result<String> {
    Ok(format!(
        r"\\.\pipe\Nook-daemon-{}",
        current_user_sid_string()?
    ))
}

impl Server {
    pub async fn bind(
        socket_path: &Path,
        shutdown_rx: broadcast::Receiver<()>,
        state: Arc<AppState>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(unix)]
        {
            if socket_path.exists() {
                fs::remove_file(socket_path)?;
            } else if let Some(parent) = socket_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let listener = {
                let old_umask = unsafe { libc::umask(0o177) };
                let bind_res = UnixListener::bind(socket_path);
                unsafe { libc::umask(old_umask) };
                bind_res?
            };

            info!(path = %socket_path.display(), "Bound Unix domain socket with permissions 0600");

            Ok(Self {
                socket_path: socket_path.to_path_buf(),
                listener,
                shutdown_rx,
                state,
            })
        }
        #[cfg(windows)]
        {
            let pipe_server = create_windows_pipe_server(true)?;
            info!(pipe = %windows_pipe_name()?, "Created Windows named-pipe server");
            Ok(Self {
                socket_path: socket_path.to_path_buf(),
                pipe_server,
                shutdown_rx,
                state,
            })
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(unix)]
        {
            loop {
                tokio::select! {
                    accept_res = self.listener.accept() => {
                        match accept_res {
                            Ok((stream, _)) => {
                                let state = self.state.clone();
                                tokio::spawn(handle_connection(stream, state));
                            }
                            Err(e) => {
                                warn!("Socket accept error: {e}");
                            }
                        }
                    }
                    _ = self.shutdown_rx.recv() => {
                        info!("Server received shutdown signal");
                        break;
                    }
                }
            }

            if self.socket_path.exists() {
                let _ = fs::remove_file(&self.socket_path);
            }

            Ok(())
        }
        #[cfg(windows)]
        {
            let mut pipe_server = self.pipe_server;
            loop {
                tokio::select! {
                    result = pipe_server.connect() => {
                        match result {
                            Ok(()) => {
                                let state = self.state.clone();
                                let connected = pipe_server;
                                tokio::spawn(handle_connection(connected, state));
                                pipe_server = create_windows_pipe_server(false)?;
                            }
                            Err(error) => {
                                warn!("Named-pipe connection failed: {error}");
                                pipe_server = create_windows_pipe_server(false)?;
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }
                        }
                    }
                    _ = self.shutdown_rx.recv() => {
                        info!("Server received shutdown signal");
                        break;
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
fn create_windows_pipe_server(first_instance: bool) -> std::io::Result<NamedPipeServer> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    use std::os::windows::ffi::OsStrExt;

    let sid = current_user_sid_string()?;
    let sddl = format!("D:P(A;;GA;;;{sid})(A;;GA;;;SY)");
    let sddl: Vec<u16> = std::ffi::OsStr::new(&sddl)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut descriptor = null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let mut options = ServerOptions::new();
    options.first_pipe_instance(first_instance);
    let result = unsafe {
        options.create_with_security_attributes_raw(
            windows_pipe_name()?,
            &mut attributes as *mut _ as _,
        )
    };
    unsafe { LocalFree(descriptor) };
    result
}

async fn handle_connection<S>(stream: S, state: Arc<AppState>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    // Client writer channel to serialize event writes
    let (tx_out, mut rx_out) = mpsc::channel::<DaemonMessage>(100);

    let writer_task = tokio::spawn(async move {
        while let Some(msg) = rx_out.recv().await {
            let mut out = serde_json::to_string(&msg).unwrap_or_default();
            out.push('\n');
            if writer.write_all(out.as_bytes()).await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
    });

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<ClientMessage>(trimmed) {
            Ok(msg) => {
                let state_clone = state.clone();
                let tx_out_clone = tx_out.clone();
                tokio::spawn(async move {
                    process_client_message(msg, state_clone, tx_out_clone).await;
                });
            }
            Err(e) => {
                error!("Invalid JSON from client: {e}");
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(id_val) = val.get("id") {
                        if let Ok(id) = serde_json::from_value::<Uuid>(id_val.clone()) {
                            let _ = tx_out
                                .send(DaemonMessage::Error {
                                    id,
                                    error: ErrorInfo {
                                        code: ErrorCode::InvalidRequest,
                                        message: format!(
                                            "Invalid request or unknown message type: {e}"
                                        ),
                                        retryable: false,
                                    },
                                })
                                .await;
                        }
                    }
                }
            }
        }
    }

    drop(tx_out);
    let _ = writer_task.await;
}

async fn process_client_message(
    msg: ClientMessage,
    state: Arc<AppState>,
    tx_out: mpsc::Sender<DaemonMessage>,
) {
    match msg {
        ClientMessage::Ping { id } => {
            let _ = tx_out.send(DaemonMessage::Pong { id }).await;
        }

        ClientMessage::GetSettings { id } => {
            let cfg = state.config.read().await;
            let has_key = keychain::get_api_key().is_some();
            let _ = tx_out
                .send(DaemonMessage::Settings {
                    id,
                    base_url: cfg.base_url.clone(),
                    model: cfg.model.clone(),
                    has_api_key: has_key,
                })
                .await;
        }

        ClientMessage::SetSettings {
            id,
            base_url,
            model,
            api_key,
        } => {
            if let Some(ref key) = api_key {
                let _ = keychain::set_api_key(key);
            }
            {
                let mut cfg = state.config.write().await;
                cfg.base_url = base_url.clone();
                cfg.model = model.clone();
                let _ = cfg.save_to_file(&state.config_path);
            }

            // Restart supervisor with updated settings without holding mutex across shutdown
            if !state.preserve_worker_on_settings_update {
                let old_sup_to_shutdown = {
                    let cfg = state.config.read().await;
                    let key = keychain::get_api_key();
                    match WorkerSupervisor::new(&cfg, key, None).await {
                        Ok(new_sup) => {
                            let mut sup_guard = state.supervisor.lock().await;
                            sup_guard.replace(new_sup)
                        }
                        Err(e) => {
                            error!(
                                "Failed to restart worker supervisor with updated settings: {e}"
                            );
                            None
                        }
                    }
                };
                if let Some(old_sup) = old_sup_to_shutdown {
                    old_sup.shutdown().await;
                }
            }

            let has_key = keychain::get_api_key().is_some();
            let _ = tx_out
                .send(DaemonMessage::Settings {
                    id,
                    base_url,
                    model,
                    has_api_key: has_key,
                })
                .await;
        }

        ClientMessage::ListChats { id } => {
            if let Some(ref pool) = state.pool {
                match db::list_chats(pool).await {
                    Ok(chats) => {
                        let _ = tx_out.send(DaemonMessage::Chats { id, chats }).await;
                    }
                    Err(e) => {
                        let _ = tx_out
                            .send(DaemonMessage::Error {
                                id,
                                error: ErrorInfo {
                                    code: ErrorCode::DatabaseUnavailable,
                                    message: e.to_string(),
                                    retryable: true,
                                },
                            })
                            .await;
                    }
                }
            } else {
                let _ = tx_out
                    .send(DaemonMessage::Error {
                        id,
                        error: ErrorInfo {
                            code: ErrorCode::DatabaseUnavailable,
                            message: "Database connection not available".to_string(),
                            retryable: true,
                        },
                    })
                    .await;
            }
        }

        ClientMessage::GetChat { id, chat_id } => {
            if let Some(ref pool) = state.pool {
                match db::get_chat(pool, chat_id).await {
                    Ok(Some((title, messages))) => {
                        let _ = tx_out
                            .send(DaemonMessage::Chat {
                                id,
                                chat_id,
                                title,
                                messages,
                            })
                            .await;
                    }
                    Ok(None) => {
                        let _ = tx_out
                            .send(DaemonMessage::Error {
                                id,
                                error: ErrorInfo {
                                    code: ErrorCode::InvalidRequest,
                                    message: "Chat not found".to_string(),
                                    retryable: false,
                                },
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx_out
                            .send(DaemonMessage::Error {
                                id,
                                error: ErrorInfo {
                                    code: ErrorCode::DatabaseUnavailable,
                                    message: e.to_string(),
                                    retryable: true,
                                },
                            })
                            .await;
                    }
                }
            } else {
                let _ = tx_out
                    .send(DaemonMessage::Error {
                        id,
                        error: ErrorInfo {
                            code: ErrorCode::DatabaseUnavailable,
                            message: "Database connection not available".to_string(),
                            retryable: true,
                        },
                    })
                    .await;
            }
        }

        ClientMessage::DeleteChat { id, chat_id } => {
            // 1. Tell worker to delete thread memory
            let sup_opt = { state.supervisor.lock().await.clone() };
            if let Some(sup) = sup_opt {
                let worker_req_id = Uuid::new_v4();
                if let Ok(mut rx) = sup
                    .send_request(DaemonWorkerRequest::DeleteChat {
                        request_id: worker_req_id,
                        chat_id,
                    })
                    .await
                {
                    let _ =
                        tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
                }
            }

            // 2. Delete database row
            if let Some(ref pool) = state.pool {
                if let Err(e) = db::delete_chat(pool, chat_id).await {
                    error!("Failed to delete chat {chat_id} from database: {e}");
                }
            }

            // 3. Remove attachments folder
            let att_dir = Config::default_attachments_dir().join(chat_id.to_string());
            if att_dir.exists() {
                let _ = fs::remove_dir_all(att_dir);
            }

            let _ = tx_out.send(DaemonMessage::Deleted { id, chat_id }).await;
        }

        ClientMessage::Cancel { id, target_id } => {
            {
                let mut cancelled = state.cancelled_requests.lock().await;
                cancelled.insert(target_id);
            }
            {
                let mut approvals = state.pending_approvals.lock().await;
                approvals.retain(|_, v| v.client_request_id != target_id);
            }

            let worker_req_id = {
                let open_reqs = state.open_requests.lock().await;
                open_reqs.get(&target_id).copied()
            };

            if let Some(w_id) = worker_req_id {
                let sup_opt = { state.supervisor.lock().await.clone() };
                if let Some(sup) = sup_opt {
                    let _ = sup
                        .send_request(DaemonWorkerRequest::Cancel { request_id: w_id })
                        .await;
                }
            }

            let _ = tx_out
                .send(DaemonMessage::Cancelled { id, target_id })
                .await;
        }

        ClientMessage::SendMessage {
            id,
            chat_id,
            text,
            attachments,
        } => {
            handle_send_message(id, chat_id, text, attachments, state, tx_out).await;
        }

        ClientMessage::ApprovalDecision {
            id,
            chat_id,
            call_id,
            action,
        } => {
            let decision_target = {
                let mut approvals = state.pending_approvals.lock().await;
                claim_pending_approval(&mut approvals, &call_id, chat_id)
            };

            match decision_target {
                Ok(worker_req_id) => {
                    let supervisor = state.supervisor.lock().await.clone();
                    let decision_result = match supervisor {
                        Some(supervisor) => supervisor
                            .send_request(DaemonWorkerRequest::ApprovalDecision {
                                request_id: worker_req_id,
                                call_id: call_id.clone(),
                                action,
                            })
                            .await
                            .map(|_| ())
                            .map_err(|error| {
                                format!("Failed to forward approval decision: {error}")
                            }),
                        None => Err("Worker supervisor is not available".to_string()),
                    };

                    match decision_result {
                        Ok(()) => {
                            state.pending_approvals.lock().await.remove(&call_id);
                        }
                        Err(message) => {
                            if let Some(entry) =
                                state.pending_approvals.lock().await.get_mut(&call_id)
                            {
                                entry.decision_in_flight = false;
                            }
                            let _ = tx_out
                                .send(DaemonMessage::Error {
                                    id,
                                    error: ErrorInfo {
                                        code: ErrorCode::InvalidRequest,
                                        message,
                                        retryable: true,
                                    },
                                })
                                .await;
                        }
                    }
                }
                Err(message) => {
                    let _ = tx_out
                        .send(DaemonMessage::Error {
                            id,
                            error: ErrorInfo {
                                code: ErrorCode::InvalidRequest,
                                message: message.to_string(),
                                retryable: false,
                            },
                        })
                        .await;
                }
            }
        }
    }
}

async fn handle_send_message(
    id: Uuid,
    chat_id: Uuid,
    text: String,
    attachments: Vec<Attachment>,
    state: Arc<AppState>,
    tx_out: mpsc::Sender<DaemonMessage>,
) {
    let worker_req_id = Uuid::new_v4();
    {
        let mut open_reqs = state.open_requests.lock().await;
        open_reqs.insert(id, worker_req_id);
    }

    // Helper macro/closure to clean up on early exit
    let cleanup = |st: &Arc<AppState>| {
        let st_clone = st.clone();
        async move {
            let mut open_reqs = st_clone.open_requests.lock().await;
            open_reqs.remove(&id);
            let mut cancelled = st_clone.cancelled_requests.lock().await;
            cancelled.remove(&id);
            let mut approvals = st_clone.pending_approvals.lock().await;
            approvals.retain(|_, v| v.client_request_id != id);
        }
    };

    // 1. Validate
    if text.trim().is_empty() && attachments.is_empty() {
        cleanup(&state).await;
        let _ = tx_out
            .send(DaemonMessage::Error {
                id,
                error: ErrorInfo {
                    code: ErrorCode::InvalidRequest,
                    message: "Message text and attachments cannot both be empty".to_string(),
                    retryable: false,
                },
            })
            .await;
        return;
    }

    if attachments.len() > 5 {
        cleanup(&state).await;
        let _ = tx_out
            .send(DaemonMessage::Error {
                id,
                error: ErrorInfo {
                    code: ErrorCode::AttachmentInvalid,
                    message: "Attachments count exceeds maximum limit of 5".to_string(),
                    retryable: false,
                },
            })
            .await;
        return;
    }

    for att in &attachments {
        if let Err(msg) = validate_attachment(att) {
            cleanup(&state).await;
            let _ = tx_out
                .send(DaemonMessage::Error {
                    id,
                    error: ErrorInfo {
                        code: ErrorCode::AttachmentInvalid,
                        message: msg,
                        retryable: false,
                    },
                })
                .await;
            return;
        }
    }

    let pool = match state.pool {
        Some(ref p) => p.clone(),
        None => {
            cleanup(&state).await;
            let _ = tx_out
                .send(DaemonMessage::Error {
                    id,
                    error: ErrorInfo {
                        code: ErrorCode::DatabaseUnavailable,
                        message: "Database connection not available".to_string(),
                        retryable: true,
                    },
                })
                .await;
            return;
        }
    };

    let user_msg_id = Uuid::new_v4();
    if let Err(e) = db::ensure_chat(&pool, chat_id, "New Chat").await {
        cleanup(&state).await;
        let _ = tx_out
            .send(DaemonMessage::Error {
                id,
                error: ErrorInfo {
                    code: ErrorCode::DatabaseUnavailable,
                    message: e.to_string(),
                    retryable: true,
                },
            })
            .await;
        return;
    }

    if let Err(e) = db::save_user_message(&pool, user_msg_id, chat_id, &text, &attachments).await {
        cleanup(&state).await;
        let _ = tx_out
            .send(DaemonMessage::Error {
                id,
                error: ErrorInfo {
                    code: ErrorCode::DatabaseUnavailable,
                    message: e.to_string(),
                    retryable: true,
                },
            })
            .await;
        return;
    }

    let supervisor = match state.supervisor.lock().await.clone() {
        Some(s) => s,
        None => {
            cleanup(&state).await;
            let _ = tx_out
                .send(DaemonMessage::Error {
                    id,
                    error: ErrorInfo {
                        code: ErrorCode::WorkerCrashed,
                        message: "Worker supervisor not running".to_string(),
                        retryable: true,
                    },
                })
                .await;
            return;
        }
    };

    // Check if cancel was already requested
    let was_cancelled = {
        let cancelled = state.cancelled_requests.lock().await;
        cancelled.contains(&id)
    };

    if was_cancelled {
        cleanup(&state).await;
        let asst_message_id = Uuid::new_v4();
        let _ = db::save_assistant_message(
            &pool,
            asst_message_id,
            chat_id,
            "",
            MessageStatus::Cancelled,
            None,
        )
        .await;
        let _ = tx_out
            .send(DaemonMessage::MessageFinished {
                id,
                chat_id,
                message_id: asst_message_id,
                status: FinishStatus::Cancelled,
            })
            .await;
        return;
    }

    let mut event_rx = match supervisor
        .send_request(DaemonWorkerRequest::Run {
            request_id: worker_req_id,
            chat_id,
            text: text.clone(),
            attachments,
        })
        .await
    {
        Ok(rx) => rx,
        Err(e) => {
            cleanup(&state).await;
            let _ = tx_out
                .send(DaemonMessage::Error {
                    id,
                    error: ErrorInfo {
                        code: ErrorCode::WorkerCrashed,
                        message: e.to_string(),
                        retryable: true,
                    },
                })
                .await;
            return;
        }
    };

    let mut full_text = String::new();
    let mut asst_message_id = Uuid::new_v4();

    while let Some(ev) = event_rx.recv().await {
        match ev {
            DaemonWorkerEvent::MessageStarted { message_id, .. } => {
                asst_message_id = message_id;
                let _ = tx_out
                    .send(DaemonMessage::MessageStarted {
                        id,
                        chat_id,
                        message_id,
                    })
                    .await;
            }

            DaemonWorkerEvent::TextDelta { text: delta, .. } => {
                full_text.push_str(&delta);
                let _ = tx_out
                    .send(DaemonMessage::TextDelta {
                        id,
                        chat_id,
                        message_id: asst_message_id,
                        text: delta,
                    })
                    .await;
            }

            DaemonWorkerEvent::ToolCallStarted {
                call_id,
                name,
                arguments,
                ..
            } => {
                let _ = tx_out
                    .send(DaemonMessage::ToolCallStarted {
                        id,
                        chat_id,
                        message_id: asst_message_id,
                        call_id,
                        name,
                        arguments,
                    })
                    .await;
            }

            DaemonWorkerEvent::ToolCallFinished {
                call_id, result, ..
            } => {
                let _ = tx_out
                    .send(DaemonMessage::ToolCallFinished {
                        id,
                        chat_id,
                        message_id: asst_message_id,
                        call_id,
                        result,
                    })
                    .await;
            }

            DaemonWorkerEvent::ApprovalRequested {
                call_id,
                tool_name,
                arguments,
                explanation,
                resource_summary,
                actions,
                ..
            } => {
                {
                    let mut approvals = state.pending_approvals.lock().await;
                    approvals.insert(
                        call_id.clone(),
                        PendingApprovalEntry {
                            chat_id,
                            client_request_id: id,
                            worker_request_id: worker_req_id,
                            decision_in_flight: false,
                        },
                    );
                }
                let _ = tx_out
                    .send(DaemonMessage::ApprovalRequested {
                        id,
                        chat_id,
                        message_id: asst_message_id,
                        call_id,
                        tool_name,
                        arguments,
                        explanation,
                        resource_summary,
                        actions,
                    })
                    .await;
            }

            DaemonWorkerEvent::MessageFinished { status, .. } => {
                let msg_status = match status {
                    FinishStatus::Complete => MessageStatus::Complete,
                    FinishStatus::Cancelled => MessageStatus::Cancelled,
                    FinishStatus::Error => MessageStatus::Error,
                };
                let _ = db::save_assistant_message(
                    &pool,
                    asst_message_id,
                    chat_id,
                    &full_text,
                    msg_status,
                    None,
                )
                .await;

                let _ = tx_out
                    .send(DaemonMessage::MessageFinished {
                        id,
                        chat_id,
                        message_id: asst_message_id,
                        status,
                    })
                    .await;

                // Dispatch title request only after successful completion if chat is untitled
                if status == FinishStatus::Complete {
                    let chat_row = db::get_chat(&pool, chat_id).await.ok().flatten();
                    if let Some((current_title, _)) = chat_row {
                        if current_title == "New Chat" || current_title.is_empty() {
                            let pool_clone = pool.clone();
                            let sup_clone = supervisor.clone();
                            let tx_out_clone = tx_out.clone();
                            let first_msg = text.clone();
                            tokio::spawn(async move {
                                request_title(
                                    chat_id,
                                    first_msg,
                                    sup_clone,
                                    pool_clone,
                                    tx_out_clone,
                                )
                                .await;
                            });
                        }
                    }
                }
                break;
            }

            DaemonWorkerEvent::Error { error, .. } => {
                let _ = db::save_assistant_message(
                    &pool,
                    asst_message_id,
                    chat_id,
                    &full_text,
                    MessageStatus::Error,
                    Some(&error),
                )
                .await;

                let _ = tx_out.send(DaemonMessage::Error { id, error }).await;
                let _ = tx_out
                    .send(DaemonMessage::MessageFinished {
                        id,
                        chat_id,
                        message_id: asst_message_id,
                        status: FinishStatus::Error,
                    })
                    .await;
                break;
            }

            _ => {}
        }
    }

    {
        let mut open_reqs = state.open_requests.lock().await;
        open_reqs.remove(&id);
        let mut cancelled = state.cancelled_requests.lock().await;
        cancelled.remove(&id);
    }
}

async fn request_title(
    chat_id: Uuid,
    first_message: String,
    supervisor: WorkerSupervisor,
    pool: PgPool,
    tx_out: mpsc::Sender<DaemonMessage>,
) {
    let title_req_id = Uuid::new_v4();
    if let Ok(mut rx) = supervisor
        .send_request(DaemonWorkerRequest::Title {
            request_id: title_req_id,
            chat_id,
            first_message: first_message.clone(),
        })
        .await
    {
        if let Some(DaemonWorkerEvent::TitleReady { title, .. }) = rx.recv().await {
            let _ = db::update_chat_title(&pool, chat_id, &title).await;
            let _ = tx_out
                .send(DaemonMessage::ChatTitled { chat_id, title })
                .await;
            return;
        }
    }

    // Fallback: safely take up to 40 characters
    let fallback = if first_message.chars().count() > 40 {
        let truncated: String = first_message.chars().take(37).collect();
        format!("{truncated}...")
    } else {
        first_message
    };
    let _ = db::update_chat_title(&pool, chat_id, &fallback).await;
    let _ = tx_out
        .send(DaemonMessage::ChatTitled {
            chat_id,
            title: fallback,
        })
        .await;
}

pub fn validate_attachment(att: &Attachment) -> Result<(), String> {
    const MAX_SIZE_BYTES: i64 = 10 * 1024 * 1024; // 10_485_760 bytes

    if att.size_bytes < 0 || att.size_bytes > MAX_SIZE_BYTES {
        return Err(format!(
            "Attachment '{}' exceeds maximum allowed size of 10 MB ({} bytes)",
            att.name, att.size_bytes
        ));
    }

    match att.kind {
        crate::protocol::AttachmentKind::Image => match att.mime.to_ascii_lowercase().as_str() {
            "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif" => Ok(()),
            _ => Err(format!(
                "Invalid MIME type '{}' for image attachment '{}' (allowed: PNG, JPEG, WebP, GIF)",
                att.mime, att.name
            )),
        },
        crate::protocol::AttachmentKind::Pdf => match att.mime.to_ascii_lowercase().as_str() {
            "application/pdf" => Ok(()),
            _ => Err(format!(
                "Invalid MIME type '{}' for PDF attachment '{}' (allowed: application/pdf)",
                att.mime, att.name
            )),
        },
        crate::protocol::AttachmentKind::Text => {
            let m = att.mime.to_ascii_lowercase();
            if m.starts_with("text/")
                || m == "application/json"
                || m == "text/csv"
                || m == "text/markdown"
                || m == "text/plain"
                || m == "application/javascript"
                || m == "application/typescript"
                || m == "application/xml"
                || m == "application/x-yaml"
                || m == "text/yaml"
            {
                Ok(())
            } else {
                Err(format!(
                    "Invalid MIME type '{}' for text attachment '{}' (allowed: text/*, JSON, CSV, Markdown, code)",
                    att.mime, att.name
                ))
            }
        }
    }
}
