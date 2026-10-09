use chrono::{DateTime, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::protocol::{
    Attachment, AttachmentKind, ChatMessage, ChatSummary, ErrorCode, ErrorInfo, MessageStatus, Role,
};

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}

pub async fn repair_open_replies(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        WITH latest_messages AS (
            SELECT DISTINCT ON (chat_id) chat_id, id, role
            FROM app.messages
            ORDER BY chat_id, created_at DESC, id DESC
        )
        SELECT chat_id FROM latest_messages WHERE role = 'user'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let count = rows.len() as u64;
    for row in rows {
        let chat_id: Uuid = row.try_get("chat_id")?;
        let msg_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO app.messages (id, chat_id, role, text, status, error_code, error_text, created_at)
            VALUES ($1, $2, 'assistant', '', 'error', 'internal', 'Daemon stopped mid-reply', now())
            "#,
        )
        .bind(msg_id)
        .bind(chat_id)
        .execute(pool)
        .await?;
    }

    Ok(count)
}

pub async fn ensure_chat(pool: &PgPool, chat_id: Uuid, title: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO app.chats (id, title, created_at, updated_at)
        VALUES ($1, $2, now(), now())
        ON CONFLICT (id) DO UPDATE SET updated_at = now()
        "#,
    )
    .bind(chat_id)
    .bind(title)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_chat_title(
    pool: &PgPool,
    chat_id: Uuid,
    title: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE app.chats
        SET title = $1, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(title)
    .bind(chat_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn save_user_message(
    pool: &PgPool,
    message_id: Uuid,
    chat_id: Uuid,
    text: &str,
    attachments: &[Attachment],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO app.messages (id, chat_id, role, text, status, created_at)
        VALUES ($1, $2, 'user', $3, 'complete', now())
        "#,
    )
    .bind(message_id)
    .bind(chat_id)
    .bind(text)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE app.chats
        SET updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(chat_id)
    .execute(&mut *tx)
    .await?;

    for att in attachments {
        let kind_str = match att.kind {
            AttachmentKind::Image => "image",
            AttachmentKind::Text => "text",
            AttachmentKind::Pdf => "pdf",
        };

        sqlx::query(
            r#"
            INSERT INTO app.attachments (id, message_id, kind, name, mime, size_bytes, path)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(att.id)
        .bind(message_id)
        .bind(kind_str)
        .bind(&att.name)
        .bind(&att.mime)
        .bind(att.size_bytes)
        .bind(&att.path)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn save_assistant_message(
    pool: &PgPool,
    message_id: Uuid,
    chat_id: Uuid,
    text: &str,
    status: MessageStatus,
    error: Option<&ErrorInfo>,
) -> Result<(), sqlx::Error> {
    let status_str = match status {
        MessageStatus::Complete => "complete",
        MessageStatus::Cancelled => "cancelled",
        MessageStatus::Error => "error",
    };

    let (error_code, error_text) = if let Some(err) = error {
        let code_str = match err.code {
            ErrorCode::EndpointUnreachable => "endpoint_unreachable",
            ErrorCode::EndpointError => "endpoint_error",
            ErrorCode::WorkerCrashed => "worker_crashed",
            ErrorCode::DatabaseUnavailable => "database_unavailable",
            ErrorCode::AttachmentInvalid => "attachment_invalid",
            ErrorCode::InvalidRequest => "invalid_request",
            ErrorCode::Internal => "internal",
        };
        (Some(code_str.to_string()), Some(err.message.clone()))
    } else {
        (None, None)
    };

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO app.messages (id, chat_id, role, text, status, error_code, error_text, created_at)
        VALUES ($1, $2, 'assistant', $3, $4, $5, $6, now())
        ON CONFLICT (id) DO UPDATE
        SET text = $3, status = $4, error_code = $5, error_text = $6
        "#,
    )
    .bind(message_id)
    .bind(chat_id)
    .bind(text)
    .bind(status_str)
    .bind(error_code)
    .bind(error_text)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE app.chats
        SET updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(chat_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn list_chats(pool: &PgPool) -> Result<Vec<ChatSummary>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, title, updated_at
        FROM app.chats
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut summaries = Vec::new();
    for row in rows {
        let chat_id: Uuid = row.try_get("id")?;
        let title: String = row.try_get("title")?;
        let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
        summaries.push(ChatSummary {
            chat_id,
            title,
            updated_at,
        });
    }

    Ok(summaries)
}

pub async fn get_chat(
    pool: &PgPool,
    chat_id: Uuid,
) -> Result<Option<(String, Vec<ChatMessage>)>, sqlx::Error> {
    let chat_row = sqlx::query("SELECT title FROM app.chats WHERE id = $1")
        .bind(chat_id)
        .fetch_optional(pool)
        .await?;

    let title = match chat_row {
        Some(row) => row.try_get::<String, _>("title")?,
        None => return Ok(None),
    };

    let msg_rows = sqlx::query(
        r#"
        SELECT id, role, text, status, error_code, error_text, created_at
        FROM app.messages
        WHERE chat_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await?;

    let att_rows = sqlx::query(
        r#"
        SELECT a.id, a.message_id, a.kind, a.name, a.mime, a.size_bytes, a.path
        FROM app.attachments a
        JOIN app.messages m ON a.message_id = m.id
        WHERE m.chat_id = $1
        "#,
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await?;

    let mut attachments_by_msg: HashMap<Uuid, Vec<Attachment>> = HashMap::new();
    for a_row in att_rows {
        let id: Uuid = a_row.try_get("id")?;
        let message_id: Uuid = a_row.try_get("message_id")?;
        let kind_str: String = a_row.try_get("kind")?;
        let kind = match kind_str.as_str() {
            "image" => AttachmentKind::Image,
            "pdf" => AttachmentKind::Pdf,
            _ => AttachmentKind::Text,
        };
        let name: String = a_row.try_get("name")?;
        let mime: String = a_row.try_get("mime")?;
        let size_bytes: i64 = a_row.try_get("size_bytes")?;
        let path: String = a_row.try_get("path")?;

        attachments_by_msg
            .entry(message_id)
            .or_default()
            .push(Attachment {
                id,
                kind,
                name,
                mime,
                size_bytes,
                path,
            });
    }

    let mut messages = Vec::new();
    for m_row in msg_rows {
        let message_id: Uuid = m_row.try_get("id")?;
        let role_str: String = m_row.try_get("role")?;
        let role = match role_str.as_str() {
            "user" => Role::User,
            _ => Role::Assistant,
        };
        let text: String = m_row.try_get("text")?;
        let status_str: String = m_row.try_get("status")?;
        let status = match status_str.as_str() {
            "complete" => MessageStatus::Complete,
            "cancelled" => MessageStatus::Cancelled,
            _ => MessageStatus::Error,
        };

        let error_code_opt: Option<String> = m_row.try_get("error_code")?;
        let error_text_opt: Option<String> = m_row.try_get("error_text")?;

        let error = match (error_code_opt, error_text_opt) {
            (Some(c), Some(m)) => {
                let code = match c.as_str() {
                    "endpoint_unreachable" => ErrorCode::EndpointUnreachable,
                    "endpoint_error" => ErrorCode::EndpointError,
                    "worker_crashed" => ErrorCode::WorkerCrashed,
                    "database_unavailable" => ErrorCode::DatabaseUnavailable,
                    "attachment_invalid" => ErrorCode::AttachmentInvalid,
                    "invalid_request" => ErrorCode::InvalidRequest,
                    _ => ErrorCode::Internal,
                };
                Some(ErrorInfo {
                    code,
                    message: m,
                    retryable: false,
                })
            }
            _ => None,
        };

        let created_at: DateTime<Utc> = m_row.try_get("created_at")?;
        let attachments = attachments_by_msg.remove(&message_id).unwrap_or_default();

        messages.push(ChatMessage {
            message_id,
            role,
            text,
            status,
            error,
            attachments,
            created_at,
        });
    }

    Ok(Some((title, messages)))
}

pub async fn delete_chat(pool: &PgPool, chat_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM app.chats WHERE id = $1")
        .bind(chat_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
