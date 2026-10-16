from typing import Any

import pytest

from nook_worker import agent
from nook_worker.agent import _create_registry_runner
from nook_worker.registry import ToolDefinition, ToolRegistry
from nook_worker.tools.fetch import StrictWebFetchInput
from nook_worker.tools.search import StrictWebSearchInput


class FakeTool:
    def __init__(self) -> None:
        self.executor_calls: list[dict[str, Any]] = []
        self.approval_calls: list[dict[str, Any]] = []

    async def executor(self, **kwargs: Any) -> str:
        self.executor_calls.append(kwargs)
        return "executed"

    async def approval(
        self, registered_name: str, kwargs: dict[str, Any]
    ) -> str | None:
        self.approval_calls.append(kwargs)
        return None


@pytest.fixture
def fake(monkeypatch: pytest.MonkeyPatch) -> FakeTool:
    fake_tool = FakeTool()
    registry = ToolRegistry()
    registry.register(
        ToolDefinition(
            name="nook:web_search",
            description="Search",
            argument_schema=StrictWebSearchInput,
            capabilities={},
            approval_tier="allow",
            executor=fake_tool.executor,
        )
    )
    registry.register(
        ToolDefinition(
            name="nook:web_fetch",
            description="Fetch",
            argument_schema=StrictWebFetchInput,
            capabilities={},
            approval_tier="ask_once_per_host",
            executor=fake_tool.executor,
        )
    )
    monkeypatch.setattr(agent, "default_registry", registry)
    monkeypatch.setattr(agent, "_check_tool_approval", fake_tool.approval)
    return fake_tool


@pytest.mark.parametrize(
    ("tool", "arguments"),
    [
        ("nook:web_search", {"query": ""}),
        ("nook:web_search", {"query": "   "}),
        ("nook:web_fetch", {}),
        ("nook:web_fetch", {"url": "ftp://example.com/a"}),
        ("nook:web_fetch", {"url": "https:///nohost"}),
    ],
)
async def test_invalid_arguments_skip_approval_and_executor(fake, tool, arguments):
    result = await _create_registry_runner(tool)(**arguments)

    assert result.startswith("Error:")
    assert fake.approval_calls == []
    assert fake.executor_calls == []


async def test_valid_search_reaches_executor_with_cleaned_arguments(fake):
    result = await _create_registry_runner("nook:web_search")(query=" cats ")

    assert result == "executed"
    assert fake.executor_calls == [{"query": "cats", "max_results": 5}]


async def test_valid_fetch_asks_approval_then_executes(fake):
    result = await _create_registry_runner("nook:web_fetch")(url="https://example.com/")

    assert result == "executed"
    assert fake.approval_calls == [{"url": "https://example.com/"}]
    assert fake.executor_calls == [{"url": "https://example.com/"}]


def test_missing_required_field_returns_error_text_to_model():
    from pydantic import ValidationError

    from nook_worker.agent import _format_validation_error
    from nook_worker.tools.fetch import WebFetchInput

    with pytest.raises(ValidationError) as caught:
        WebFetchInput.model_validate({})
    message = _format_validation_error(caught.value)
    assert message.startswith("Error: invalid arguments.")
    assert "url" in message
