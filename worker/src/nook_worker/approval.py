import asyncio
from collections.abc import Awaitable, Callable
from contextvars import ContextVar
from dataclasses import dataclass
from uuid import UUID

from nook_worker.protocol import ApprovalRequestedEvent, format_event


@dataclass(frozen=True)
class RunApprovalContext:
    request_id: UUID
    message_id: UUID
    chat_id: UUID
    write_line: Callable[[str], Awaitable[None]]


_current_run_context: ContextVar[RunApprovalContext | None] = ContextVar(
    "_current_run_context", default=None
)


class ApprovalManager:
    """Manages interactive tool approval requests and decisions between agent and daemon."""

    def __init__(self) -> None:
        self._pending_decisions: dict[str, tuple[UUID, asyncio.Future[str]]] = {}
        self._approved_hosts: set[tuple[str, str]] = set()

    def set_context(
        self,
        request_id: UUID,
        message_id: UUID,
        chat_id: UUID,
        write_line: Callable[[str], Awaitable[None]],
    ) -> None:
        _current_run_context.set(
            RunApprovalContext(
                request_id=request_id,
                message_id=message_id,
                chat_id=chat_id,
                write_line=write_line,
            )
        )

    def clear_context(self, request_id: UUID | None = None) -> None:
        if request_id:
            self.cancel_for_request(request_id)
        _current_run_context.set(None)

    @property
    def current_chat_id(self) -> UUID | None:
        ctx = _current_run_context.get()
        return ctx.chat_id if ctx else None

    def is_host_approved(self, chat_id: str, host: str) -> bool:
        return (chat_id, host.lower()) in self._approved_hosts

    def approve_host(self, chat_id: str, host: str) -> None:
        if host:
            self._approved_hosts.add((chat_id, host.lower()))

    async def request_approval(
        self,
        call_id: str,
        tool_name: str,
        arguments: str,
        explanation: str,
        resource_summary: str,
        actions: list[str],
    ) -> str:
        ctx = _current_run_context.get()
        if not ctx:
            return "deny"

        event = ApprovalRequestedEvent(
            request_id=ctx.request_id,
            message_id=ctx.message_id,
            call_id=call_id,
            tool_name=tool_name,
            arguments=arguments,
            explanation=explanation,
            resource_summary=resource_summary,
            actions=actions,
        )

        loop = asyncio.get_running_loop()
        future: asyncio.Future[str] = loop.create_future()
        self._pending_decisions[call_id] = (ctx.request_id, future)

        try:
            await ctx.write_line(format_event(event))
            return await future
        finally:
            self._pending_decisions.pop(call_id, None)

    def resolve_decision(self, call_id: str, action: str) -> bool:
        entry = self._pending_decisions.get(call_id)
        if entry:
            _, future = entry
            if not future.done():
                future.set_result(action)
                return True
        return False

    def cancel_for_request(self, request_id: UUID) -> None:
        calls_to_cancel = [
            call_id
            for call_id, (req_id, _) in self._pending_decisions.items()
            if req_id == request_id
        ]
        for call_id in calls_to_cancel:
            entry = self._pending_decisions.pop(call_id, None)
            if entry:
                _, future = entry
                if not future.done():
                    future.cancel()

    def cancel_all(self) -> None:
        for _, future in self._pending_decisions.values():
            if not future.done():
                future.cancel()
        self._pending_decisions.clear()


# Default singleton instance
default_approval_manager = ApprovalManager()
