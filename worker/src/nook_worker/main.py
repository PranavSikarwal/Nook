import asyncio
import sys
from collections.abc import Awaitable, Callable, Coroutine
from typing import Any
from uuid import UUID, uuid4

from nook_worker import __version__
from nook_worker.agent import (
    AgentRunner,
    RealAgentRunner,
    build_deep_agent,
    create_model,
    map_exception_to_error_info,
)
from nook_worker.approval import default_approval_manager
from nook_worker.checkpointer import (
    create_pool,
    delete_thread_memory,
    setup_checkpointer,
)
from nook_worker.config import WorkerConfig
from nook_worker.protocol import (
    ApprovalDecisionRequest,
    CancelRequest,
    DeleteChatRequest,
    DeletedEvent,
    ErrorEvent,
    ErrorInfo,
    MessageFinishedEvent,
    MessageStartedEvent,
    ReadyEvent,
    RequestMessage,
    RunRequest,
    ShutdownRequest,
    TitleReadyEvent,
    TitleRequest,
    format_event,
    parse_request,
)
from nook_worker.titles import generate_title


async def handle_run(
    request: RunRequest,
    runner: AgentRunner,
    write_line: Callable[[str], Awaitable[None]],
) -> None:
    message_id = uuid4()
    default_approval_manager.set_context(
        request_id=request.request_id,
        message_id=message_id,
        chat_id=request.chat_id,
        write_line=write_line,
    )
    try:
        async for event in runner.run(request, message_id=message_id):
            if isinstance(event, MessageStartedEvent):
                message_id = event.message_id
                default_approval_manager.set_context(
                    request_id=request.request_id,
                    message_id=message_id,
                    chat_id=request.chat_id,
                    write_line=write_line,
                )
            await write_line(format_event(event))
    except asyncio.CancelledError:
        default_approval_manager.cancel_for_request(request.request_id)
        finished_event = MessageFinishedEvent(
            request_id=request.request_id,
            message_id=message_id,
            status="cancelled",
        )
        await write_line(format_event(finished_event))
        raise
    except Exception as err:  # noqa: BLE001
        default_approval_manager.cancel_for_request(request.request_id)
        err_info = map_exception_to_error_info(err)
        err_event = ErrorEvent(
            request_id=request.request_id,
            error=err_info,
        )
        await write_line(format_event(err_event))
    finally:
        default_approval_manager.clear_context(request.request_id)


async def handle_title(
    request: TitleRequest,
    title_handler: Callable[[str], Awaitable[str]],
    write_line: Callable[[str], Awaitable[None]],
) -> None:
    try:
        title = await title_handler(request.first_message)
        event = TitleReadyEvent(
            request_id=request.request_id,
            title=title,
        )
        await write_line(format_event(event))
    except Exception as err:  # noqa: BLE001
        err_info = map_exception_to_error_info(err)
        err_event = ErrorEvent(
            request_id=request.request_id,
            error=err_info,
        )
        await write_line(format_event(err_event))


async def handle_delete(
    request: DeleteChatRequest,
    delete_handler: Callable[[str], Awaitable[None]],
    write_line: Callable[[str], Awaitable[None]],
) -> None:
    try:
        await delete_handler(str(request.chat_id))
        event = DeletedEvent(
            request_id=request.request_id,
            chat_id=request.chat_id,
        )
        await write_line(format_event(event))
    except Exception as err:  # noqa: BLE001
        err_info = map_exception_to_error_info(err)
        err_event = ErrorEvent(
            request_id=request.request_id,
            error=err_info,
        )
        await write_line(format_event(err_event))


def _spawn_tracked_task(
    coro: Coroutine[Any, Any, None],
    req_id: UUID,
    active_tasks: dict[UUID, asyncio.Task[None]],
) -> None:
    old_task = active_tasks.get(req_id)
    if old_task and not old_task.done():
        old_task.cancel()

    task = asyncio.create_task(coro)
    active_tasks[req_id] = task

    def cleanup(_: asyncio.Task[None]) -> None:
        if active_tasks.get(req_id) is task:
            active_tasks.pop(req_id, None)

    task.add_done_callback(cleanup)


async def _send_error(
    write_line: Callable[[str], Awaitable[None]],
    req_id: UUID,
    message: str,
) -> None:
    err = ErrorEvent(
        request_id=req_id,
        error=ErrorInfo(
            code="internal",
            message=message,
            retryable=False,
        ),
    )
    await write_line(format_event(err))


def _dispatch_request(
    req: RequestMessage,
    runner: AgentRunner,
    write_line: Callable[[str], Awaitable[None]],
    active_tasks: dict[UUID, asyncio.Task[None]],
    title_handler: Callable[[str], Awaitable[str]] | None,
    delete_handler: Callable[[str], Awaitable[None]] | None,
) -> bool:
    match req:
        case ShutdownRequest():
            return False
        case CancelRequest(request_id=req_id):
            default_approval_manager.cancel_for_request(req_id)
            active_task = active_tasks.get(req_id)
            if active_task and not active_task.done():
                active_task.cancel()
        case ApprovalDecisionRequest(call_id=call_id, action=action):
            default_approval_manager.resolve_decision(call_id, action)
        case RunRequest(request_id=req_id):
            _spawn_tracked_task(
                handle_run(req, runner, write_line), req_id, active_tasks
            )
        case TitleRequest(request_id=req_id):
            if title_handler is not None:
                _spawn_tracked_task(
                    handle_title(req, title_handler, write_line),
                    req_id,
                    active_tasks,
                )
            else:
                _spawn_tracked_task(
                    _send_error(write_line, req_id, "Title handler not configured"),
                    req_id,
                    active_tasks,
                )
        case DeleteChatRequest(request_id=req_id):
            if delete_handler is not None:
                _spawn_tracked_task(
                    handle_delete(req, delete_handler, write_line),
                    req_id,
                    active_tasks,
                )
            else:
                _spawn_tracked_task(
                    _send_error(write_line, req_id, "Delete handler not configured"),
                    req_id,
                    active_tasks,
                )
    return True


async def run_worker(
    stdin_reader: Any,
    write_line: Callable[[str], Awaitable[None]],
    runner: AgentRunner,
    title_handler: Callable[[str], Awaitable[str]] | None = None,
    delete_handler: Callable[[str], Awaitable[None]] | None = None,
) -> None:
    ready = ReadyEvent(version=__version__)
    await write_line(format_event(ready))

    active_tasks: dict[UUID, asyncio.Task[None]] = {}

    while True:
        line_bytes = await stdin_reader.readline()
        if not line_bytes:
            break

        line_str = line_bytes.decode("utf-8").strip()
        if not line_str:
            continue

        try:
            req = parse_request(line_str)
        except ValueError as err:
            sys.stderr.write(f"Worker received unparseable line: {err}\n")
            sys.stderr.flush()
            continue

        should_continue = _dispatch_request(
            req,
            runner,
            write_line,
            active_tasks,
            title_handler,
            delete_handler,
        )
        if not should_continue:
            for task in active_tasks.values():
                task.cancel()
            break

    if active_tasks:
        await asyncio.gather(*active_tasks.values(), return_exceptions=True)


def _write_stdout(line: str) -> None:
    sys.stdout.write(line + "\n")
    sys.stdout.flush()


class _StdinReader:
    def __init__(self, is_tty: bool, reader: asyncio.StreamReader) -> None:
        self._is_tty = is_tty
        self._reader = reader

    async def readline(self) -> bytes:
        if self._is_tty:
            line_str = await asyncio.to_thread(sys.stdin.readline)
            return line_str.encode("utf-8")
        return await self._reader.readline()


async def _async_main() -> None:
    is_tty = sys.stdin.isatty()
    reader = asyncio.StreamReader()
    if not is_tty:
        loop = asyncio.get_running_loop()
        protocol = asyncio.StreamReaderProtocol(reader)
        await loop.connect_read_pipe(lambda: protocol, sys.stdin)

    stdin_reader = _StdinReader(is_tty, reader)
    stdout_lock = asyncio.Lock()

    async def stdout_writer(line: str) -> None:
        async with stdout_lock:
            await asyncio.to_thread(_write_stdout, line)

    config = WorkerConfig()
    pool = create_pool(config.database_url)
    async with pool:
        checkpointer = await setup_checkpointer(pool)
        model = create_model(config)
        agent = build_deep_agent(model, config, checkpointer)
        runner = RealAgentRunner(agent)

        async def title_fn(first_message: str) -> str:
            return await generate_title(model, first_message)

        async def delete_fn(chat_id: str) -> None:
            await delete_thread_memory(checkpointer, chat_id)

        await run_worker(
            stdin_reader=stdin_reader,
            write_line=stdout_writer,
            runner=runner,
            title_handler=title_fn,
            delete_handler=delete_fn,
        )


def main() -> None:
    asyncio.run(_async_main())


if __name__ == "__main__":
    main()
