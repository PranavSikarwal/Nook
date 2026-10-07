import asyncio
from collections.abc import Awaitable, Callable
from uuid import UUID

from nook_worker.protocol import ApprovalRequestedEvent, format_event


class ApprovalManager:
    """Manages interactive tool approval requests and decisions between agent and daemon."""

    def __init__(self) -> None:
        self._pending_decisions: dict[str, asyncio.Future[str]] = {}
        self._approved_hosts: set[tuple[str, str]] = set()
        self._write_line: Callable[[str], Awaitable[None]] | None = None
        self._current_request_id: UUID | None = None
        self._current_message_id: UUID | None = None
        self._current_chat_id: UUID | None = None

    def set_context(
        self,
        request_id: UUID,
        message_id: UUID,
        chat_id: UUID,
        write_line: Callable[[str], Awaitable[None]],
    ) -> None:
        self._current_request_id = request_id
        self._current_message_id = message_id
        self._current_chat_id = chat_id
        self._write_line = write_line

    def clear_context(self) -> None:
        self.cancel_all()
        self._current_request_id = None
        self._current_message_id = None
        self._current_chat_id = None
        self._write_line = None

    @property
    def current_chat_id(self) -> UUID | None:
        return self._current_chat_id

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
        if (
            not self._write_line
            or not self._current_request_id
            or not self._current_message_id
        ):
            return "allow_once"

        event = ApprovalRequestedEvent(
            request_id=self._current_request_id,
            message_id=self._current_message_id,
            call_id=call_id,
            tool_name=tool_name,
            arguments=arguments,
            explanation=explanation,
            resource_summary=resource_summary,
            actions=actions,
        )

        loop = asyncio.get_running_loop()
        future: asyncio.Future[str] = loop.create_future()
        self._pending_decisions[call_id] = future

        await self._write_line(format_event(event))

        try:
            return await future
        finally:
            self._pending_decisions.pop(call_id, None)

    def resolve_decision(self, call_id: str, action: str) -> bool:
        future = self._pending_decisions.get(call_id)
        if future and not future.done():
            future.set_result(action)
            return True
        return False

    def cancel_all(self) -> None:
        for future in self._pending_decisions.values():
            if not future.done():
                future.cancel()
        self._pending_decisions.clear()


# Default singleton instance
default_approval_manager = ApprovalManager()
