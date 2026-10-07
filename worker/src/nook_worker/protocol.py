import json
from typing import Annotated, Literal
from uuid import UUID

from pydantic import BaseModel, ConfigDict, Field, TypeAdapter

AttachmentKind = Literal["image", "text", "pdf"]


class Attachment(BaseModel):
    model_config = ConfigDict(extra="forbid")

    id: UUID
    kind: AttachmentKind
    name: str = Field(min_length=1)
    mime: str = Field(min_length=1)
    size_bytes: int = Field(ge=0, le=10485760)
    path: str = Field(min_length=1)


ErrorCode = Literal[
    "endpoint_unreachable",
    "endpoint_error",
    "worker_crashed",
    "database_unavailable",
    "attachment_invalid",
    "invalid_request",
    "internal",
]


class ErrorInfo(BaseModel):
    model_config = ConfigDict(extra="forbid")

    code: ErrorCode
    message: str
    retryable: bool


class RunRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["run"] = "run"
    request_id: UUID
    chat_id: UUID
    text: str
    attachments: list[Attachment] = Field(default_factory=list, max_length=5)


class TitleRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["title"] = "title"
    request_id: UUID
    chat_id: UUID
    first_message: str


class CancelRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["cancel"] = "cancel"
    request_id: UUID


class DeleteChatRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["delete_chat"] = "delete_chat"
    request_id: UUID
    chat_id: UUID


class ShutdownRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["shutdown"] = "shutdown"


class ApprovalDecisionRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["approval_decision"] = "approval_decision"
    request_id: UUID
    call_id: str
    action: Literal[
        "allow_once",
        "allow_for_chat",
        "allow_for_chat_host",
        "always_allow",
        "deny",
    ]


RequestMessage = Annotated[
    RunRequest
    | TitleRequest
    | CancelRequest
    | DeleteChatRequest
    | ShutdownRequest
    | ApprovalDecisionRequest,
    Field(discriminator="type"),
]


class ReadyEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["ready"] = "ready"
    version: str


class MessageStartedEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["message_started"] = "message_started"
    request_id: UUID
    message_id: UUID


class TextDeltaEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["text_delta"] = "text_delta"
    request_id: UUID
    message_id: UUID
    text: str


class ToolCallStartedEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["tool_call_started"] = "tool_call_started"
    request_id: UUID
    message_id: UUID
    call_id: str
    name: str
    arguments: str


class ToolCallFinishedEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["tool_call_finished"] = "tool_call_finished"
    request_id: UUID
    message_id: UUID
    call_id: str
    result: str


class ApprovalRequestedEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["approval_requested"] = "approval_requested"
    request_id: UUID
    message_id: UUID
    call_id: str
    tool_name: str
    arguments: str
    explanation: str
    resource_summary: str
    actions: list[str]


class MessageFinishedEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["message_finished"] = "message_finished"
    request_id: UUID
    message_id: UUID
    status: Literal["complete", "cancelled"]


class TitleReadyEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["title_ready"] = "title_ready"
    request_id: UUID
    title: str


class DeletedEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["deleted"] = "deleted"
    request_id: UUID
    chat_id: UUID


class ErrorEvent(BaseModel):
    model_config = ConfigDict(extra="forbid")

    type: Literal["error"] = "error"
    request_id: UUID
    error: ErrorInfo


EventMessage = Annotated[
    ReadyEvent
    | MessageStartedEvent
    | TextDeltaEvent
    | ToolCallStartedEvent
    | ToolCallFinishedEvent
    | ApprovalRequestedEvent
    | MessageFinishedEvent
    | TitleReadyEvent
    | DeletedEvent
    | ErrorEvent,
    Field(discriminator="type"),
]

_request_adapter: TypeAdapter[RequestMessage] = TypeAdapter(RequestMessage)
_event_adapter: TypeAdapter[EventMessage] = TypeAdapter(EventMessage)


def parse_request(raw_line: str) -> RequestMessage:
    try:
        data = json.loads(raw_line)
    except json.JSONDecodeError as err:
        raise ValueError(f"Invalid JSON string: {err}") from err
    return _request_adapter.validate_python(data)


def parse_event(raw_line: str) -> EventMessage:
    try:
        data = json.loads(raw_line)
    except json.JSONDecodeError as err:
        raise ValueError(f"Invalid JSON string: {err}") from err
    return _event_adapter.validate_python(data)


def format_event(event: EventMessage) -> str:
    return _event_adapter.dump_json(event).decode("utf-8")
