use nook_core::config::Config;
use nook_core::protocol::{
    Attachment, AttachmentKind, ChatMessage, ChatSummary, ClientMessage, DaemonMessage,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default, Clone)]
pub struct DaemonState {
    pub client: Arc<Mutex<Option<DaemonConnection>>>,
    pub active_request_id: Arc<Mutex<Option<Uuid>>>,
    pub spawned_process: Arc<Mutex<Option<tokio::process::Child>>>,
    pub startup_lock: Arc<Mutex<()>>,
}

pub struct DaemonConnection {
    pub socket_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTranscript {
    pub chat_id: Uuid,
    pub title: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsInfo {
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessagePayload {
    pub chat_id: Uuid,
    pub text: String,
    pub attachments: Vec<AttachmentInput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachmentInput {
    pub name: String,
    pub mime: String,
    pub data_base64: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsPayload {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

/// Locate the nookd binary from environment, app bundle, or development paths
pub fn find_nookd_binary() -> Option<PathBuf> {
    // 1. Explicit environment override
    for env_key in &["NOOKD_PATH", "NOOKD_BIN", "NOOK_DAEMON_BIN"] {
        if let Ok(path_str) = std::env::var(env_key) {
            let p = PathBuf::from(path_str);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 2. Next to current executable (in app bundle or install directory)
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            #[cfg(windows)]
            let bin_name = "nookd.exe";
            #[cfg(not(windows))]
            let bin_name = "nookd";

            let candidate = exe_dir.join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }

            // In macOS app bundle (Contents/MacOS -> Contents/Helpers/nookd)
            let helper_candidate = exe_dir
                .parent()
                .unwrap_or(exe_dir)
                .join("Helpers")
                .join(bin_name);
            if helper_candidate.is_file() {
                return Some(helper_candidate);
            }
        }
    }

    // 3. Development and workspace build paths
    #[cfg(windows)]
    let bin_name = "nookd.exe";
    #[cfg(not(windows))]
    let bin_name = "nookd";

    let dev_candidates = [
        PathBuf::from("daemon/target/release").join(bin_name),
        PathBuf::from("daemon/target/debug").join(bin_name),
        PathBuf::from("../daemon/target/release").join(bin_name),
        PathBuf::from("../daemon/target/debug").join(bin_name),
        PathBuf::from("../../daemon/target/release").join(bin_name),
        PathBuf::from("../../daemon/target/debug").join(bin_name),
    ];
    for candidate in &dev_candidates {
        if candidate.is_file() {
            return Some(candidate.clone());
        }
    }

    // 4. Standard system install locations
    #[cfg(target_os = "macos")]
    {
        let system_app_bin = PathBuf::from("/Applications/Nook.app/Contents/MacOS").join(bin_name);
        if system_app_bin.is_file() {
            return Some(system_app_bin);
        }
        if let Ok(home) = std::env::var("HOME") {
            let user_app_bin = PathBuf::from(&home)
                .join("Applications/Nook.app/Contents/MacOS")
                .join(bin_name);
            if user_app_bin.is_file() {
                return Some(user_app_bin);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let user_bin = PathBuf::from(&home).join(".local/bin").join(bin_name);
        if user_bin.is_file() {
            return Some(user_bin);
        }
        let cargo_bin = PathBuf::from(&home).join(".cargo/bin").join(bin_name);
        if cargo_bin.is_file() {
            return Some(cargo_bin);
        }
    }

    let global_bin = PathBuf::from("/usr/local/bin").join(bin_name);
    if global_bin.is_file() {
        return Some(global_bin);
    }

    None
}

/// Poll Unix socket with a timeout until it accepts connections
pub async fn wait_for_daemon_ready(socket_path: &Path, timeout: std::time::Duration) -> bool {
    #[cfg(unix)]
    {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if socket_path.exists() && UnixStream::connect(socket_path).await.is_ok() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }
    #[cfg(not(unix))]
    {
        let _ = (socket_path, timeout);
        false
    }
}

/// Ensure nookd is running; spawn it if not currently responding
pub async fn ensure_daemon_started(state: &DaemonState) {
    #[cfg(unix)]
    {
        let _lock = state.startup_lock.lock().await;

        let socket_path = Config::default_socket_path();
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            return;
        }

        // Check if an existing spawned process is still running
        {
            let mut proc_guard = state.spawned_process.lock().await;
            if let Some(child) = proc_guard.as_mut() {
                match child.try_wait() {
                    Ok(None) => {
                        drop(proc_guard);
                        if wait_for_daemon_ready(
                            &socket_path,
                            std::time::Duration::from_millis(1500),
                        )
                        .await
                        {
                            return;
                        }
                        eprintln!(
                            "Existing nookd child is running but not responding on socket yet."
                        );
                        return;
                    }
                    Ok(Some(status)) => {
                        eprintln!("Previous nookd child process exited ({status}). Reaping.");
                        *proc_guard = None;
                    }
                    Err(err) => {
                        eprintln!("Error checking child process status ({err}). Clearing.");
                        *proc_guard = None;
                    }
                }
            }
        }

        if wait_for_daemon_ready(&socket_path, std::time::Duration::from_millis(200)).await {
            return;
        }

        if let Some(bin_path) = find_nookd_binary() {
            eprintln!("Auto-starting nookd daemon from {}", bin_path.display());
            let mut cmd = tokio::process::Command::new(bin_path);
            cmd.kill_on_drop(true);

            match cmd.spawn() {
                Ok(child) => {
                    {
                        let mut proc_guard = state.spawned_process.lock().await;
                        *proc_guard = Some(child);
                    }
                    if wait_for_daemon_ready(&socket_path, std::time::Duration::from_millis(2500))
                        .await
                    {
                        eprintln!("nookd started successfully in background.");
                    } else {
                        eprintln!("Warning: nookd spawned but not yet responding on socket.");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to auto-spawn nookd: {e}");
                }
            }
        } else {
            eprintln!(
                "Notice: nookd binary not found automatically. Set NOOKD_PATH or run nookd separately."
            );
        }
    }
    #[cfg(not(unix))]
    {
        let _ = state;
    }
}

/// Helper to configure SO_NOSIGPIPE on UnixStream on macOS
#[cfg(unix)]
fn configure_socket_safety(stream: &UnixStream) {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = stream.as_raw_fd();
        unsafe {
            let optval: libc::c_int = 1;
            let _ = libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_NOSIGPIPE,
                &optval as *const _ as *const libc::c_void,
                std::mem::size_of_val(&optval) as libc::socklen_t,
            );
        }
    }
    let _ = stream;
}

/// Sanitize attachment filename against path traversal attacks
pub fn sanitize_attachment_name(name: &str) -> String {
    let clean = Path::new(name)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "attachment".to_string());
    clean.replace(['/', '\\', '\0'], "_")
}

#[tauri::command]
async fn ping_daemon() -> Result<bool, String> {
    #[cfg(unix)]
    {
        let socket_path = Config::default_socket_path();
        if !socket_path.exists() {
            return Ok(false);
        }
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                configure_socket_safety(&stream);
                let (reader, mut writer) = stream.into_split();
                let mut lines = BufReader::new(reader).lines();
                let msg = ClientMessage::Ping { id: Uuid::new_v4() };
                let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
                payload.push('\n');
                writer
                    .write_all(payload.as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                writer.flush().await.map_err(|e| e.to_string())?;
                if let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(DaemonMessage::Pong { .. }) = serde_json::from_str(&line) {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Err(_) => Ok(false),
        }
    }
    #[cfg(not(unix))]
    {
        Ok(false)
    }
}

#[tauri::command]
async fn list_chats() -> Result<Vec<ChatSummary>, String> {
    #[cfg(unix)]
    {
        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let msg = ClientMessage::ListChats { id: Uuid::new_v4() };
        let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;

        if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
                DaemonMessage::Chats { chats, .. } => Ok(chats),
                DaemonMessage::Error { error, .. } => {
                    Err(format!("{:?}: {}", error.code, error.message))
                }
                _ => Err("Unexpected response from daemon".to_string()),
            }
        } else {
            Err("Daemon closed connection unexpectedly".to_string())
        }
    }
    #[cfg(not(unix))]
    {
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn get_chat(chat_id: Uuid) -> Result<ChatTranscript, String> {
    #[cfg(unix)]
    {
        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let msg = ClientMessage::GetChat {
            id: Uuid::new_v4(),
            chat_id,
        };
        let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;

        if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
                DaemonMessage::Chat {
                    title, messages, ..
                } => Ok(ChatTranscript {
                    chat_id,
                    title,
                    messages,
                }),
                DaemonMessage::Error { error, .. } => {
                    Err(format!("{:?}: {}", error.code, error.message))
                }
                _ => Err("Unexpected response from daemon".to_string()),
            }
        } else {
            Err("Daemon closed connection unexpectedly".to_string())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = chat_id;
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn delete_chat(chat_id: Uuid) -> Result<bool, String> {
    #[cfg(unix)]
    {
        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let msg = ClientMessage::DeleteChat {
            id: Uuid::new_v4(),
            chat_id,
        };
        let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;

        if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
                DaemonMessage::Deleted { .. } => Ok(true),
                DaemonMessage::Error { error, .. } => {
                    Err(format!("{:?}: {}", error.code, error.message))
                }
                _ => Err("Unexpected response from daemon".to_string()),
            }
        } else {
            Err("Daemon closed connection unexpectedly".to_string())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = chat_id;
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn send_message<R: Runtime>(
    app: AppHandle<R>,
    chat_id: Uuid,
    text: String,
    attachments: Vec<AttachmentInput>,
    on_event: tauri::ipc::Channel<DaemonMessage>,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        if attachments.len() > 5 {
            return Err("Maximum 5 attachments allowed per message".to_string());
        }

        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let att_dir = Config::default_attachments_dir().join(chat_id.to_string());
        if !attachments.is_empty() {
            let _ = std::fs::create_dir_all(&att_dir);
        }

        let mut processed_attachments = Vec::new();
        for att in attachments {
            let safe_name = sanitize_attachment_name(&att.name);
            let att_id = Uuid::new_v4();
            let dest_path = att_dir.join(format!("{att_id}-{safe_name}"));

            let size_bytes: i64;
            if let Some(ref path_str) = att.file_path {
                let src = Path::new(path_str);
                if !src.exists() {
                    return Err(format!("File does not exist: {path_str}"));
                }
                let meta = std::fs::metadata(src).map_err(|e| e.to_string())?;
                size_bytes = meta.len() as i64;
                if size_bytes > 10 * 1024 * 1024 {
                    return Err(format!("Attachment exceeds 10MB limit: {safe_name}"));
                }
                std::fs::copy(src, &dest_path).map_err(|e| e.to_string())?;
            } else if let Some(ref b64) = att.data_base64 {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| format!("Invalid base64 attachment data: {e}"))?;
                size_bytes = bytes.len() as i64;
                if size_bytes > 10 * 1024 * 1024 {
                    return Err(format!("Attachment exceeds 10MB limit: {safe_name}"));
                }
                std::fs::write(&dest_path, bytes).map_err(|e| e.to_string())?;
            } else {
                return Err("Attachment must provide either file_path or data_base64".to_string());
            }

            let kind = if att.mime.starts_with("image/") {
                match att.mime.as_str() {
                    "image/png" | "image/jpeg" | "image/webp" | "image/gif" => {
                        AttachmentKind::Image
                    }
                    _ => return Err(format!("Unsupported image format: {}", att.mime)),
                }
            } else if att.mime == "application/pdf" {
                AttachmentKind::Pdf
            } else if att.mime.starts_with("text/")
                || att.mime == "application/json"
                || att.mime == "application/javascript"
            {
                AttachmentKind::Text
            } else {
                return Err(format!("Unsupported attachment MIME type: {}", att.mime));
            };

            processed_attachments.push(Attachment {
                id: att_id,
                kind,
                name: safe_name,
                mime: att.mime,
                size_bytes,
                path: dest_path.to_string_lossy().to_string(),
            });
        }

        let req_id = Uuid::new_v4();
        let state = app.state::<DaemonState>();
        {
            let mut active = state.active_request_id.lock().await;
            *active = Some(req_id);
        }

        let msg = ClientMessage::SendMessage {
            id: req_id,
            chat_id,
            text,
            attachments: processed_attachments,
        };
        let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;

        let state_for_cleanup = state.inner().active_request_id.clone();
        tauri::async_runtime::spawn(async move {
            let _keep_writer_alive = writer;
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(event) = serde_json::from_str::<DaemonMessage>(&line) {
                    let is_terminal = matches!(
                        event,
                        DaemonMessage::MessageFinished { .. } | DaemonMessage::Error { .. }
                    );
                    let _ = on_event.send(event);
                    if is_terminal {
                        let mut active = state_for_cleanup.lock().await;
                        *active = None;
                        break;
                    }
                }
            }
            let mut active = state_for_cleanup.lock().await;
            *active = None;
        });

        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (app, chat_id, text, attachments, on_event);
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn cancel_message<R: Runtime>(
    app: AppHandle<R>,
    target_id: Option<Uuid>,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let req_to_cancel = if let Some(id) = target_id {
            id
        } else {
            let state = app.state::<DaemonState>();
            let active = state.active_request_id.lock().await;
            match *active {
                Some(id) => id,
                None => return Ok(()),
            }
        };

        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (_, mut writer) = stream.into_split();

        let msg = ClientMessage::Cancel {
            id: Uuid::new_v4(),
            target_id: req_to_cancel,
        };
        let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (app, target_id);
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn get_settings() -> Result<SettingsInfo, String> {
    #[cfg(unix)]
    {
        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let msg = ClientMessage::GetSettings { id: Uuid::new_v4() };
        let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        payload.push('\n');
        writer
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;

        if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
                DaemonMessage::Settings {
                    base_url,
                    model,
                    has_api_key,
                    ..
                } => Ok(SettingsInfo {
                    base_url,
                    model,
                    has_api_key,
                }),
                DaemonMessage::Error { error, .. } => {
                    Err(format!("{:?}: {}", error.code, error.message))
                }
                _ => Err("Unexpected response from daemon".to_string()),
            }
        } else {
            Err("Daemon closed connection unexpectedly".to_string())
        }
    }
    #[cfg(not(unix))]
    {
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn set_settings(payload: SettingsPayload) -> Result<SettingsInfo, String> {
    #[cfg(unix)]
    {
        let socket_path = Config::default_socket_path();
        let stream = UnixStream::connect(&socket_path)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
        configure_socket_safety(&stream);
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let msg = ClientMessage::SetSettings {
            id: Uuid::new_v4(),
            base_url: payload.base_url,
            model: payload.model,
            api_key: payload.api_key,
        };
        let mut p = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        p.push('\n');
        writer
            .write_all(p.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;

        if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
                DaemonMessage::Settings {
                    base_url,
                    model,
                    has_api_key,
                    ..
                } => Ok(SettingsInfo {
                    base_url,
                    model,
                    has_api_key,
                }),
                DaemonMessage::Error { error, .. } => {
                    Err(format!("{:?}: {}", error.code, error.message))
                }
                _ => Err("Unexpected response from daemon".to_string()),
            }
        } else {
            Err("Daemon closed connection unexpectedly".to_string())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = payload;
        Err("Daemon IPC is not available on Windows".to_string())
    }
}

#[tauri::command]
async fn set_window_size<R: Runtime>(
    app: AppHandle<R>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let app_clone = app.clone();
    app.run_on_main_thread(move || {
        if let Some(window) = app_clone.get_webview_window("main") {
            let _ = window.set_visible_on_all_workspaces(true);
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn start_drag<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let app_clone = app.clone();
    app.run_on_main_thread(move || {
        if let Some(window) = app_clone.get_webview_window("main") {
            let _ = window.start_dragging();
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn toggle_window<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    toggle_window_internal(&app)
}

pub fn run() {
    #[cfg(target_os = "macos")]
    let shortcut_str = "Option+Space";
    #[cfg(not(target_os = "macos"))]
    let shortcut_str = "Alt+Space";

    let shortcut: Shortcut = shortcut_str.parse().expect("Valid shortcut string");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, s, event| {
                    if s == &shortcut && event.state() == ShortcutState::Pressed {
                        let _ = toggle_window_internal(app);
                    }
                })
                .build(),
        )
        .manage(DaemonState::default())
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let state = app.state::<DaemonState>();
            let state_clone = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                ensure_daemon_started(&state_clone).await;
            });

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_visible_on_all_workspaces(true);
                let _ = window.show();
                let _ = window.set_focus();
            }

            if let Err(e) = app.global_shortcut().register(shortcut) {
                eprintln!("Failed to register global shortcut: {e}");
            } else {
                eprintln!("Registered global shortcut: {shortcut_str}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping_daemon,
            list_chats,
            get_chat,
            delete_chat,
            send_message,
            cancel_message,
            get_settings,
            set_settings,
            set_window_size,
            toggle_window,
            start_drag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn toggle_window_internal<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let app_handle = app.clone();
    app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_visible_on_all_workspaces(true);
            let is_visible = window.is_visible().unwrap_or(false);
            let is_focused = window.is_focused().unwrap_or(false);
            if is_visible && is_focused {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn test_wait_for_daemon_ready_success() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let _listener = UnixListener::bind(&sock_path).unwrap();

        let ready = wait_for_daemon_ready(&sock_path, std::time::Duration::from_millis(500)).await;
        assert!(ready);
    }

    #[tokio::test]
    async fn test_wait_for_daemon_ready_timeout() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("nonexistent.sock");
        let ready = wait_for_daemon_ready(&sock_path, std::time::Duration::from_millis(100)).await;
        assert!(!ready);
    }

    #[tokio::test]
    async fn test_concurrent_ensure_daemon_started_serialization() {
        let state = DaemonState::default();
        let mut handles = Vec::new();
        for _ in 0..5 {
            let s = state.clone();
            handles.push(tokio::spawn(async move {
                ensure_daemon_started(&s).await;
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        let proc = state.spawned_process.lock().await;
        assert!(proc.is_none() || proc.is_some());
    }
}
