import asyncio
import json
import sys
from typing import Any
from uuid import uuid4

write_lock = asyncio.Lock()
pending_authorizations: dict[tuple[str, str], asyncio.Future[dict[str, Any]]] = {}
pending_tool_results: dict[tuple[str, str], asyncio.Future[dict[str, Any]]] = {}
active_requests: dict[str, asyncio.Task[None]] = {}


async def write_event(event: dict[str, Any]) -> None:
    async with write_lock:
        sys.stdout.write(json.dumps(event) + "\n")
        sys.stdout.flush()


async def finish_request(
    request_id: str, message_id: str, status: str = "complete"
) -> None:
    await write_event(
        {
            "type": "message_finished",
            "request_id": request_id,
            "message_id": message_id,
            "status": status,
        }
    )


async def emit_text(request_id: str, message_id: str, text: str) -> None:
    await write_event(
        {
            "type": "text_delta",
            "request_id": request_id,
            "message_id": message_id,
            "text": text,
        }
    )


async def emit_error(request_id: str, text: str) -> None:
    await write_event(
        {
            "type": "error",
            "request_id": request_id,
            "error": {
                "code": "endpoint_error",
                "message": text.removeprefix("E2E_ERROR:").strip()
                or "Scripted E2E error",
                "retryable": True,
            },
        }
    )


def url_host(url: str) -> str:
    if url.startswith(("http://", "https://")):
        return url.split("/")[2].lower()
    return url


async def authorize_tool(
    request_id: str,
    chat_id: str,
    message_id: str,
    tool_name: str,
    arguments: dict[str, str],
) -> tuple[str, dict[str, Any]]:
    call_id = f"call_{uuid4().hex}"
    arguments_text = json.dumps(arguments)
    result: asyncio.Future[dict[str, Any]] = asyncio.get_running_loop().create_future()
    pending_authorizations[(request_id, call_id)] = result
    try:
        await write_event(
            {
                "type": "authorize_tool",
                "request_id": request_id,
                "message_id": message_id,
                "call_id": call_id,
                "chat_id": chat_id,
                "tool_name": tool_name,
                "arguments": arguments_text,
            }
        )
        return call_id, await result
    finally:
        pending_authorizations.pop((request_id, call_id), None)


async def execute_tool(
    request_id: str,
    chat_id: str,
    message_id: str,
    tool_name: str,
    arguments: dict[str, Any],
) -> dict[str, Any]:
    call_id = f"call_{uuid4().hex}"
    result: asyncio.Future[dict[str, Any]] = asyncio.get_running_loop().create_future()
    pending_tool_results[(request_id, call_id)] = result
    try:
        await write_event(
            {
                "type": "execute_tool",
                "request_id": request_id,
                "message_id": message_id,
                "call_id": call_id,
                "chat_id": chat_id,
                "tool_name": tool_name,
                "arguments": json.dumps(arguments),
            }
        )
        return await result
    finally:
        pending_tool_results.pop((request_id, call_id), None)


async def run_execute_prompt(
    request_id: str,
    chat_id: str,
    message_id: str,
    tool_name: str,
    arguments: dict[str, Any],
) -> None:
    result = await execute_tool(request_id, chat_id, message_id, tool_name, arguments)
    await emit_text(
        request_id, message_id, f"Tool {result['status']}: {result['output']}"
    )
    await finish_request(request_id, message_id)


async def run_tool_prompt(
    request_id: str,
    chat_id: str,
    message_id: str,
    tool_name: str,
    arguments: dict[str, str],
    subject: str,
) -> None:
    call_id, result = await authorize_tool(
        request_id, chat_id, message_id, tool_name, arguments
    )
    if result["decision"] != "allow":
        reason = result.get("reason")
        detail = f": {reason}" if reason else ""
        await emit_text(
            request_id,
            message_id,
            f"Approval denied for {url_host(subject)}{detail}",
        )
        await finish_request(request_id, message_id)
        return

    await write_event(
        {
            "type": "tool_call_started",
            "request_id": request_id,
            "message_id": message_id,
            "call_id": call_id,
            "name": tool_name,
            "arguments": json.dumps(arguments),
        }
    )
    await write_event(
        {
            "type": "tool_call_finished",
            "request_id": request_id,
            "message_id": message_id,
            "call_id": call_id,
            "result": f"Deterministic result for {subject}",
        }
    )
    for chunk in ("Approved ", "deterministic ", f"fetch for {subject}."):
        await emit_text(request_id, message_id, chunk)
    await finish_request(request_id, message_id)


async def run_message(request: dict[str, Any]) -> None:
    request_id = str(request["request_id"])
    chat_id = str(request["chat_id"])
    message_id = str(uuid4())
    text = str(request.get("text", ""))
    await write_event(
        {"type": "message_started", "request_id": request_id, "message_id": message_id}
    )

    try:
        if text.startswith("E2E_WAIT:"):
            await asyncio.sleep(5)
            text = text.removeprefix("E2E_WAIT:").strip() or "E2E delayed reply"
        if text.startswith("E2E_STALE:"):
            await emit_text(str(uuid4()), message_id, "STALE EVENT MUST NOT RENDER")
            text = (
                text.removeprefix("E2E_STALE:").strip()
                or "E2E response after stale event"
            )
        if text.startswith("E2E_ERROR:"):
            await emit_error(request_id, text)
            return
        if text.startswith("E2E_APPROVAL:"):
            url = text.removeprefix("E2E_APPROVAL:").strip()
            await run_tool_prompt(
                request_id, chat_id, message_id, "nook:web_fetch", {"url": url}, url
            )
            return
        if text.startswith("E2E_FILE_READ:"):
            path = text.removeprefix("E2E_FILE_READ:").strip()
            await run_execute_prompt(
                request_id,
                chat_id,
                message_id,
                "nook:read_local_file",
                {"path": path},
            )
            return
        if text.startswith("E2E_COMMAND:"):
            argv = text.removeprefix("E2E_COMMAND:").split()
            await run_execute_prompt(
                request_id,
                chat_id,
                message_id,
                "nook:run_command",
                {"argv": argv},
            )
            return
        if text.startswith("E2E_FILE_LIST:"):
            path = text.removeprefix("E2E_FILE_LIST:").strip()
            await run_execute_prompt(
                request_id,
                chat_id,
                message_id,
                "nook:list_local_directory",
                {"path": path},
            )
            return
        if text.startswith("E2E_FILE_WRITE:"):
            path, _, content = text.removeprefix("E2E_FILE_WRITE:").partition("|")
            await run_execute_prompt(
                request_id,
                chat_id,
                message_id,
                "nook:write_local_file",
                {"path": path.strip(), "content": content, "overwrite": False},
            )
            return
        if text.startswith("E2E_SEARCH:"):
            query = text.removeprefix("E2E_SEARCH:").strip()
            await run_tool_prompt(
                request_id,
                chat_id,
                message_id,
                "nook:web_search",
                {"query": query},
                query,
            )
            return

        reply = (
            text.removeprefix("E2E_REPLY:").strip()
            if text.startswith("E2E_REPLY:")
            else "E2E deterministic reply"
        )
        await emit_text(request_id, message_id, reply)
        await finish_request(request_id, message_id)
    except asyncio.CancelledError:
        await finish_request(request_id, message_id, "cancelled")
        raise


def handle_shutdown() -> bool:
    for task in active_requests.values():
        task.cancel()
    return False


def handle_run(request: dict[str, Any]) -> None:
    request_id = str(request["request_id"])
    task = asyncio.create_task(run_message(request))
    active_requests[request_id] = task
    task.add_done_callback(lambda _: active_requests.pop(request_id, None))


def handle_cancel(request: dict[str, Any]) -> None:
    task = active_requests.get(str(request.get("request_id", "")))
    if task and not task.done():
        task.cancel()


def handle_authorization_result(request: dict[str, Any]) -> None:
    key = (str(request.get("request_id", "")), str(request.get("call_id", "")))
    result = pending_authorizations.get(key)
    if result and not result.done():
        result.set_result(request)


def handle_tool_result(request: dict[str, Any]) -> None:
    key = (str(request.get("request_id", "")), str(request.get("call_id", "")))
    result = pending_tool_results.get(key)
    if result and not result.done():
        result.set_result(request)


async def handle_title(request: dict[str, Any]) -> None:
    await write_event(
        {
            "type": "title_ready",
            "request_id": str(request.get("request_id", "")),
            "title": "E2E test chat",
        }
    )


async def handle_delete_chat(request: dict[str, Any]) -> None:
    chat_id = str(request.get("chat_id", ""))
    await write_event(
        {
            "type": "deleted",
            "request_id": str(request.get("request_id", "")),
            "chat_id": chat_id,
        }
    )


async def handle_request(request: dict[str, Any]) -> None:
    request_type = str(request.get("type", ""))
    if request_type == "run":
        handle_run(request)
    elif request_type == "cancel":
        handle_cancel(request)
    elif request_type == "authorization_result":
        handle_authorization_result(request)
    elif request_type == "tool_result":
        handle_tool_result(request)
    elif request_type == "title":
        await handle_title(request)
    elif request_type == "delete_chat":
        await handle_delete_chat(request)


async def main() -> None:
    await write_event({"type": "ready", "version": "e2e"})
    while line := await asyncio.to_thread(sys.stdin.readline):
        try:
            request = json.loads(line)
        except (json.JSONDecodeError, KeyError):
            continue
        if request.get("type") == "shutdown":
            handle_shutdown()
            return
        await handle_request(request)


if __name__ == "__main__":
    asyncio.run(main())
