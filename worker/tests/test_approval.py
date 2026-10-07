import asyncio
import json
from uuid import uuid4

import pytest

from nook_worker.approval import ApprovalManager
from nook_worker.protocol import parse_event


@pytest.mark.asyncio
async def test_approval_flow_emits_event_and_resumes_on_decision():
    manager = ApprovalManager()
    written_lines: list[str] = []

    async def mock_write_line(line: str) -> None:
        written_lines.append(line)

    req_id = uuid4()
    msg_id = uuid4()
    chat_id = uuid4()
    manager.set_context(
        request_id=req_id,
        message_id=msg_id,
        chat_id=chat_id,
        write_line=mock_write_line,
    )

    async def run_request() -> str:
        return await manager.request_approval(
            call_id="call_test_123",
            tool_name="nook:web_fetch",
            arguments=json.dumps({"url": "https://example.com"}),
            explanation="Requires approval",
            resource_summary="https://example.com",
            actions=["allow_once", "deny"],
        )

    task = asyncio.create_task(run_request())
    await asyncio.sleep(0.01)

    assert len(written_lines) == 1
    event = parse_event(written_lines[0])
    assert event.type == "approval_requested"
    assert event.call_id == "call_test_123"  # type: ignore[attr-defined]

    # Resolve decision
    resolved = manager.resolve_decision("call_test_123", "allow_once")
    assert resolved is True

    result = await task
    assert result == "allow_once"


@pytest.mark.asyncio
async def test_host_approval_caching():
    manager = ApprovalManager()
    chat1 = str(uuid4())
    chat2 = str(uuid4())

    assert not manager.is_host_approved(chat1, "example.com")
    manager.approve_host(chat1, "example.com")
    assert manager.is_host_approved(chat1, "example.com")
    assert not manager.is_host_approved(chat2, "example.com")


@pytest.mark.asyncio
async def test_approval_cancellation():
    manager = ApprovalManager()

    async def mock_write_line(_: str) -> None:
        pass

    manager.set_context(
        request_id=uuid4(),
        message_id=uuid4(),
        chat_id=uuid4(),
        write_line=mock_write_line,
    )

    task = asyncio.create_task(
        manager.request_approval(
            call_id="call_cancel_1",
            tool_name="nook:web_fetch",
            arguments="{}",
            explanation="Test cancel",
            resource_summary="test",
            actions=["allow_once", "deny"],
        )
    )
    await asyncio.sleep(0.01)

    manager.cancel_all()

    with pytest.raises(asyncio.CancelledError):
        await task
