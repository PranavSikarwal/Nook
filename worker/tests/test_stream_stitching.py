from collections.abc import AsyncIterator
from typing import Any
from uuid import uuid4

import pytest
from langchain_core.messages import AIMessageChunk

from nook_worker.agent import RealAgentRunner
from nook_worker.protocol import (
    MessageFinishedEvent,
    MessageStartedEvent,
    RunRequest,
    TextDeltaEvent,
)


class MockChunkAgent:
    def __init__(self, stream_items: list[tuple[Any, dict[str, Any]]]) -> None:
        self.stream_items = stream_items

    async def astream(
        self,
        inputs: dict[str, Any],
        stream_mode: str = "messages",
        config: dict[str, Any] | None = None,
    ) -> AsyncIterator[tuple[Any, dict[str, Any]]]:
        for chunk, meta in self.stream_items:
            yield chunk, meta


@pytest.mark.asyncio
async def test_stream_stitching_filters_summarization_and_separates_turns():
    # Setup: 2 chunks from model call 1, 1 chunk from summarization, 2 chunks from model call 2
    chunk1a = AIMessageChunk(content="Let me fetch that page.", id="call_1")
    chunk1b = AIMessageChunk(content=" Starting now.", id="call_1")

    # Summarization chunk (should be dropped)
    chunk_sum = AIMessageChunk(content="[Internal summary text]", id="call_sum")

    # Internal middleware chunk (should be dropped)
    chunk_int = AIMessageChunk(content="[Internal tool setup]", id="call_int")

    # Second model call chunks (should be preceded by \n\n)
    chunk2a = AIMessageChunk(
        content="# Results\nHere is the page content.", id="call_2"
    )
    chunk2b = AIMessageChunk(content=" Hope this helps.", id="call_2")

    items = [
        (chunk1a, {"run_id": "run_1"}),
        (chunk1b, {"run_id": "run_1"}),
        (chunk_sum, {"lc_source": "summarization", "run_id": "run_sum"}),
        (chunk_int, {"lc_internal_call": True, "run_id": "run_int"}),
        (chunk2a, {"run_id": "run_2"}),
        (chunk2b, {"run_id": "run_2"}),
    ]

    agent = MockChunkAgent(items)
    runner = RealAgentRunner(agent)

    req = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="fetch https://example.com",
    )

    events = [event async for event in runner.run(req)]

    assert isinstance(events[0], MessageStartedEvent)
    assert isinstance(events[-1], MessageFinishedEvent)

    deltas = [e.text for e in events if isinstance(e, TextDeltaEvent)]
    full_output = "".join(deltas)

    # 1. Summarization and internal chunks were dropped
    assert "Internal summary text" not in full_output
    assert "Internal tool setup" not in full_output

    # 2. Pre-tool and post-tool turns were separated by \n\n
    assert (
        "Let me fetch that page. Starting now.\n\n# Results\nHere is the page content. Hope this helps."
        == full_output
    )
