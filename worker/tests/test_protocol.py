from pathlib import Path
from uuid import UUID

import pytest

from nook_worker.protocol import (
    ApprovalDecisionRequest,
    ApprovalRequestedEvent,
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

    assert len(lines) == 16

    # Requests in daemon-worker.ndjson
    req_run = parse_request(lines[0])
    assert isinstance(req_run, RunRequest)
    assert req_run.text == "What is the capital of France?"
    assert len(req_run.attachments) == 1
    assert req_run.attachments[0].name == "diagram.png"

    req_title = parse_request(lines[1])
    assert isinstance(req_title, TitleRequest)
    assert req_title.first_message == "What is the capital of France?"

    req_decision = parse_request(lines[2])
    assert isinstance(req_decision, ApprovalDecisionRequest)
    assert req_decision.call_id == "call_search_9"
    assert req_decision.action == "allow_once"

    req_cancel = parse_request(lines[3])
    assert isinstance(req_cancel, CancelRequest)

    req_delete = parse_request(lines[4])
    assert isinstance(req_delete, DeleteChatRequest)

    req_shutdown = parse_request(lines[5])
    assert isinstance(req_shutdown, ShutdownRequest)

    # Events in daemon-worker.ndjson
    ev_ready = parse_event(lines[6])
    assert isinstance(ev_ready, ReadyEvent)
    assert ev_ready.version == "0.1.0"

    ev_started = parse_event(lines[7])
    assert isinstance(ev_started, MessageStartedEvent)

    ev_delta = parse_event(lines[8])
    assert isinstance(ev_delta, TextDeltaEvent)
    assert ev_delta.text == "The capital is Paris."

    ev_tool_start = parse_event(lines[9])
    assert isinstance(ev_tool_start, ToolCallStartedEvent)
    assert ev_tool_start.name == "get_weather"

    ev_tool_finish = parse_event(lines[10])
    assert isinstance(ev_tool_finish, ToolCallFinishedEvent)
    assert ev_tool_finish.result == '{"temp":20}'

    ev_approval_req = parse_event(lines[11])
    assert isinstance(ev_approval_req, ApprovalRequestedEvent)
    assert ev_approval_req.call_id == "call_search_9"
    assert ev_approval_req.tool_name == "nook:web_search"

    ev_finished = parse_event(lines[12])
    assert isinstance(ev_finished, MessageFinishedEvent)
    assert ev_finished.status == "complete"

    ev_title_ready = parse_event(lines[13])
    assert isinstance(ev_title_ready, TitleReadyEvent)
    assert ev_title_ready.title == "Capital of France"

    ev_deleted = parse_event(lines[14])
    assert isinstance(ev_deleted, DeletedEvent)

    ev_error = parse_event(lines[15])
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
