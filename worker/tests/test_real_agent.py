from uuid import uuid4

import httpx
import psycopg
import pytest
from nook_worker.agent import RealAgentRunner, build_deep_agent, create_model
from nook_worker.checkpointer import (
    create_pool,
    delete_thread_memory,
    setup_checkpointer,
)
from nook_worker.config import WorkerConfig
from nook_worker.protocol import (
    MessageFinishedEvent,
    MessageStartedEvent,
    RunRequest,
    TextDeltaEvent,
)
from nook_worker.titles import generate_title


def _is_postgres_reachable(url: str) -> bool:
    try:
        with psycopg.connect(url, connect_timeout=2) as conn:
            conn.close()
        return True
    except (psycopg.Error, OSError):
        return False


def _is_endpoint_authenticated(url: str, key: str) -> bool:
    if not url or not key:
        return False
    try:
        with httpx.Client(timeout=3) as client:
            resp = client.get(
                f"{url}/models",
                headers={"Authorization": f"Bearer {key}"},
            )
            return resp.status_code == 200
    except (httpx.HTTPError, OSError):
        return False


_CONFIG = WorkerConfig()
_POSTGRES_REACHABLE = _is_postgres_reachable(_CONFIG.database_url)
_ENDPOINT_READY = _is_endpoint_authenticated(
    _CONFIG.base_url, _CONFIG.api_key.get_secret_value()
)


@pytest.mark.skipif(
    not _ENDPOINT_READY,
    reason="Live model credentials not configured or endpoint not authenticated",
)
@pytest.mark.asyncio
async def test_real_agent_streaming_and_no_tools():
    config = WorkerConfig()
    model = create_model(config)
    agent = build_deep_agent(model, config)
    runner = RealAgentRunner(agent)

    req = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="Please run the shell command `echo 42` and return the result. If you have no tools, just say NO_TOOLS.",
    )

    events = [event async for event in runner.run(req)]

    assert len(events) >= 3
    assert isinstance(events[0], MessageStartedEvent)

    deltas = [e.text for e in events if isinstance(e, TextDeltaEvent)]
    assert len(deltas) >= 1
    full_text = "".join(deltas)
    assert len(full_text) > 0

    # Ensure no tool call events were generated
    event_types = [e.type for e in events]
    assert "tool_call_started" not in event_types
    assert "tool_call_finished" not in event_types

    assert isinstance(events[-1], MessageFinishedEvent)
    assert events[-1].status == "complete"


@pytest.mark.skipif(
    not _ENDPOINT_READY or not _POSTGRES_REACHABLE,
    reason="Live model endpoint or Postgres not available",
)
@pytest.mark.asyncio
async def test_real_agent_conversation_memory_and_delete():
    config = WorkerConfig()
    pool = create_pool(config.database_url)
    async with pool:
        checkpointer = await setup_checkpointer(pool)
        model = create_model(config)
        agent = build_deep_agent(model, config, checkpointer)
        runner = RealAgentRunner(agent)

        chat_id = uuid4()

        # Turn 1: give a secret number
        req1 = RunRequest(
            request_id=uuid4(),
            chat_id=chat_id,
            text="Remember this secret number: 93821. Reply with only 'OK'.",
        )
        events1 = [event async for event in runner.run(req1)]
        assert isinstance(events1[-1], MessageFinishedEvent)
        assert events1[-1].status == "complete"

        # Turn 2: ask for the secret number
        req2 = RunRequest(
            request_id=uuid4(),
            chat_id=chat_id,
            text="What was the secret number I asked you to remember? Reply with only the number.",
        )
        events2 = [event async for event in runner.run(req2)]
        assert isinstance(events2[-1], MessageFinishedEvent)
        assert events2[-1].status == "complete"

        deltas2 = "".join([e.text for e in events2 if isinstance(e, TextDeltaEvent)])
        assert "93821" in deltas2

        # Test W8: Delete chat memory
        await delete_thread_memory(checkpointer, str(chat_id))

        # Turn 3: ask again after deletion, should not know the secret number
        req3 = RunRequest(
            request_id=uuid4(),
            chat_id=chat_id,
            text="What was the secret number I asked you to remember earlier?",
        )
        events3 = [event async for event in runner.run(req3)]
        deltas3 = "".join([e.text for e in events3 if isinstance(e, TextDeltaEvent)])
        assert "93821" not in deltas3


@pytest.mark.skipif(
    not _ENDPOINT_READY,
    reason="Live model credentials not configured or endpoint not authenticated",
)
@pytest.mark.asyncio
async def test_real_title_generation():
    config = WorkerConfig()
    model = create_model(config)
    title = await generate_title(model, "How do LangGraph checkpointers work?")
    assert len(title.split()) <= 6
    assert '"' not in title
    assert "'" not in title
    assert not title.endswith(".")
