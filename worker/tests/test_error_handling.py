from collections.abc import AsyncIterator
from typing import Any, cast
from uuid import UUID, uuid4

import httpx
import openai
import pytest
from nook_worker.agent import RealAgentRunner, build_deep_agent, create_model
from nook_worker.config import WorkerConfig
from nook_worker.main import handle_run
from nook_worker.protocol import ErrorEvent, EventMessage, RunRequest, parse_event
from pydantic import SecretStr


@pytest.mark.asyncio
async def test_wrong_base_url_error_mapping():
    config = WorkerConfig(
        base_url="http://127.0.0.1:59999/v1",  # Unreachable port
        api_key=SecretStr("dummy"),
        model="test-model",
    )
    model = create_model(config)
    agent = build_deep_agent(model, config)
    runner = RealAgentRunner(agent)

    req = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="Hello",
    )

    events: list[str] = []

    async def mock_write(line: str) -> None:
        events.append(line)

    await handle_run(req, runner, mock_write)

    assert len(events) >= 1
    err_event = parse_event(events[-1])
    assert isinstance(err_event, ErrorEvent)
    assert err_event.error.code == "endpoint_unreachable"
    assert err_event.error.retryable is True


@pytest.mark.asyncio
async def test_handle_run_maps_timeout_error():
    class TimeoutRunner:
        async def run(
            self, request: RunRequest, message_id: UUID | None = None
        ) -> AsyncIterator[EventMessage]:
            raise httpx.ReadTimeout("Read timed out")
            yield  # type: ignore[unreachable]

    events: list[str] = []

    async def mock_write(line: str) -> None:
        events.append(line)

    req = RunRequest(request_id=uuid4(), chat_id=uuid4(), text="hi")
    await handle_run(req, TimeoutRunner(), mock_write)

    assert len(events) == 1
    err_event = parse_event(events[0])
    assert isinstance(err_event, ErrorEvent)
    assert err_event.error.code == "endpoint_unreachable"
    assert err_event.error.retryable is True


@pytest.mark.asyncio
async def test_handle_run_maps_api_status_error():
    class StatusErrorRunner:
        async def run(
            self, request: RunRequest, message_id: UUID | None = None
        ) -> AsyncIterator[EventMessage]:
            http_req = httpx.Request("POST", "http://localhost:8000")
            http_resp = httpx.Response(404, request=http_req)
            raise openai.APIStatusError(
                message="Not found",
                response=cast(Any, http_resp),
                body=None,
            )
            yield  # type: ignore[unreachable]

    events: list[str] = []

    async def mock_write(line: str) -> None:
        events.append(line)

    req = RunRequest(request_id=uuid4(), chat_id=uuid4(), text="hi")
    await handle_run(req, StatusErrorRunner(), mock_write)

    assert len(events) == 1
    err_event = parse_event(events[0])
    assert isinstance(err_event, ErrorEvent)
    assert err_event.error.code == "endpoint_error"
    assert err_event.error.retryable is False


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


@pytest.mark.asyncio
async def test_wrong_model_error_mapping():
    real_config = WorkerConfig()
    if not _is_endpoint_authenticated(
        real_config.base_url, real_config.api_key.get_secret_value()
    ):
        pytest.skip("NOOK_BASE_URL or NOOK_API_KEY is not configured or authenticated")

    config = WorkerConfig(
        base_url=real_config.base_url,
        api_key=real_config.api_key,
        model="nonexistent-model-name-xyz",
    )
    model = create_model(config)
    agent = build_deep_agent(model, config)
    runner = RealAgentRunner(agent)

    req = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="Hello",
    )

    events: list[str] = []

    async def mock_write(line: str) -> None:
        events.append(line)

    await handle_run(req, runner, mock_write)

    assert len(events) >= 1
    err_event = parse_event(events[-1])
    assert isinstance(err_event, ErrorEvent)
    assert err_event.error.code == "endpoint_error"
    assert err_event.error.retryable is False
