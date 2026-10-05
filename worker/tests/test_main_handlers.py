import asyncio
from uuid import uuid4

import pytest

from nook_worker.fake_agent import FakeAgentRunner
from nook_worker.main import run_worker
from nook_worker.protocol import (
    DeleteChatRequest,
    DeletedEvent,
    ReadyEvent,
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
