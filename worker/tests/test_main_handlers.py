import asyncio
from collections.abc import AsyncIterator
from uuid import UUID, uuid4

import pytest

from nook_worker import main
from nook_worker.authorization import AuthorizationDecision, AuthorizationManager
from nook_worker.fake_agent import FakeAgentRunner
from nook_worker.main import run_worker
from nook_worker.protocol import (
    AuthorizationResultRequest,
    AuthorizeToolEvent,
    DeleteChatRequest,
    DeletedEvent,
    EventMessage,
    ReadyEvent,
    RunRequest,
    TitleReadyEvent,
    TitleRequest,
    parse_event,
)


@pytest.mark.asyncio
async def test_main_handles_title_and_delete_requests():
    req_title_id = uuid4()
    req_delete_id = uuid4()
    chat_id = uuid4()

    title_req = TitleRequest(
        request_id=req_title_id,
        chat_id=chat_id,
        first_message="How do checkpointers work?",
    )
    delete_req = DeleteChatRequest(
        request_id=req_delete_id,
        chat_id=chat_id,
    )

    stdin_reader = asyncio.StreamReader()
    output_lines: list[str] = []

    async def mock_write_line(line: str) -> None:
        output_lines.append(line)

    async def mock_title_fn(text: str) -> str:
        return "Checkpointers Explained"

    async def mock_delete_fn(c_id: str) -> None:
        pass

    worker_task = asyncio.create_task(
        run_worker(
            stdin_reader=stdin_reader,
            write_line=mock_write_line,
            runner=FakeAgentRunner(),
            title_handler=mock_title_fn,
            delete_handler=mock_delete_fn,
        )
    )

    stdin_reader.feed_data(f"{title_req.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)
    stdin_reader.feed_data(f"{delete_req.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)
    stdin_reader.feed_eof()

    await worker_task

    assert len(output_lines) == 3
    ev_ready = parse_event(output_lines[0])
    assert isinstance(ev_ready, ReadyEvent)

    ev_title = parse_event(output_lines[1])
    assert isinstance(ev_title, TitleReadyEvent)
    assert ev_title.title == "Checkpointers Explained"
    assert ev_title.request_id == req_title_id

    ev_del = parse_event(output_lines[2])
    assert isinstance(ev_del, DeletedEvent)
    assert ev_del.chat_id == chat_id
    assert ev_del.request_id == req_delete_id


@pytest.mark.asyncio
async def test_main_dispatches_authorization_result_to_waiting_call(
    monkeypatch: pytest.MonkeyPatch,
):
    manager = AuthorizationManager()
    monkeypatch.setattr(main, "default_authorization_manager", manager)
    run_request = RunRequest(request_id=uuid4(), chat_id=uuid4(), text="hi")
    stdin_reader = asyncio.StreamReader()
    output_lines: list[str] = []
    decisions: list[AuthorizationDecision] = []

    async def mock_write_line(line: str) -> None:
        output_lines.append(line)

    class AuthorizingRunner:
        async def run(
            self, request: RunRequest, message_id: UUID | None = None
        ) -> AsyncIterator[EventMessage]:
            decisions.append(await manager.authorize("nook:web_fetch", "{}"))
            return
            yield

    worker_task = asyncio.create_task(
        run_worker(
            stdin_reader=stdin_reader,
            write_line=mock_write_line,
            runner=AuthorizingRunner(),
        )
    )
    stdin_reader.feed_data(f"{run_request.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)

    event = parse_event(output_lines[1])
    assert isinstance(event, AuthorizeToolEvent)
    result = AuthorizationResultRequest(
        request_id=event.request_id,
        call_id=event.call_id,
        decision="deny",
        reason="not allowed",
    )
    stdin_reader.feed_data(f"{result.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)
    stdin_reader.feed_eof()
    await worker_task

    assert decisions == [AuthorizationDecision(allowed=False, reason="not allowed")]


@pytest.mark.asyncio
async def test_main_ignores_authorization_result_for_unknown_call(
    monkeypatch: pytest.MonkeyPatch,
):
    manager = AuthorizationManager()
    monkeypatch.setattr(main, "default_authorization_manager", manager)
    stdin_reader = asyncio.StreamReader()
    output_lines: list[str] = []

    async def mock_write_line(line: str) -> None:
        output_lines.append(line)

    worker_task = asyncio.create_task(
        run_worker(
            stdin_reader=stdin_reader,
            write_line=mock_write_line,
            runner=FakeAgentRunner(),
        )
    )
    stray = AuthorizationResultRequest(
        request_id=uuid4(), call_id="call_unknown", decision="allow"
    )
    stdin_reader.feed_data(f"{stray.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)
    stdin_reader.feed_eof()
    await worker_task

    assert len(output_lines) == 1
