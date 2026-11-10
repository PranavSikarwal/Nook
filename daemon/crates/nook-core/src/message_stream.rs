use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::db;
use crate::protocol::{
    Attachment, DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest, ErrorCode, ErrorInfo,
    FinishStatus, MessageStatus,
};
use crate::server::{validate_attachment, AppState};
use crate::supervisor::WorkerSupervisor;
use crate::tool_approval::{handle_authorize_tool, AuthorizeContext, AuthorizeToolCall};

fn check_send_request(text: &str, attachments: &[Attachment]) -> Result<(), ErrorInfo> {
    if text.trim().is_empty() && attachments.is_empty() {
        return Err(ErrorInfo {
            code: ErrorCode::InvalidRequest,
            message: "Message text and attachments cannot both be empty".to_string(),
            retryable: false,
        });
    }

    if attachments.len() > 5 {
        return Err(ErrorInfo {
            code: ErrorCode::AttachmentInvalid,
            message: "Attachments count exceeds maximum limit of 5".to_string(),
            retryable: false,
        });
    }

    for att in attachments {
        if let Err(msg) = validate_attachment(att) {
            return Err(ErrorInfo {
                code: ErrorCode::AttachmentInvalid,
                message: msg,
                retryable: false,
            });
        }
    }

    Ok(())
}

async fn cleanup_request(state: &AppState, id: Uuid) {
    let mut open_reqs = state.open_requests.lock().await;
    open_reqs.remove(&id);
    let mut cancelled = state.cancelled_requests.lock().await;
    cancelled.remove(&id);
    let mut approvals = state.pending_approvals.lock().await;
    approvals.retain(|_, v| v.client_request_id != id);
    state.running_tools.lock().await.remove(&id);
}

async fn fail_request(
    state: &AppState,
    id: Uuid,
    error: ErrorInfo,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    cleanup_request(state, id).await;
    let _ = tx_out.send(DaemonMessage::Error { id, error }).await;
}

pub(crate) async fn handle_send_message(
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

    if let Err(error) = check_send_request(&text, &attachments) {
        fail_request(&state, id, error, &tx_out).await;
        return;
    }

    let pool = match state.pool.get() {
        Some(p) => p,
        None => {
            fail_request(&state, id, crate::server::database_unavailable(), &tx_out).await;
            return;
        }
    };

    let user_msg_id = Uuid::new_v4();
    if let Err(e) = db::ensure_chat(&pool, chat_id, "New Chat").await {
        fail_request(
            &state,
            id,
            ErrorInfo {
                code: ErrorCode::DatabaseUnavailable,
                message: e.to_string(),
                retryable: true,
            },
            &tx_out,
        )
        .await;
        return;
    }

    if let Err(e) = db::save_user_message(&pool, user_msg_id, chat_id, &text, &attachments).await {
        fail_request(
            &state,
            id,
            ErrorInfo {
                code: ErrorCode::DatabaseUnavailable,
                message: e.to_string(),
                retryable: true,
            },
            &tx_out,
        )
        .await;
        return;
    }

    let supervisor = match state.supervisor.lock().await.clone() {
        Some(s) => s,
        None => {
            fail_request(
                &state,
                id,
                ErrorInfo {
                    code: ErrorCode::WorkerCrashed,
                    message: "Worker supervisor not running".to_string(),
                    retryable: true,
                },
                &tx_out,
            )
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
        cleanup_request(&state, id).await;
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
            fail_request(
                &state,
                id,
                ErrorInfo {
                    code: ErrorCode::WorkerCrashed,
                    message: e.to_string(),
                    retryable: true,
                },
                &tx_out,
            )
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
                let message = DaemonMessage::MessageStarted {
                    id,
                    chat_id,
                    message_id,
                };
                forward_unless_cancelled(&state, id, message, &tx_out).await;
            }

            DaemonWorkerEvent::TextDelta { text: delta, .. } => {
                forward_text_delta(
                    &state,
                    id,
                    chat_id,
                    asst_message_id,
                    delta,
                    &mut full_text,
                    &tx_out,
                )
                .await;
            }

            event @ (DaemonWorkerEvent::ToolCallStarted { .. }
            | DaemonWorkerEvent::ToolCallFinished { .. }) => {
                forward_tool_call_event(&state, id, chat_id, asst_message_id, event, &tx_out).await;
            }

            DaemonWorkerEvent::AuthorizeTool {
                message_id,
                call_id,
                chat_id: event_chat_id,
                tool_name,
                arguments,
                ..
            } => {
                handle_authorize_tool(
                    &state,
                    &AuthorizeContext {
                        client_request_id: id,
                        worker_request_id: worker_req_id,
                        chat_id,
                    },
                    AuthorizeToolCall {
                        message_id,
                        call_id,
                        chat_id: event_chat_id,
                        tool_name,
                        arguments,
                        execute: false,
                    },
                    &tx_out,
                )
                .await;
            }

            DaemonWorkerEvent::ExecuteTool {
                message_id,
                call_id,
                chat_id: event_chat_id,
                tool_name,
                arguments,
                ..
            } => {
                handle_authorize_tool(
                    &state,
                    &AuthorizeContext {
                        client_request_id: id,
                        worker_request_id: worker_req_id,
                        chat_id,
                    },
                    AuthorizeToolCall {
                        message_id,
                        call_id,
                        chat_id: event_chat_id,
                        tool_name,
                        arguments,
                        execute: true,
                    },
                    &tx_out,
                )
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

                forward_unless_cancelled(&state, id, DaemonMessage::Error { id, error }, &tx_out)
                    .await;
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

    cleanup_request(&state, id).await;
}

pub async fn forward_unless_cancelled(
    state: &AppState,
    client_request_id: Uuid,
    message: DaemonMessage,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    let cancelled = state
        .cancelled_requests
        .lock()
        .await
        .contains(&client_request_id);
    if !cancelled {
        let _ = tx_out.send(message).await;
    }
}

pub async fn forward_text_delta(
    state: &AppState,
    client_request_id: Uuid,
    chat_id: Uuid,
    message_id: Uuid,
    delta: String,
    full_text: &mut String,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    full_text.push_str(&delta);
    let message = DaemonMessage::TextDelta {
        id: client_request_id,
        chat_id,
        message_id,
        text: delta,
    };
    forward_unless_cancelled(state, client_request_id, message, tx_out).await;
}

pub async fn forward_tool_call_event(
    state: &AppState,
    client_request_id: Uuid,
    chat_id: Uuid,
    message_id: Uuid,
    event: DaemonWorkerEvent,
    tx_out: &mpsc::Sender<DaemonMessage>,
) {
    let message = match event {
        DaemonWorkerEvent::ToolCallStarted {
            call_id,
            name,
            arguments,
            ..
        } => DaemonMessage::ToolCallStarted {
            id: client_request_id,
            chat_id,
            message_id,
            call_id,
            name,
            arguments,
        },
        DaemonWorkerEvent::ToolCallFinished {
            call_id, result, ..
        } => DaemonMessage::ToolCallFinished {
            id: client_request_id,
            chat_id,
            message_id,
            call_id,
            result,
        },
        _ => return,
    };
    forward_unless_cancelled(state, client_request_id, message, tx_out).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Attachment, AttachmentKind, ErrorCode};

    fn make_attachment(name: &str, mime: &str, kind: AttachmentKind) -> Attachment {
        Attachment {
            id: Uuid::new_v4(),
            kind,
            name: name.to_string(),
            mime: mime.to_string(),
            size_bytes: 1024,
            path: format!("/tmp/{name}"),
        }
    }

    #[test]
    fn check_send_request_rejects_empty_text_and_no_attachments() {
        let err = check_send_request("", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            err.message,
            "Message text and attachments cannot both be empty"
        );

        let err_spaces = check_send_request("   \t\n ", &[]).unwrap_err();
        assert_eq!(err_spaces.code, ErrorCode::InvalidRequest);
        assert_eq!(
            err_spaces.message,
            "Message text and attachments cannot both be empty"
        );
    }

    #[test]
    fn check_send_request_rejects_exceeding_attachments_limit() {
        let atts: Vec<_> = (0..6)
            .map(|i| make_attachment(&format!("file{i}.txt"), "text/plain", AttachmentKind::Text))
            .collect();
        let err = check_send_request("hello", &atts).unwrap_err();
        assert_eq!(err.code, ErrorCode::AttachmentInvalid);
        assert_eq!(err.message, "Attachments count exceeds maximum limit of 5");
    }

    #[test]
    fn check_send_request_rejects_invalid_attachment() {
        let invalid = make_attachment(
            "test.exe",
            "application/x-msdownload",
            AttachmentKind::Image,
        );
        let expected_msg = validate_attachment(&invalid).unwrap_err();
        let err = check_send_request("hello", &[invalid]).unwrap_err();
        assert_eq!(err.code, ErrorCode::AttachmentInvalid);
        assert_eq!(err.message, expected_msg);
    }

    #[test]
    fn check_send_request_empty_check_wins_over_count_check() {
        let err = check_send_request("", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert_eq!(
            err.message,
            "Message text and attachments cannot both be empty"
        );
    }
}
