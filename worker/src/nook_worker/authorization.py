import asyncio
import logging
from collections.abc import Awaitable, Callable
from contextvars import ContextVar
from dataclasses import dataclass
from typing import Literal
from uuid import UUID, uuid4

from nook_worker.protocol import AuthorizeToolEvent, format_event

logger = logging.getLogger("nook_worker.authorization")

Decision = Literal["allow", "deny"]
CallKey = tuple[UUID, str]


@dataclass(frozen=True)
class AuthorizationDecision:
    allowed: bool
    reason: str | None = None


@dataclass(frozen=True)
class RunContext:
    request_id: UUID
    message_id: UUID
    chat_id: UUID
    write_line: Callable[[str], Awaitable[None]]


_current_run_context: ContextVar[RunContext | None] = ContextVar(
    "_current_run_context", default=None
)


class AuthorizationManager:
    """Asks the Daemon to authorize each tool call and waits for its result."""

    def __init__(self, authorization_timeout: float = 300.0) -> None:
        self._waiting: dict[CallKey, asyncio.Future[AuthorizationDecision]] = {}
        self._authorization_timeout = authorization_timeout

    def set_context(
        self,
        request_id: UUID,
        message_id: UUID,
        chat_id: UUID,
        write_line: Callable[[str], Awaitable[None]],
    ) -> None:
        _current_run_context.set(
            RunContext(
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

    async def authorize(self, tool_name: str, arguments: str) -> AuthorizationDecision:
        ctx = _current_run_context.get()
        if ctx is None:
            return AuthorizationDecision(allowed=False, reason="no active run")

        call_id = f"call_{uuid4().hex}"
        key = (ctx.request_id, call_id)
        future: asyncio.Future[AuthorizationDecision] = (
            asyncio.get_running_loop().create_future()
        )
        self._waiting[key] = future
        event = AuthorizeToolEvent(
            request_id=ctx.request_id,
            message_id=ctx.message_id,
            call_id=call_id,
            chat_id=ctx.chat_id,
            tool_name=tool_name,
            arguments=arguments,
        )
        try:
            await ctx.write_line(format_event(event))
            return await asyncio.wait_for(future, self._authorization_timeout)
        except TimeoutError:
            return AuthorizationDecision(
                allowed=False, reason="authorization timed out"
            )
        finally:
            self._waiting.pop(key, None)

    def resolve(
        self,
        request_id: UUID,
        call_id: str,
        decision: Decision,
        reason: str | None = None,
    ) -> bool:
        future = self._waiting.get((request_id, call_id))
        if future is None or future.done():
            logger.debug(
                "ignored authorization result: request_id=%s call_id=%s",
                request_id,
                call_id,
            )
            return False
        future.set_result(
            AuthorizationDecision(allowed=decision == "allow", reason=reason)
        )
        return True

    def cancel_for_request(self, request_id: UUID) -> None:
        for (waiting_request_id, _), future in list(self._waiting.items()):
            if waiting_request_id == request_id and not future.done():
                future.cancel()

    def cancel_all(self) -> None:
        for future in self._waiting.values():
            if not future.done():
                future.cancel()


default_authorization_manager = AuthorizationManager()
