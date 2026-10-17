import asyncio
import json
from typing import Any
from uuid import uuid4

import pytest

from nook_worker import agent
from nook_worker.agent import _create_registry_runner
from nook_worker.authorization import AuthorizationManager
from nook_worker.protocol import (
    AuthorizeToolEvent,
    EventMessage,
    ToolCallFinishedEvent,
    ToolCallStartedEvent,
    parse_event,
)
from nook_worker.registry import ToolDefinition, ToolRegistry
from nook_worker.tools.fetch import StrictWebFetchInput
from nook_worker.tools.search import StrictWebSearchInput


class FakeTool:
    def __init__(self) -> None:
        self.executor_calls: list[dict[str, Any]] = []
        self.authorize_events: list[AuthorizeToolEvent] = []
        self.events: list[EventMessage] = []
        self.decision: str = "allow"
        self.reason: str | None = None
        self.manager = AuthorizationManager()

    async def executor(self, **kwargs: Any) -> str:
        self.executor_calls.append(kwargs)
        return "executed"

    async def daemon(self, line: str) -> None:
        event = parse_event(line)
        self.events.append(event)
        if not isinstance(event, AuthorizeToolEvent):
            return
        self.authorize_events.append(event)
        self.manager.resolve(
            event.request_id,
            event.call_id,
            "allow" if self.decision == "allow" else "deny",
            self.reason,
        )

    def start_run(self) -> None:
        self.manager.set_context(uuid4(), uuid4(), uuid4(), self.daemon)


@pytest.fixture
def fake(monkeypatch: pytest.MonkeyPatch) -> FakeTool:
    fake_tool = FakeTool()
    registry = ToolRegistry()
    registry.register(
        ToolDefinition(
            name="nook:web_search",
            description="Search",
            argument_schema=StrictWebSearchInput,
            capabilities={},
            executor=fake_tool.executor,
        )
    )
    registry.register(
        ToolDefinition(
            name="nook:web_fetch",
            description="Fetch",
            argument_schema=StrictWebFetchInput,
            capabilities={},
            executor=fake_tool.executor,
        )
    )
    monkeypatch.setattr(agent, "default_registry", registry)
    monkeypatch.setattr(agent, "default_authorization_manager", fake_tool.manager)
    fake_tool.start_run()
    return fake_tool


@pytest.mark.parametrize(
    ("tool", "arguments"),
    [
        ("nook:web_search", {"query": ""}),
        ("nook:web_search", {"query": "   "}),
        ("nook:web_fetch", {}),
        ("nook:web_fetch", {"url": "ftp://example.com/a"}),
        ("nook:web_fetch", {"url": "https:///nohost"}),
    ],
)
async def test_invalid_arguments_never_emit_authorize_tool(fake, tool, arguments):
    result = await _create_registry_runner(tool)(**arguments)

    assert result.startswith("Error:")
    assert fake.authorize_events == []
    assert fake.executor_calls == []


async def test_authorized_search_runs_executor_once_with_cleaned_arguments(fake):
    result = await _create_registry_runner("nook:web_search")(query=" cats ")

    assert result == "executed"
    assert fake.executor_calls == [{"query": "cats", "max_results": 5}]
    [event] = fake.authorize_events
    assert event.tool_name == "nook:web_search"
    assert json.loads(event.arguments) == {"query": "cats", "max_results": 5}


async def test_authorized_fetch_runs_executor_once(fake):
    result = await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    assert result == "executed"
    assert fake.executor_calls == [{"url": "https://example.com/"}]
    [event] = fake.authorize_events
    assert event.tool_name == "nook:web_fetch"
    assert json.loads(event.arguments) == {"url": "https://example.com/"}


@pytest.mark.parametrize(
    ("tool", "arguments"),
    [
        ("nook:web_search", {"query": "cats"}),
        ("nook:web_fetch", {"url": "https://example.com/"}),
    ],
)
async def test_denied_call_never_runs_executor(fake, tool, arguments):
    fake.decision = "deny"

    result = await _create_registry_runner(tool)(**arguments)

    assert result == "Error: request denied"
    assert fake.executor_calls == []


async def test_denied_call_includes_reason_when_given(fake):
    fake.decision = "deny"
    fake.reason = "host is not allowed"

    result = await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    assert result == "Error: request denied: host is not allowed"
    assert fake.executor_calls == []


async def test_call_without_active_run_is_denied(fake):
    fake.manager.clear_context()

    result = await _create_registry_runner("nook:web_search")(query="cats")

    assert result.startswith("Error: request denied")
    assert fake.executor_calls == []


async def test_unregistered_tool_returns_error_without_authorization(fake):
    result = await _create_registry_runner("nook:missing")()

    assert result == "Error: Tool 'nook:missing' is not registered."
    assert fake.authorize_events == []


def test_missing_required_field_returns_error_text_to_model():
    from pydantic import ValidationError

    from nook_worker.agent import _format_validation_error
    from nook_worker.tools.fetch import WebFetchInput

    with pytest.raises(ValidationError) as caught:
        WebFetchInput.model_validate({})
    message = _format_validation_error(caught.value)
    assert message.startswith("Error: invalid arguments.")
    assert "url" in message


async def test_allowed_call_emits_started_then_finished_with_authorize_call_id(fake):
    await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    [authorize, started, finished] = fake.events
    assert isinstance(authorize, AuthorizeToolEvent)
    assert isinstance(started, ToolCallStartedEvent)
    assert isinstance(finished, ToolCallFinishedEvent)
    assert started.call_id == authorize.call_id == finished.call_id
    assert started.request_id == authorize.request_id
    assert started.message_id == authorize.message_id
    assert started.name == "nook:web_fetch"
    assert json.loads(started.arguments) == {"url": "https://example.com/"}
    assert finished.result == "executed"


async def test_started_is_emitted_before_executor_runs(fake):
    seen_before_executor: list[str] = []

    async def executor(**kwargs: Any) -> str:
        seen_before_executor.extend(e.type for e in fake.events)
        return "ok"

    agent.default_registry.get("nook:web_fetch").executor = executor  # type: ignore[union-attr]

    await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    assert seen_before_executor == ["authorize_tool", "tool_call_started"]


async def test_long_result_is_truncated_in_finished_event(fake):
    async def executor(**kwargs: Any) -> str:
        return "x" * 5000

    agent.default_registry.get("nook:web_fetch").executor = executor  # type: ignore[union-attr]

    result = await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    finished = fake.events[-1]
    assert isinstance(finished, ToolCallFinishedEvent)
    assert len(finished.result) == 2000
    assert len(result) == 5000


async def test_executor_exception_emits_finished_with_error_text_and_raises(fake):
    async def executor(**kwargs: Any) -> str:
        raise RuntimeError("boom")

    agent.default_registry.get("nook:web_fetch").executor = executor  # type: ignore[union-attr]

    runner = _create_registry_runner("nook:web_fetch")

    with pytest.raises(RuntimeError, match="boom"):
        await runner(url="https://example.com/")

    [_, started, finished] = fake.events
    assert isinstance(started, ToolCallStartedEvent)
    assert isinstance(finished, ToolCallFinishedEvent)
    assert finished.call_id == started.call_id
    assert finished.result == "Error: boom"


async def test_cancelled_executor_emits_no_finished_event(fake):
    async def executor(**kwargs: Any) -> str:
        raise asyncio.CancelledError

    agent.default_registry.get("nook:web_fetch").executor = executor  # type: ignore[union-attr]

    runner = _create_registry_runner("nook:web_fetch")

    with pytest.raises(asyncio.CancelledError):
        await runner(url="https://example.com/")

    assert [e.type for e in fake.events] == ["authorize_tool", "tool_call_started"]


async def test_denied_call_emits_no_tool_call_events(fake):
    fake.decision = "deny"

    await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    assert [e.type for e in fake.events] == ["authorize_tool"]


async def test_invalid_arguments_emit_no_events(fake):
    await _create_registry_runner("nook:web_fetch")(url="ftp://example.com/")

    assert fake.events == []
