import json
from pathlib import Path
from uuid import UUID

import pytest

from nook_worker.protocol import (
    ApprovalDecisionRequest,
    ApprovalRequestedEvent,
    AuthorizationResultRequest,
    AuthorizeToolEvent,
    CancelRequest,
    DeleteChatRequest,
    DeletedEvent,
    ErrorEvent,
    MessageFinishedEvent,
    MessageStartedEvent,
    ReadyEvent,
    RunRequest,
    ShutdownRequest,
    TextDeltaEvent,
    TitleReadyEvent,
    TitleRequest,
    ToolCallFinishedEvent,
    ToolCallStartedEvent,
    format_event,
    parse_event,
    parse_request,
)


def test_parse_examples_file():
    root = Path(__file__).resolve().parents[2]
    examples_path = root / "contracts" / "examples" / "daemon-worker.ndjson"
    assert examples_path.is_file(), f"Examples file not found at {examples_path}"

    with open(examples_path, encoding="utf-8") as f:
        lines = [line.strip() for line in f if line.strip()]

    def example(message_type: str, index: int) -> str:
        matches = [line for line in lines if json.loads(line)["type"] == message_type]
        return matches[index]

    # Requests in daemon-worker.ndjson
    req_run = parse_request(example("run", 0))
    assert isinstance(req_run, RunRequest)
    assert req_run.text == "What is the capital of France?"
    assert len(req_run.attachments) == 1
    assert req_run.attachments[0].name == "diagram.png"

    req_title = parse_request(example("title", 0))
    assert isinstance(req_title, TitleRequest)
    assert req_title.first_message == "What is the capital of France?"

    req_decision = parse_request(example("approval_decision", 0))
    assert isinstance(req_decision, ApprovalDecisionRequest)
    assert req_decision.call_id == "call_search_9"
    assert req_decision.action == "allow_once"

    req_allow = parse_request(example("authorization_result", 0))
    assert isinstance(req_allow, AuthorizationResultRequest)
    assert req_allow.decision == "allow"
    assert req_allow.reason is None

    req_deny = parse_request(example("authorization_result", 1))
    assert isinstance(req_deny, AuthorizationResultRequest)
    assert req_deny.decision == "deny"
    assert req_deny.reason

    req_cancel = parse_request(example("cancel", 0))
    assert isinstance(req_cancel, CancelRequest)

    req_delete = parse_request(example("delete_chat", 0))
    assert isinstance(req_delete, DeleteChatRequest)

    req_shutdown = parse_request(example("shutdown", 0))
    assert isinstance(req_shutdown, ShutdownRequest)

    # Events in daemon-worker.ndjson
    ev_ready = parse_event(example("ready", 0))
    assert isinstance(ev_ready, ReadyEvent)
    assert ev_ready.version == "0.1.0"

    ev_started = parse_event(example("message_started", 0))
    assert isinstance(ev_started, MessageStartedEvent)

    ev_delta = parse_event(example("text_delta", 0))
    assert isinstance(ev_delta, TextDeltaEvent)
    assert ev_delta.text == "The capital is Paris."

    ev_tool_start = parse_event(example("tool_call_started", 0))
    assert isinstance(ev_tool_start, ToolCallStartedEvent)
    assert ev_tool_start.name == "get_weather"

    ev_tool_finish = parse_event(example("tool_call_finished", 0))
    assert isinstance(ev_tool_finish, ToolCallFinishedEvent)
    assert ev_tool_finish.result == '{"temp":20}'

    ev_approval_req = parse_event(example("approval_requested", 0))
    assert isinstance(ev_approval_req, ApprovalRequestedEvent)
    assert ev_approval_req.call_id == "call_search_9"
    assert ev_approval_req.tool_name == "nook:web_search"

    ev_authorize = parse_event(example("authorize_tool", 0))
    assert isinstance(ev_authorize, AuthorizeToolEvent)
    assert ev_authorize.call_id == "call_fetch_1"

    ev_finished = parse_event(example("message_finished", 0))
    assert isinstance(ev_finished, MessageFinishedEvent)
    assert ev_finished.status == "complete"

    ev_title_ready = parse_event(example("title_ready", 0))
    assert isinstance(ev_title_ready, TitleReadyEvent)
    assert ev_title_ready.title == "Capital of France"

    ev_deleted = parse_event(example("deleted", 0))
    assert isinstance(ev_deleted, DeletedEvent)

    ev_error = parse_event(example("error", 0))
    assert isinstance(ev_error, ErrorEvent)
    assert ev_error.error.code == "endpoint_unreachable"
    assert ev_error.error.retryable is True


def test_format_event_round_trip():
    event = TextDeltaEvent(
        request_id=UUID("55555555-5555-5555-5555-555555555551"),
        message_id=UUID("44444444-4444-4444-4444-444444444444"),
        text="Hello world",
    )
    raw = format_event(event)
    parsed = parse_event(raw)
    assert parsed == event


def test_reject_invalid_request():
    with pytest.raises(ValueError):
        parse_request('{"type": "unknown"}')

    with pytest.raises(ValueError):
        parse_request('{"type": "run", "request_id": "not-a-uuid"}')

    with pytest.raises(ValueError):
        parse_request("not-json")


REQUEST_ID = UUID("55555555-5555-5555-5555-555555555551")
MESSAGE_ID = UUID("44444444-4444-4444-4444-444444444444")
CHAT_ID = UUID("33333333-3333-3333-3333-333333333333")


def _authorization_result_line(**overrides: str) -> str:
    payload = {
        "type": "authorization_result",
        "request_id": str(REQUEST_ID),
        "call_id": "call_1",
        "decision": "allow",
        **overrides,
    }
    return json.dumps(payload)


def test_authorize_tool_event_round_trip():
    event = AuthorizeToolEvent(
        request_id=REQUEST_ID,
        message_id=MESSAGE_ID,
        call_id="call_1",
        chat_id=CHAT_ID,
        tool_name="nook:web_fetch",
        arguments='{"url":"https://example.com"}',
    )
    raw = format_event(event)
    assert '"type":"authorize_tool"' in raw
    assert parse_event(raw) == event


def test_authorize_tool_event_rejects_unknown_field():
    raw = json.dumps(
        {
            "type": "authorize_tool",
            "request_id": str(REQUEST_ID),
            "message_id": str(MESSAGE_ID),
            "call_id": "c",
            "chat_id": str(CHAT_ID),
            "tool_name": "t",
            "arguments": "{}",
            "extra": 1,
        }
    )
    with pytest.raises(ValueError):
        parse_event(raw)


@pytest.mark.parametrize("decision", ["allow", "deny"])
def test_authorization_result_request_parses(decision):
    request = parse_request(
        _authorization_result_line(decision=decision, reason="blocked")
    )
    assert isinstance(request, AuthorizationResultRequest)
    assert request.decision == decision
    assert request.reason == "blocked"


def test_authorization_result_request_reason_is_optional():
    request = parse_request(_authorization_result_line())
    assert isinstance(request, AuthorizationResultRequest)
    assert request.reason is None


def test_authorization_result_request_rejects_bad_decision():
    with pytest.raises(ValueError):
        parse_request(_authorization_result_line(decision="maybe"))
