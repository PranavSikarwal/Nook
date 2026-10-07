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

    # Resolve decision with matching request_id
    resolved = manager.resolve_decision(req_id, "call_test_123", "allow_once")
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


@pytest.mark.asyncio
async def test_approval_fails_closed_without_context():
    manager = ApprovalManager()
    manager.clear_context()

    # Without context, must fail-closed (deny)
    res = await manager.request_approval(
        call_id="call_no_ctx",
        tool_name="nook:web_fetch",
        arguments="{}",
        explanation="No context test",
        resource_summary="test",
        actions=["allow_once", "deny"],
    )
    assert res == "deny"


@pytest.mark.asyncio
async def test_approval_write_failure_cleans_up_future():
    manager = ApprovalManager()

    async def failing_write_line(_: str) -> None:
        raise OSError("Broken pipe")

    manager.set_context(
        request_id=uuid4(),
        message_id=uuid4(),
        chat_id=uuid4(),
        write_line=failing_write_line,
    )

    with pytest.raises(OSError, match="Broken pipe"):
        await manager.request_approval(
            call_id="call_pipe_err",
            tool_name="nook:web_fetch",
            arguments="{}",
            explanation="Test write error",
            resource_summary="test",
            actions=["allow_once", "deny"],
        )

    # Future must not leak in pending decisions
    assert "call_pipe_err" not in manager._pending_decisions


@pytest.mark.asyncio
async def test_concurrent_runs_context_isolation():
    manager = ApprovalManager()
    events_run1: list[str] = []
    events_run2: list[str] = []

    async def write_run1(line: str) -> None:
        events_run1.append(line)

    async def write_run2(line: str) -> None:
        events_run2.append(line)

    req1, msg1, chat1 = uuid4(), uuid4(), uuid4()
    req2, msg2, chat2 = uuid4(), uuid4(), uuid4()

    async def run1():
        manager.set_context(req1, msg1, chat1, write_run1)
        await asyncio.sleep(0.02)
        return await manager.request_approval(
            call_id="call_r1",
            tool_name="nook:web_fetch",
            arguments="{}",
            explanation="Run 1",
            resource_summary="url1",
            actions=["allow_once", "deny"],
        )

    async def run2():
        manager.set_context(req2, msg2, chat2, write_run2)
        await asyncio.sleep(0.01)
        return await manager.request_approval(
            call_id="call_r2",
            tool_name="nook:web_fetch",
            arguments="{}",
            explanation="Run 2",
            resource_summary="url2",
            actions=["allow_once", "deny"],
        )

    t1 = asyncio.create_task(run1())
    t2 = asyncio.create_task(run2())

    await asyncio.sleep(0.05)

    assert len(events_run1) == 1
    assert len(events_run2) == 1

    ev1 = parse_event(events_run1[0])
    ev2 = parse_event(events_run2[0])

    assert ev1.request_id == req1  # type: ignore[attr-defined]
    assert ev2.request_id == req2  # type: ignore[attr-defined]

    # Resolve decisions separately
    manager.resolve_decision(req1, "call_r1", "allow_once")
    manager.resolve_decision(req2, "call_r2", "deny")

    assert await t1 == "allow_once"
    assert await t2 == "deny"


@pytest.mark.asyncio
async def test_resolve_decision_validates_request_and_action():
    manager = ApprovalManager()

    async def mock_write_line(_: str) -> None:
        pass

    req_id = uuid4()
    wrong_req_id = uuid4()
    manager.set_context(
        request_id=req_id,
        message_id=uuid4(),
        chat_id=uuid4(),
        write_line=mock_write_line,
    )

    task = asyncio.create_task(
        manager.request_approval(
            call_id="call_val_1",
            tool_name="nook:web_fetch",
            arguments="{}",
            explanation="Test",
            resource_summary="test",
            actions=["allow_once", "deny"],
        )
    )
    await asyncio.sleep(0.01)

    # 1. Wrong request ID rejected
    assert manager.resolve_decision(wrong_req_id, "call_val_1", "allow_once") is False

    # 2. Unlisted action rejected
    assert manager.resolve_decision(req_id, "call_val_1", "invalid_action") is False

    # 3. Valid resolution succeeds
    assert manager.resolve_decision(req_id, "call_val_1", "allow_once") is True
    assert await task == "allow_once"


@pytest.mark.asyncio
async def test_clear_chat_prunes_cache():
    manager = ApprovalManager()
    chat_a = str(uuid4())
    chat_b = str(uuid4())

    manager.approve_host(chat_a, "docs.example.com")
    manager.approve_host(chat_b, "api.example.com")

    assert manager.is_host_approved(chat_a, "docs.example.com")
    assert manager.is_host_approved(chat_b, "api.example.com")

    manager.clear_chat(chat_a)

    assert not manager.is_host_approved(chat_a, "docs.example.com")
    assert manager.is_host_approved(chat_b, "api.example.com")
