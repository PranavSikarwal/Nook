import asyncio
import json
from typing import Any
from uuid import uuid4

import pytest

from nook_worker import authorization, remote_tools
from nook_worker.agent import RealAgentRunner
from nook_worker.authorization import AuthorizationManager, ToolOutcome
from nook_worker.main import _dispatch_request
from nook_worker.protocol import (
    ExecuteToolEvent,
    ToolCatalogRequest,
    ToolDescriptor,
    ToolResultRequest,
    parse_event,
    parse_request,
)

READ_TOOL = ToolDescriptor(
    id="nook:read_local_file",
    name="read_local_file",
    description="Read a text file inside an allowed folder.",
    input_schema={
        "type": "object",
        "properties": {"path": {"type": "string"}},
        "required": ["path"],
        "additionalProperties": False,
    },
)


class Run:
    def __init__(self, manager: AuthorizationManager) -> None:
        self.manager = manager
        self.request_id = uuid4()
        self.message_id = uuid4()
        self.chat_id = uuid4()
        self.lines: list[str] = []
        manager.set_context(
            request_id=self.request_id,
            message_id=self.message_id,
            chat_id=self.chat_id,
            write_line=self.write_line,
        )

    async def write_line(self, line: str) -> None:
        self.lines.append(line)

    def events(self) -> list[ExecuteToolEvent]:
        parsed = [parse_event(line) for line in self.lines]
        assert all(isinstance(event, ExecuteToolEvent) for event in parsed)
        return [e for e in parsed if isinstance(e, ExecuteToolEvent)]


async def settle() -> None:
    await asyncio.sleep(0.01)


async def test_execute_sends_the_call_and_returns_the_daemon_output():
    run = Run(AuthorizationManager())

    task = asyncio.create_task(
        run.manager.execute("nook:read_local_file", '{"path": "/work/a.txt"}')
    )
    await settle()

    [event] = run.events()
    assert event.request_id == run.request_id
    assert event.chat_id == run.chat_id
    assert event.tool_name == "nook:read_local_file"
    assert event.arguments == '{"path": "/work/a.txt"}'
    assert run.manager.resolve_tool_result(
        run.request_id, event.call_id, "ok", "file text"
    )
    assert await task == ToolOutcome(status="ok", output="file text")
    assert not run.manager.was_denied(run.request_id)


async def test_a_denied_result_marks_the_reply_denied():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.execute("nook:read_local_file", "{}"))
    await settle()
    [event] = run.events()

    run.manager.resolve_tool_result(
        run.request_id, event.call_id, "denied", "Denied by the user"
    )

    assert await task == ToolOutcome(status="denied", output="Denied by the user")
    assert run.manager.was_denied(run.request_id)


async def test_an_error_result_does_not_deny_the_reply():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.execute("nook:read_local_file", "{}"))
    await settle()
    [event] = run.events()

    run.manager.resolve_tool_result(
        run.request_id, event.call_id, "error", "Error: file not found"
    )

    assert (await task).status == "error"
    assert not run.manager.was_denied(run.request_id)


async def test_a_result_for_another_call_or_request_is_ignored():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.execute("nook:read_local_file", "{}"))
    await settle()
    [event] = run.events()

    assert not run.manager.resolve_tool_result(run.request_id, "call_other", "ok", "x")
    assert not run.manager.resolve_tool_result(uuid4(), event.call_id, "ok", "x")
    assert not task.done()
    run.manager.cancel_for_request(run.request_id)
    with pytest.raises(asyncio.CancelledError):
        await task


async def test_a_second_result_for_the_same_call_is_ignored():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.execute("nook:read_local_file", "{}"))
    await settle()
    [event] = run.events()

    assert run.manager.resolve_tool_result(run.request_id, event.call_id, "ok", "one")
    assert not run.manager.resolve_tool_result(
        run.request_id, event.call_id, "ok", "two"
    )
    assert (await task).output == "one"


async def test_execute_times_out_as_a_denial():
    run = Run(AuthorizationManager(execution_timeout=0.02))

    outcome = await run.manager.execute("nook:read_local_file", "{}")

    assert outcome.status == "denied"
    assert run.manager.was_denied(run.request_id)


async def test_execute_without_a_run_is_denied():
    outcome = await AuthorizationManager().execute("nook:read_local_file", "{}")

    assert outcome.status == "denied"


async def test_the_remote_tool_checks_arguments_before_asking_the_daemon(monkeypatch):
    manager = AuthorizationManager()
    run = Run(manager)
    tool = remote_tools.build_remote_tool(READ_TOOL)
    monkeypatch.setattr(remote_tools, "default_authorization_manager", manager)

    result = await tool.ainvoke({"path": 5})

    assert result.startswith("Error: invalid arguments")
    assert run.lines == []


async def test_the_remote_tool_returns_output_and_reports_denials(monkeypatch):
    manager = AuthorizationManager()
    run = Run(manager)
    tool = remote_tools.build_remote_tool(READ_TOOL)
    monkeypatch.setattr(remote_tools, "default_authorization_manager", manager)

    task = asyncio.create_task(tool.ainvoke({"path": "/work/a.txt"}))
    await settle()
    [event] = run.events()
    assert json.loads(event.arguments) == {"path": "/work/a.txt"}
    manager.resolve_tool_result(run.request_id, event.call_id, "ok", "contents")
    assert await task == "contents"

    task = asyncio.create_task(tool.ainvoke({"path": "/work/b.txt"}))
    await settle()
    event = run.events()[-1]
    manager.resolve_tool_result(
        run.request_id, event.call_id, "denied", "Denied by the user"
    )
    assert await task == "Error: request denied: Denied by the user"


def test_the_catalog_builds_one_tool_for_each_descriptor():
    other = READ_TOOL.model_copy(
        update={"id": "nook:list_local_directory", "name": "list_local_directory"}
    )

    tools = remote_tools.build_remote_tools([READ_TOOL, other])

    assert [tool.name for tool in tools] == ["read_local_file", "list_local_directory"]
    assert tools[0].description == READ_TOOL.description


def test_a_catalog_request_rebuilds_the_agent():
    built: list[list[str]] = []

    def rebuild(tools: list[Any]) -> str:
        built.append([tool.name for tool in tools])
        return "rebuilt-agent"

    runner = RealAgentRunner("first-agent", rebuild=rebuild)
    request = parse_request(ToolCatalogRequest(tools=[READ_TOOL]).model_dump_json())

    assert isinstance(request, ToolCatalogRequest)
    _dispatch_request(request, runner, _unused_writer, {}, None, None)

    assert built == [["read_local_file"]]


def test_an_empty_catalog_removes_every_remote_tool():
    built: list[list[str]] = []

    def rebuild(tools: list[Any]) -> str:
        built.append([tool.name for tool in tools])
        return "rebuilt-agent"

    runner = RealAgentRunner("first-agent", rebuild=rebuild)
    request = parse_request(ToolCatalogRequest(tools=[]).model_dump_json())

    assert isinstance(request, ToolCatalogRequest)
    _dispatch_request(request, runner, _unused_writer, {}, None, None)

    assert built == [[]]


async def _unused_writer(line: str) -> None:
    raise AssertionError(f"Unexpected write: {line}")


async def test_a_tool_result_request_resolves_the_waiting_call():
    manager = authorization.default_authorization_manager
    run = Run(manager)
    task = asyncio.create_task(manager.execute("nook:read_local_file", "{}"))
    await settle()
    [event] = run.events()
    request = parse_request(
        ToolResultRequest(
            request_id=run.request_id,
            call_id=event.call_id,
            status="ok",
            output="done",
        ).model_dump_json()
    )

    _dispatch_request(request, FakeRunner(), _unused_writer, {}, None, None)

    assert await task == ToolOutcome(status="ok", output="done")
    manager.clear_context(run.request_id)


class FakeRunner:
    def run(self, request: Any, message_id: Any = None) -> Any:
        raise AssertionError("Unexpected run")
