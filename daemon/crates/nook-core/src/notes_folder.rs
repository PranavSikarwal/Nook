use crate::config::Config;
use crate::policy::grant_key;
use crate::protocol::{DaemonMessage, ErrorCode, ErrorInfo, ToolSourceState};
use crate::providers::notes::TOOL_IDS;
use crate::server::AppState;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

const NOT_SAVED: &str = "The folder could not be saved";

pub static NOTES_GRANT_KEYS: LazyLock<[&'static str; 6]> =
    LazyLock::new(|| TOOL_IDS.map(grant_key));

pub fn notes_grant_keys() -> &'static [&'static str] {
    &NOTES_GRANT_KEYS[..]
}

pub async fn apply_folder_change(state: &Arc<AppState>, config: &mut Config) {
    crate::tool_approval::deny_pending_notes_approvals(state).await;
    state
        .policy
        .authorizer
        .grant_store()
        .lock()
        .await
        .remove_tools(notes_grant_keys());
    let registry = state.providers.current().rebuilt(&config.tools);
    let descriptors = registry.descriptors().to_vec();
    state.providers.replace(registry);
    let supervisor = state.supervisor.lock().await.clone();
    if let Some(supervisor) = supervisor {
        supervisor.update_tools(descriptors).await;
    }
}

/// Returns `(supported, reason)` for the Notes tools.
/// `supported` is true when the registry's notes program was `Some`.
/// `reason` is one of the two fixed texts when the tools are off, or `None`.
pub fn notes_status(state: &AppState) -> (bool, Option<String>) {
    let source = state
        .providers
        .sources()
        .into_iter()
        .find(|s| s.source_id == "notes");
    match source {
        Some(s) => {
            let reason = if s.state == ToolSourceState::Off {
                s.reason
            } else {
                None
            };
            (true, reason)
        }
        None => (false, None),
    }
}

pub async fn get(state: &AppState, id: Uuid, tx_out: &mpsc::Sender<DaemonMessage>) {
    let folder = {
        let config = state.config.read().await;
        config
            .tools
            .notes
            .folder
            .chars()
            .take(200)
            .collect::<String>()
    };
    let (supported, reason) = notes_status(state);
    let _ = tx_out
        .send(DaemonMessage::NotesSettings {
            id,
            folder,
            supported,
            reason,
        })
        .await;
}

pub async fn set(
    state: &Arc<AppState>,
    id: Uuid,
    folder: String,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    if let Err(reason) = crate::config::validate_notes_folder(&folder) {
        let _ = tx_out
            .send(DaemonMessage::Error {
                id,
                error: ErrorInfo {
                    code: ErrorCode::InvalidRequest,
                    message: reason.to_string(),
                    retryable: false,
                },
            })
            .await;
        return;
    }

    let trimmed = folder.trim().to_string();
    let result: Result<DaemonMessage, String> = async {
        let mut config = state.config.write().await;
        update_notes_folder(&state.config_path, &trimmed).map_err(|error| {
            warn!("Could not save the Notes folder: {error}");
            NOT_SAVED.to_string()
        })?;
        config.tools.notes.folder = trimmed.clone();
        apply_folder_change(state, &mut config).await;
        let (supported, reason) = notes_status(state);
        let stored_folder: String = config.tools.notes.folder.chars().take(200).collect();
        Ok(DaemonMessage::NotesSettings {
            id,
            folder: stored_folder,
            supported,
            reason,
        })
    }
    .await;

    match result {
        Ok(msg) => {
            let _ = tx_out.send(msg).await;
        }
        Err(message) => {
            let _ = tx_out
                .send(DaemonMessage::Error {
                    id,
                    error: ErrorInfo {
                        code: ErrorCode::InvalidRequest,
                        message,
                        retryable: false,
                    },
                })
                .await;
        }
    }
}

fn update_notes_folder(
    path: &Path,
    folder: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut document = text.parse::<toml_edit::DocumentMut>()?;
    let tools = document
        .entry("tools")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or("`tools` in config.toml is not a table")?;
    tools.set_implicit(true);
    let notes = tools
        .entry("notes")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or("`tools.notes` in config.toml is not a table")?;
    notes.insert("folder", toml_edit::value(folder));
    write_private(path, &document.to_string())
}

fn write_private(
    path: &Path,
    contents: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("toml.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    Ok(())
}
