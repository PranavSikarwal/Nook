use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    Text,
    Pdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub kind: AttachmentKind,
    pub name: String,
    pub mime: String,
    pub size_bytes: i64,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    EndpointUnreachable,
    EndpointError,
    WorkerCrashed,
    DatabaseUnavailable,
    AttachmentInvalid,
    InvalidRequest,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Complete,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishStatus {
    Complete,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub message_id: Uuid,
    pub role: Role,
    pub text: String,
    pub status: MessageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
    pub attachments: Vec<Attachment>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSummary {
    pub chat_id: Uuid,
    pub title: String,
    pub updated_at: DateTime<Utc>,
}

/// Requests sent from the Panel or `nookctl` to the Daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    SendMessage {
        id: Uuid,
        chat_id: Uuid,
        text: String,
        attachments: Vec<Attachment>,
    },
    Cancel {
        id: Uuid,
        target_id: Uuid,
    },
    ListChats {
        id: Uuid,
    },
    GetChat {
        id: Uuid,
        chat_id: Uuid,
    },
    DeleteChat {
        id: Uuid,
        chat_id: Uuid,
    },
    GetSettings {
        id: Uuid,
    },
    SetSettings {
        id: Uuid,
        base_url: String,
        model: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
    },
    Ping {
        id: Uuid,
    },
    ApprovalDecision {
        id: Uuid,
        chat_id: Uuid,
        call_id: String,
        action: String,
    },
}

/// Replies and events sent from the Daemon to the Panel or `nookctl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonMessage {
    MessageStarted {
        id: Uuid,
        chat_id: Uuid,
        message_id: Uuid,
    },
    TextDelta {
        id: Uuid,
        chat_id: Uuid,
        message_id: Uuid,
        text: String,
    },
    ToolCallStarted {
        id: Uuid,
        chat_id: Uuid,
        message_id: Uuid,
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallFinished {
        id: Uuid,
        chat_id: Uuid,
        message_id: Uuid,
        call_id: String,
        result: String,
    },
    ApprovalRequested {
        id: Uuid,
        chat_id: Uuid,
        message_id: Uuid,
        call_id: String,
        tool_name: String,
        arguments: String,
        explanation: String,
        resource_summary: String,
        actions: Vec<String>,
    },
    MessageFinished {
        id: Uuid,
        chat_id: Uuid,
        message_id: Uuid,
        status: FinishStatus,
    },
    ChatTitled {
        chat_id: Uuid,
        title: String,
    },
    Chats {
        id: Uuid,
        chats: Vec<ChatSummary>,
    },
    Chat {
        id: Uuid,
        chat_id: Uuid,
        title: String,
        messages: Vec<ChatMessage>,
    },
    Deleted {
        id: Uuid,
        chat_id: Uuid,
    },
    Settings {
        id: Uuid,
        base_url: String,
        model: String,
        has_api_key: bool,
    },
    Cancelled {
        id: Uuid,
        target_id: Uuid,
    },
    Pong {
        id: Uuid,
    },
    Error {
        id: Uuid,
        error: ErrorInfo,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecision {
    Allow,
    Deny,
}

/// Requests sent from the Daemon to the Worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonWorkerRequest {
    Run {
        request_id: Uuid,
        chat_id: Uuid,
        text: String,
        attachments: Vec<Attachment>,
    },
    Title {
        request_id: Uuid,
        chat_id: Uuid,
        first_message: String,
    },
    Cancel {
        request_id: Uuid,
    },
    DeleteChat {
        request_id: Uuid,
        chat_id: Uuid,
    },
    ApprovalDecision {
        request_id: Uuid,
        call_id: String,
        action: String,
    },
    AuthorizationResult {
        request_id: Uuid,
        call_id: String,
        decision: AuthorizationDecision,
        reason: Option<String>,
    },
    Shutdown,
}

/// Events sent from the Worker to the Daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonWorkerEvent {
    Ready {
        version: String,
    },
    MessageStarted {
        request_id: Uuid,
        message_id: Uuid,
    },
    TextDelta {
        request_id: Uuid,
        message_id: Uuid,
        text: String,
    },
    ToolCallStarted {
        request_id: Uuid,
        message_id: Uuid,
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallFinished {
        request_id: Uuid,
        message_id: Uuid,
        call_id: String,
        result: String,
    },
    ApprovalRequested {
        request_id: Uuid,
        message_id: Uuid,
        call_id: String,
        tool_name: String,
        arguments: String,
        explanation: String,
        resource_summary: String,
        actions: Vec<String>,
    },
    AuthorizeTool {
        request_id: Uuid,
        message_id: Uuid,
        call_id: String,
        chat_id: Uuid,
        tool_name: String,
        arguments: String,
    },
    MessageFinished {
        request_id: Uuid,
        message_id: Uuid,
        status: FinishStatus,
    },
    TitleReady {
        request_id: Uuid,
        title: String,
    },
    Deleted {
        request_id: Uuid,
        chat_id: Uuid,
    },
    Error {
        request_id: Uuid,
        error: ErrorInfo,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn authorize_tool_event_round_trips() {
        let event = DaemonWorkerEvent::AuthorizeTool {
            request_id: Uuid::new_v4(),
            message_id: Uuid::new_v4(),
            call_id: "call-1".to_string(),
            chat_id: Uuid::new_v4(),
            tool_name: "nook:web_fetch".to_string(),
            arguments: r#"{"url":"https://example.com"}"#.to_string(),
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["type"], "authorize_tool");
        assert_eq!(value["tool_name"], "nook:web_fetch");
        let back: DaemonWorkerEvent = serde_json::from_value(value).unwrap();
        assert_eq!(back, event);
    }

    #[test]
    fn authorization_result_request_round_trips() {
        let request = DaemonWorkerRequest::AuthorizationResult {
            request_id: Uuid::new_v4(),
            call_id: "call-1".to_string(),
            decision: AuthorizationDecision::Deny,
            reason: Some("not allowed".to_string()),
        };
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["type"], "authorization_result");
        assert_eq!(value["decision"], "deny");
        let back: DaemonWorkerRequest = serde_json::from_value(value).unwrap();
        assert_eq!(back, request);
    }

    #[test]
    fn authorization_result_accepts_allow_without_reason() {
        let id = Uuid::new_v4();
        let value = json!({
            "type": "authorization_result",
            "request_id": id,
            "call_id": "c",
            "decision": "allow",
            "reason": null
        });
        let request: DaemonWorkerRequest = serde_json::from_value(value).unwrap();
        assert_eq!(
            request,
            DaemonWorkerRequest::AuthorizationResult {
                request_id: id,
                call_id: "c".to_string(),
                decision: AuthorizationDecision::Allow,
                reason: None,
            }
        );
    }
}
