import asyncio
from collections.abc import AsyncIterator
from uuid import UUID, uuid4

import pytest
from nook_worker.fake_agent import FakeAgentRunner
from nook_worker.main import run_worker
from nook_worker.protocol import (
    CancelRequest,
    EventMessage,
    MessageFinishedEvent,
    MessageStartedEvent,
    ReadyEvent,
    RunRequest,
    TextDeltaEvent,
    parse_event,
)


@pytest.mark.asyncio
async def test_fake_agent_runner_yields_scripted_events():
    runner = FakeAgentRunner(scripted_text=["Hello, ", "this is ", "Nook."])
    request = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="Hello",
    )

    events = [event async for event in runner.run(request)]

    assert len(events) == 5
    assert isinstance(events[0], MessageStartedEvent)
    assert events[0].request_id == request.request_id

    assert isinstance(events[1], TextDeltaEvent)
    assert events[1].text == "Hello, "

    assert isinstance(events[2], TextDeltaEvent)
    assert events[2].text == "this is "

    assert isinstance(events[3], TextDeltaEvent)
    assert events[3].text == "Nook."

    assert isinstance(events[4], MessageFinishedEvent)
    assert events[4].status == "complete"
    assert events[4].request_id == request.request_id


@pytest.mark.asyncio
async def test_run_worker_stdin_stdout_stream():
    req_id = uuid4()
    chat_id = uuid4()
    run_req = RunRequest(
        request_id=req_id,
        chat_id=chat_id,
        text="Hi",
    )

    input_lines = f"{run_req.model_dump_json()}\n"
    stdin_reader = asyncio.StreamReader()
    stdin_reader.feed_data(input_lines.encode("utf-8"))
    stdin_reader.feed_eof()

    output_lines: list[str] = []

    async def mock_write_line(line: str) -> None:
        output_lines.append(line)

    runner = FakeAgentRunner(scripted_text=["Test reply."])
    await run_worker(
        stdin_reader=stdin_reader,
        write_line=mock_write_line,
        runner=runner,
    )

    assert len(output_lines) == 4
    ev_ready = parse_event(output_lines[0])
    assert isinstance(ev_ready, ReadyEvent)
    assert ev_ready.version == "0.1.0"

    ev_started = parse_event(output_lines[1])
    assert isinstance(ev_started, MessageStartedEvent)
    assert ev_started.request_id == req_id

    ev_delta = parse_event(output_lines[2])
    assert isinstance(ev_delta, TextDeltaEvent)
    assert ev_delta.text == "Test reply."

    ev_finished = parse_event(output_lines[3])
    assert isinstance(ev_finished, MessageFinishedEvent)
    assert ev_finished.status == "complete"


@pytest.mark.asyncio
async def test_run_worker_cancel():
    req_id = uuid4()
    chat_id = uuid4()
    run_req = RunRequest(
        request_id=req_id,
        chat_id=chat_id,
        text="Hi",
    )
    cancel_req = CancelRequest(request_id=req_id)

    stdin_reader = asyncio.StreamReader()

    output_lines: list[str] = []

    async def mock_write_line(line: str) -> None:
        output_lines.append(line)

    class SlowRunner:
        async def run(
            self, request: RunRequest, message_id: UUID | None = None
        ) -> AsyncIterator[EventMessage]:
            msg_id = message_id or uuid4()
            yield MessageStartedEvent(
                request_id=request.request_id,
                message_id=msg_id,
            )
            await asyncio.sleep(1.0)
            yield TextDeltaEvent(
                request_id=request.request_id,
                message_id=msg_id,
                text="Never reached",
            )

    worker_task = asyncio.create_task(
        run_worker(
            stdin_reader=stdin_reader,
            write_line=mock_write_line,
            runner=SlowRunner(),
        )
    )

    stdin_reader.feed_data(f"{run_req.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)
    stdin_reader.feed_data(f"{cancel_req.model_dump_json()}\n".encode())
    await asyncio.sleep(0.02)
    stdin_reader.feed_eof()

    await worker_task

    assert len(output_lines) >= 3
    ev_ready = parse_event(output_lines[0])
    assert isinstance(ev_ready, ReadyEvent)

    ev_started = parse_event(output_lines[1])
    assert isinstance(ev_started, MessageStartedEvent)

    ev_finished = parse_event(output_lines[2])
    assert isinstance(ev_finished, MessageFinishedEvent)
    assert ev_finished.status == "cancelled"
