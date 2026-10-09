import asyncio
import json
import sys
from typing import Any
from uuid import uuid4

write_lock = asyncio.Lock()
pending_approvals: dict[str, dict[str, str]] = {}
active_requests: dict[str, asyncio.Task[None]] = {}
approved_hosts: set[tuple[str, str]] = set()


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
            await write_event(
                {
                    "type": "text_delta",
                    "request_id": str(uuid4()),
                    "message_id": message_id,
                    "text": "STALE EVENT MUST NOT RENDER",
                }
            )
            text = (
                text.removeprefix("E2E_STALE:").strip()
                or "E2E response after stale event"
            )

        if text.startswith("E2E_ERROR:"):
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

            return

        if text.startswith("E2E_APPROVAL:"):
            url = text.removeprefix("E2E_APPROVAL:").strip()
            host = (
                url.split("/")[2].lower()
                if url.startswith(("http://", "https://"))
                else url
            )
            if (chat_id, host) in approved_hosts or (chat_id, "*") in approved_hosts:
                await write_event(
                    {
                        "type": "text_delta",
                        "request_id": request_id,
                        "message_id": message_id,
                        "text": f"Previously approved {host}",
                    }
                )
                await finish_request(request_id, message_id)
                return

            call_id = f"call_{uuid4().hex}"
            pending_approvals[call_id] = {
                "request_id": request_id,
                "chat_id": chat_id,
                "message_id": message_id,
                "host": host,
                "url": url,
            }
            await write_event(
                {
                    "type": "approval_requested",
                    "request_id": request_id,
                    "message_id": message_id,
                    "call_id": call_id,
                    "tool_name": "nook:web_fetch",
                    "arguments": json.dumps({"url": url}),
                    "explanation": "Deterministic test approval",
                    "resource_summary": url,
                    "actions": [
                        "allow_once",
                        "allow_for_chat_host",
                        "allow_for_chat",
                        "deny",
                    ],
                }
            )
            return

        reply = (
            text.removeprefix("E2E_REPLY:").strip()
            if text.startswith("E2E_REPLY:")
            else "E2E deterministic reply"
        )
        await write_event(
            {
                "type": "text_delta",
                "request_id": request_id,
                "message_id": message_id,
                "text": reply,
            }
        )
        await finish_request(request_id, message_id)
    except asyncio.CancelledError:
        await finish_request(request_id, message_id, "cancelled")
        raise


async def handle_request(request: dict[str, Any]) -> bool:
    request_type = request.get("type")
    request_id = str(request.get("request_id", ""))

    if request_type == "shutdown":
        for task in active_requests.values():
            task.cancel()
        return False

    if request_type == "run":
        task = asyncio.create_task(run_message(request))
        active_requests[request_id] = task
        task.add_done_callback(lambda _: active_requests.pop(request_id, None))
        return True

    if request_type == "cancel":
        target_id = str(request.get("target_id", ""))
        for call_id, pending in list(pending_approvals.items()):
            if pending["request_id"] == target_id:
                pending_approvals.pop(call_id, None)
        task = active_requests.get(target_id)
        if task and not task.done():
            task.cancel()
        return True

    if request_type == "approval_decision":
        call_id = str(request.get("call_id", ""))
        approval = pending_approvals.pop(call_id, None)
        if approval:
            action = str(request.get("action", "deny"))
            if action == "allow_for_chat_host":
                approved_hosts.add((approval["chat_id"], approval["host"]))
            elif action == "allow_for_chat":
                approved_hosts.add((approval["chat_id"], "*"))
            response = (
                f"Approval denied for {approval['host']}"
                if action == "deny"
                else f"Approved deterministic fetch for {approval['url']}."
            )
            await write_event(
                {
                    "type": "text_delta",
                    "request_id": approval["request_id"],
                    "message_id": approval["message_id"],
                    "text": response,
                }
            )
            await finish_request(approval["request_id"], approval["message_id"])
        return True

    if request_type == "title":
        await write_event(
            {"type": "title_ready", "request_id": request_id, "title": "E2E test chat"}
        )
    elif request_type == "delete_chat":
        chat_id = str(request.get("chat_id", ""))
        approved_hosts.difference_update(
            (grant_chat, host)
            for grant_chat, host in approved_hosts
            if grant_chat == chat_id
        )
        await write_event(
            {
                "type": "deleted",
                "request_id": request_id,
                "chat_id": chat_id,
            }
        )
    return True


async def main() -> None:
    await write_event({"type": "ready", "version": "e2e"})
    while line := await asyncio.to_thread(sys.stdin.readline):
        try:
            should_continue = await handle_request(json.loads(line))
        except (json.JSONDecodeError, KeyError, ValueError):
            continue
        if not should_continue:
            return


if __name__ == "__main__":
    asyncio.run(main())
