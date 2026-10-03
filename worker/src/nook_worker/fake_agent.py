from collections.abc import AsyncIterator
from uuid import UUID, uuid4

from nook_worker.agent import AgentRunner
from nook_worker.protocol import (
    EventMessage,
    MessageFinishedEvent,
    MessageStartedEvent,
    RunRequest,
    TextDeltaEvent,
)

__all__ = ["AgentRunner", "FakeAgentRunner"]


class FakeAgentRunner:
    def __init__(self, scripted_text: list[str] | None = None) -> None:
        self._scripted_text = (
            scripted_text
            if scripted_text is not None
            else ["Hello from fake Nook worker!"]
        )

    async def run(
        self, request: RunRequest, message_id: UUID | None = None
    ) -> AsyncIterator[EventMessage]:
        msg_id = message_id or uuid4()
        yield MessageStartedEvent(
            request_id=request.request_id,
            message_id=msg_id,
        )

        for chunk in self._scripted_text:
            yield TextDeltaEvent(
                request_id=request.request_id,
                message_id=msg_id,
                text=chunk,
            )

        yield MessageFinishedEvent(
            request_id=request.request_id,
            message_id=msg_id,
            status="complete",
        )
