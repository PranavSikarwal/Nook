import asyncio
import json
from collections.abc import AsyncIterator
from typing import Any, Literal
from uuid import UUID, uuid4

import pytest
from langchain_core.messages import AIMessageChunk

from nook_worker.agent import DENIAL_NOTICE, RealAgentRunner
from nook_worker.authorization import (
    AuthorizationManager,
    default_authorization_manager,
)
from nook_worker.protocol import (
    MessageFinishedEvent,
    RunRequest,
    TextDeltaEvent,
)


def _manager_with_daemon(
    decision: Literal["allow", "deny"],
) -> tuple[AuthorizationManager, UUID]:
    manager = AuthorizationManager(authorization_timeout=0.2)
    request_id = uuid4()

    async def write_line(line: str) -> None:
        message = json.loads(line)
        if message["type"] == "authorize_tool":
            manager.resolve(
                UUID(message["request_id"]), message["call_id"], decision, "x"
            )

    manager.set_context(request_id, uuid4(), uuid4(), write_line)
    return manager, request_id


@pytest.mark.asyncio
async def test_deny_marks_the_request_as_denied():
    manager, request_id = _manager_with_daemon("deny")
    decision = await manager.authorize("nook:web_fetch", "{}")
    assert not decision.allowed
    assert manager.was_denied(request_id)


@pytest.mark.asyncio
async def test_allow_does_not_mark_the_request():
    manager, request_id = _manager_with_daemon("allow")
    decision = await manager.authorize("nook:web_fetch", "{}")
    assert decision.allowed
    assert not manager.was_denied(request_id)


@pytest.mark.asyncio
async def test_timeout_marks_the_request_as_denied():
    manager = AuthorizationManager(authorization_timeout=0.05)
    request_id = uuid4()

    async def write_line(line: str) -> None:
        return None

    manager.set_context(request_id, uuid4(), uuid4(), write_line)
    decision = await manager.authorize("nook:web_fetch", "{}")
    assert not decision.allowed
    assert manager.was_denied(request_id)


@pytest.mark.asyncio
async def test_clear_context_forgets_the_denial():
    manager, request_id = _manager_with_daemon("deny")
    await manager.authorize("nook:web_fetch", "{}")
    manager.clear_context(request_id)
    assert not manager.was_denied(request_id)


class DenyingAgent:
    def __init__(self, request_id: UUID) -> None:
        self.request_id = request_id

    async def astream(
        self,
        inputs: dict[str, Any],
        stream_mode: str = "messages",
        config: dict[str, Any] | None = None,
    ) -> AsyncIterator[tuple[Any, dict[str, Any]]]:
        yield AIMessageChunk(content="Let me fetch it.", id="a"), {"run_id": "1"}
        default_authorization_manager.mark_denied(self.request_id)
        yield AIMessageChunk(content="The page says hello.", id="b"), {"run_id": "2"}
        await asyncio.sleep(0)
        yield AIMessageChunk(content=" More text.", id="b"), {"run_id": "2"}


@pytest.mark.asyncio
async def test_run_stops_after_a_denial_and_ends_complete():
    request = RunRequest(request_id=uuid4(), chat_id=uuid4(), text="fetch it")
    runner = RealAgentRunner(DenyingAgent(request.request_id))

    events = [event async for event in runner.run(request)]

    text = "".join(e.text for e in events if isinstance(e, TextDeltaEvent))
    assert "Let me fetch it." in text
    assert "The page says hello." not in text
    assert text.endswith(DENIAL_NOTICE)
    finished = events[-1]
    assert isinstance(finished, MessageFinishedEvent)
    assert finished.status == "complete"
