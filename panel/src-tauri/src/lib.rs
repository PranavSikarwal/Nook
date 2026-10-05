use std::path::{Path, PathBuf};
use std::sync::Arc;
use nook_core::config::Config;
use nook_core::protocol::{
    Attachment, AttachmentKind, ChatMessage, ChatSummary, ClientMessage, DaemonMessage,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct DaemonState {
    pub client: Arc<Mutex<Option<DaemonConnection>>>,
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

/// Helper to configure SO_NOSIGPIPE on UnixStream on macOS
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
            writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
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

#[tauri::command]
async fn list_chats() -> Result<Vec<ChatSummary>, String> {
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
    writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
            DaemonMessage::Chats { chats, .. } => Ok(chats),
            DaemonMessage::Error { error, .. } => Err(error.message),
            _ => Err("Unexpected response from daemon".to_string()),
        }
    } else {
        Err("Daemon connection closed".to_string())
    }
}

#[tauri::command]
async fn get_chat(chat_id: Uuid) -> Result<ChatTranscript, String> {
    let socket_path = Config::default_socket_path();
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
    configure_socket_safety(&stream);
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    let msg = ClientMessage::GetChat { id: Uuid::new_v4(), chat_id };
    let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
            DaemonMessage::Chat { title, messages, .. } => {
                Ok(ChatTranscript {
                    chat_id,
                    title,
                    messages,
                })
            }
            DaemonMessage::Error { error, .. } => Err(error.message),
            _ => Err("Unexpected response from daemon".to_string()),
        }
    } else {
        Err("Daemon connection closed".to_string())
    }
}

#[tauri::command]
async fn delete_chat(chat_id: Uuid) -> Result<bool, String> {
    let socket_path = Config::default_socket_path();
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
    configure_socket_safety(&stream);
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    let msg = ClientMessage::DeleteChat { id: Uuid::new_v4(), chat_id };
    let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
            DaemonMessage::Deleted { .. } => Ok(true),
            DaemonMessage::Error { error, .. } => Err(error.message),
            _ => Err("Unexpected response from daemon".to_string()),
        }
    } else {
        Err("Daemon connection closed".to_string())
    }
}

#[tauri::command]
async fn send_message<R: Runtime>(
    _app: AppHandle<R>,
    chat_id: Uuid,
    text: String,
    attachments: Vec<AttachmentInput>,
    on_event: tauri::ipc::Channel<DaemonMessage>,
) -> Result<(), String> {
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
            AttachmentKind::Image
        } else if att.mime == "application/pdf" {
            AttachmentKind::Pdf
        } else {
            AttachmentKind::Text
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
    let msg = ClientMessage::SendMessage {
        id: req_id,
        chat_id,
        text,
        attachments: processed_attachments,
    };
    let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

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
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn cancel_message(target_id: Uuid) -> Result<(), String> {
    let socket_path = Config::default_socket_path();
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| format!("Failed to connect to daemon: {e}"))?;
    configure_socket_safety(&stream);
    let (_, mut writer) = stream.into_split();

    let msg = ClientMessage::Cancel {
        id: Uuid::new_v4(),
        target_id,
    };
    let mut payload = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_settings() -> Result<SettingsInfo, String> {
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
    writer.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
            DaemonMessage::Settings { base_url, model, has_api_key, .. } => {
                Ok(SettingsInfo {
                    base_url,
                    model,
                    has_api_key,
                })
            }
            DaemonMessage::Error { error, .. } => Err(error.message),
            _ => Err("Unexpected response from daemon".to_string()),
        }
    } else {
        Err("Daemon connection closed".to_string())
    }
}

#[tauri::command]
async fn set_settings(payload: SettingsPayload) -> Result<SettingsInfo, String> {
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
    writer.write_all(p.as_bytes()).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    if let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        match serde_json::from_str::<DaemonMessage>(&line).map_err(|e| e.to_string())? {
            DaemonMessage::Settings { base_url, model, has_api_key, .. } => {
                Ok(SettingsInfo {
                    base_url,
                    model,
                    has_api_key,
                })
            }
            DaemonMessage::Error { error, .. } => Err(error.message),
            _ => Err("Unexpected response from daemon".to_string()),
        }
    } else {
        Err("Daemon connection closed".to_string())
    }
}

#[tauri::command]
async fn set_window_size<R: Runtime>(app: AppHandle<R>, width: f64, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn toggle_window<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn start_drag<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.start_dragging();
    }
    Ok(())
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

            if let Err(e) = app.global_shortcut().register(shortcut) {
                eprintln!("Failed to register global shortcut: {e}");
            } else {
                eprintln!("Registered global shortcut: {shortcut_str}");
            }

            // Initially show window centered, then user can toggle
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
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
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);
        if is_visible && is_focused {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
    } else {
        eprintln!("Warning: Webview window 'main' not found!");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shortcuts() {
        let opt: Result<Shortcut, _> = "Option+Space".parse();
        println!("Option+Space parse: {:?}", opt);
        let alt: Result<Shortcut, _> = "Alt+Space".parse();
        println!("Alt+Space parse: {:?}", alt);
    }
}
