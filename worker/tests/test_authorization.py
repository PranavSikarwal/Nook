import asyncio
from uuid import UUID, uuid4

import pytest

from nook_worker.authorization import AuthorizationDecision, AuthorizationManager
from nook_worker.protocol import AuthorizeToolEvent, parse_event


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

    def events(self) -> list[AuthorizeToolEvent]:
        parsed = [parse_event(line) for line in self.lines]
        assert all(isinstance(event, AuthorizeToolEvent) for event in parsed)
        return [e for e in parsed if isinstance(e, AuthorizeToolEvent)]


async def settle() -> None:
    await asyncio.sleep(0.01)


async def test_authorize_emits_event_and_returns_allow_on_matching_result():
    run = Run(AuthorizationManager())

    task = asyncio.create_task(
        run.manager.authorize("nook:web_fetch", '{"url": "https://example.com"}')
    )
    await settle()

    [event] = run.events()
    assert event.request_id == run.request_id
    assert event.message_id == run.message_id
    assert event.chat_id == run.chat_id
    assert event.tool_name == "nook:web_fetch"
    assert event.arguments == '{"url": "https://example.com"}'
    assert event.call_id.startswith("call_")

    assert run.manager.resolve(run.request_id, event.call_id, "allow") is True
    assert await task == AuthorizationDecision(allowed=True)


async def test_deny_result_carries_reason():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.authorize("nook:web_fetch", "{}"))
    await settle()
    [event] = run.events()

    run.manager.resolve(run.request_id, event.call_id, "deny", "blocked by policy")

    assert await task == AuthorizationDecision(
        allowed=False, reason="blocked by policy"
    )


async def test_call_ids_are_unique_per_call():
    run = Run(AuthorizationManager(authorization_timeout=0.02))

    await run.manager.authorize("nook:web_fetch", "{}")
    await run.manager.authorize("nook:web_fetch", "{}")

    first, second = run.events()
    assert first.call_id != second.call_id


async def test_result_with_wrong_call_id_does_not_resolve():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.authorize("nook:web_fetch", "{}"))
    await settle()
    [event] = run.events()

    assert run.manager.resolve(run.request_id, "call_other", "allow") is False
    await settle()
    assert not task.done()

    run.manager.resolve(run.request_id, event.call_id, "deny")
    assert (await task).allowed is False


async def test_result_with_wrong_request_id_does_not_resolve():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.authorize("nook:web_fetch", "{}"))
    await settle()
    [event] = run.events()

    assert run.manager.resolve(uuid4(), event.call_id, "allow") is False
    await settle()
    assert not task.done()

    run.manager.resolve(run.request_id, event.call_id, "allow")
    assert (await task).allowed is True


async def test_result_for_unknown_call_is_ignored():
    manager = AuthorizationManager()

    assert manager.resolve(uuid4(), "call_unknown", "allow") is False


async def test_result_cannot_resolve_the_same_call_twice():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.authorize("nook:web_fetch", "{}"))
    await settle()
    [event] = run.events()

    assert run.manager.resolve(run.request_id, event.call_id, "allow") is True
    assert run.manager.resolve(run.request_id, event.call_id, "deny") is False
    assert (await task).allowed is True


async def test_cancel_for_request_unblocks_waiters():
    run = Run(AuthorizationManager())
    task = asyncio.create_task(run.manager.authorize("nook:web_fetch", "{}"))
    await settle()

    run.manager.cancel_for_request(run.request_id)

    with pytest.raises(asyncio.CancelledError):
        await task


async def test_cancel_for_request_leaves_other_requests_waiting():
    manager = AuthorizationManager()
    first = Run(manager)
    first_task = asyncio.create_task(manager.authorize("nook:web_fetch", "{}"))
    await settle()
    [first_event] = first.events()

    async def second_run() -> AuthorizationDecision:
        second = Run(manager)
        second_ids.append(second.request_id)
        return await manager.authorize("nook:web_search", "{}")

    second_ids: list[UUID] = []
    second_task = asyncio.create_task(second_run())
    await settle()

    manager.cancel_for_request(second_ids[0])

    with pytest.raises(asyncio.CancelledError):
        await second_task
    assert not first_task.done()
    manager.resolve(first.request_id, first_event.call_id, "allow")
    assert (await first_task).allowed is True


async def test_timeout_denies():
    run = Run(AuthorizationManager(authorization_timeout=0.05))

    decision = await run.manager.authorize("nook:web_fetch", "{}")

    assert decision.allowed is False
    [event] = run.events()
    assert run.manager.resolve(run.request_id, event.call_id, "allow") is False


async def test_missing_context_denies_without_emitting():
    manager = AuthorizationManager()
    manager.clear_context()

    decision = await manager.authorize("nook:web_fetch", "{}")

    assert decision.allowed is False


async def test_write_failure_leaves_no_waiting_call():
    manager = AuthorizationManager()
    sent: list[str] = []

    async def failing_write_line(line: str) -> None:
        sent.append(line)
        raise OSError("Broken pipe")

    request_id = uuid4()
    manager.set_context(request_id, uuid4(), uuid4(), failing_write_line)

    with pytest.raises(OSError, match="Broken pipe"):
        await manager.authorize("nook:web_fetch", "{}")

    event = parse_event(sent[0])
    assert isinstance(event, AuthorizeToolEvent)
    assert manager.resolve(request_id, event.call_id, "allow") is False


async def test_concurrent_calls_in_one_run_resolve_independently():
    run = Run(AuthorizationManager())
    fetch = asyncio.create_task(run.manager.authorize("nook:web_fetch", "{}"))
    search = asyncio.create_task(run.manager.authorize("nook:web_search", "{}"))
    await settle()
    by_tool = {event.tool_name: event for event in run.events()}

    run.manager.resolve(run.request_id, by_tool["nook:web_search"].call_id, "deny")
    await settle()
    assert search.done() and not fetch.done()

    run.manager.resolve(run.request_id, by_tool["nook:web_fetch"].call_id, "allow")
    assert (await fetch).allowed is True
    assert (await search).allowed is False


async def test_concurrent_runs_keep_separate_contexts():
    manager = AuthorizationManager()
    runs: dict[str, Run] = {}

    async def run_tool(name: str) -> AuthorizationDecision:
        runs[name] = Run(manager)
        return await manager.authorize(f"nook:{name}", "{}")

    first = asyncio.create_task(run_tool("web_fetch"))
    second = asyncio.create_task(run_tool("web_search"))
    await settle()

    assert [e.request_id for e in runs["web_fetch"].events()] == [
        runs["web_fetch"].request_id
    ]
    assert [e.request_id for e in runs["web_search"].events()] == [
        runs["web_search"].request_id
    ]
    manager.cancel_all()
    for task in (first, second):
        with pytest.raises(asyncio.CancelledError):
            await task
