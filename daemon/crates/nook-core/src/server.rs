use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
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

use crate::authorization::AuthorizationRequest;
use crate::config::Config;
use crate::db;
use crate::keychain;
use crate::message_stream::handle_send_message;
pub use crate::message_stream::{
    forward_text_delta, forward_tool_call_event, forward_unless_cancelled,
};
use crate::policy_assets::PolicyAssets;
use crate::protocol::{
    Attachment, ClientMessage, DaemonMessage, DaemonWorkerRequest, ErrorCode, ErrorInfo,
};
use crate::providers::ProviderRegistry;
use crate::supervisor::WorkerSupervisor;
use crate::tool_approval::{
    abort_running_tools, deny_pending_for_request, handle_approval_decision,
};

#[derive(Debug, Clone)]
pub struct PendingApprovalEntry {
    pub chat_id: Uuid,
    pub client_request_id: Uuid,
    pub worker_request_id: Uuid,
    pub decision_in_flight: bool,
    pub request: AuthorizationRequest,
    pub offered_actions: Vec<String>,
    pub execution: Option<crate::tool_approval::Execution>,
}

pub fn claim_pending_approval(
    approvals: &mut HashMap<String, PendingApprovalEntry>,
    call_id: &str,
    chat_id: Uuid,
    action: &str,
) -> Result<PendingApprovalEntry, &'static str> {
    match approvals.get_mut(call_id) {
        Some(entry) if entry.chat_id != chat_id => {
            Err("Approval call ID does not belong to the specified chat")
        }
        Some(entry) if entry.decision_in_flight => {
            Err("Approval decision is already being forwarded")
        }
        Some(entry)
            if !entry
                .offered_actions
                .iter()
                .any(|offered| offered == action) =>
        {
            Err("Approval action was not offered for this call")
        }
        Some(entry) => {
            entry.decision_in_flight = true;
            Ok(entry.clone())
        }
        None => Err("Unknown or invalid approval call id"),
    }
}

/// The database pool. It is empty when the Daemon starts without a database
/// and holds the pool once a later connection attempt works.
#[derive(Default)]
pub struct DbPool(std::sync::RwLock<Option<PgPool>>);

impl DbPool {
    pub fn new(pool: Option<PgPool>) -> Self {
        Self(std::sync::RwLock::new(pool))
    }

    pub fn get(&self) -> Option<PgPool> {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set(&self, pool: PgPool) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = Some(pool);
    }
}

pub struct AppState {
    pub config: RwLock<Config>,
    pub config_path: PathBuf,
    pub pool: DbPool,
    pub supervisor: Mutex<Option<WorkerSupervisor>>,
    pub preserve_worker_on_settings_update: bool,
    pub open_requests: Mutex<HashMap<Uuid, Uuid>>, // maps client id -> worker request_id
    pub pending_approvals: Mutex<HashMap<String, PendingApprovalEntry>>, // maps call_id -> entry
    pub cancelled_requests: Mutex<HashSet<Uuid>>,
    pub approval_timeout: Duration,
    pub providers: Arc<ProviderRegistry>,
    pub running_tools: Mutex<HashMap<Uuid, Vec<(String, tokio::task::AbortHandle)>>>,
    pub policy: Arc<PolicyAssets>,
}

pub struct Server {
    socket_path: PathBuf,
    #[cfg(unix)]
    socket_inode: u64,
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
                if std::os::unix::net::UnixStream::connect(socket_path).is_ok() {
                    return Err(format!(
                        "Another daemon is already listening on {}",
                        socket_path.display()
                    )
                    .into());
                }
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
                socket_inode: std::os::unix::fs::MetadataExt::ino(&fs::metadata(socket_path)?),
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

            let still_ours = fs::metadata(&self.socket_path)
                .is_ok_and(|meta| std::os::unix::fs::MetadataExt::ino(&meta) == self.socket_inode);
            if still_ours {
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
                if let Err(error) =
                    Config::update_model_settings(&state.config_path, &base_url, &model)
                {
                    warn!("Could not save the settings to config.toml: {error}");
                }
            }

            // Restart supervisor with updated settings without holding mutex across shutdown
            if !state.preserve_worker_on_settings_update {
                let old_sup_to_shutdown = {
                    let cfg = state.config.read().await;
                    let key = keychain::get_api_key();
                    match WorkerSupervisor::with_tools(
                        &cfg,
                        key,
                        None,
                        state.providers.descriptors().to_vec(),
                    )
                    .await
                    {
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
            if let Some(pool) = state.pool.get() {
                match db::list_chats(&pool).await {
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
            if let Some(pool) = state.pool.get() {
                match db::get_chat(&pool, chat_id).await {
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
            state
                .policy
                .authorizer
                .grant_store()
                .lock()
                .await
                .remove_chat(chat_id);

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
            if let Some(pool) = state.pool.get() {
                if let Err(e) = db::delete_chat(&pool, chat_id).await {
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
            deny_pending_for_request(&state, target_id, "Cancelled").await;
            abort_running_tools(&state, target_id).await;

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

        ClientMessage::ListPermissions { id } => {
            send_permissions(&state, id, &tx_out).await;
        }

        ClientMessage::RevokeGrant { id, grant_id } => {
            match crate::permissions::revoke_grant(&state, &grant_id).await {
                Ok(()) => send_permissions(&state, id, &tx_out).await,
                Err(message) => send_invalid_request(id, message, &tx_out).await,
            }
        }

        ClientMessage::SetToolPreference { id, tool_id, tier } => {
            match crate::permissions::set_preference(&state, &tool_id, &tier).await {
                Ok(()) => send_permissions(&state, id, &tx_out).await,
                Err(message) => send_invalid_request(id, message, &tx_out).await,
            }
        }

        ClientMessage::ApprovalDecision {
            id,
            chat_id,
            call_id,
            action,
        } => {
            handle_approval_decision(&state, id, chat_id, call_id, action, &tx_out).await;
        }
    }
}

async fn send_permissions(state: &AppState, id: Uuid, tx_out: &mpsc::Sender<DaemonMessage>) {
    let (grants, tools) = crate::permissions::snapshot(state).await;
    let _ = tx_out
        .send(DaemonMessage::Permissions { id, grants, tools })
        .await;
}

async fn send_invalid_request(id: Uuid, message: &str, tx_out: &mpsc::Sender<DaemonMessage>) {
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
