import pytest
from pydantic import BaseModel

from nook_worker.registry import ToolArgumentError, ToolDefinition, ToolRegistry
from nook_worker.tools.fetch import StrictWebFetchInput
from nook_worker.tools.search import StrictWebSearchInput


class DummyArgs(BaseModel):
    query: str


async def dummy_executor(query: str) -> str:
    return f"Result: {query}"


def test_tool_registry():
    registry = ToolRegistry()
    assert len(registry.list_tools()) == 0

    tool = ToolDefinition(
        name="nook:web_search",
        description="Search web",
        argument_schema=DummyArgs,
        capabilities={"network": True, "read_only": True},
        executor=dummy_executor,
    )
    registry.register(tool)

    assert registry.get("nook:web_search") == tool
    assert registry.get("nonexistent") is None
    assert len(registry.list_tools()) == 1


def _registry_with_real_schemas() -> ToolRegistry:
    registry = ToolRegistry()
    registry.register(
        ToolDefinition(
            name="nook:web_search",
            description="Search",
            argument_schema=StrictWebSearchInput,
            capabilities={},
            executor=dummy_executor,
        )
    )
    registry.register(
        ToolDefinition(
            name="nook:web_fetch",
            description="Fetch",
            argument_schema=StrictWebFetchInput,
            capabilities={},
            executor=dummy_executor,
        )
    )
    return registry


def test_validate_search_returns_cleaned_arguments():
    registry = _registry_with_real_schemas()
    assert registry.validate_arguments("nook:web_search", {"query": "  cats "}) == {
        "query": "cats",
        "max_results": 5,
    }


@pytest.mark.parametrize(
    "arguments",
    [{"query": ""}, {"query": "   "}, {}, {"query": 5}, {"query": None}],
)
def test_validate_search_rejects_bad_query(arguments):
    registry = _registry_with_real_schemas()
    with pytest.raises(ToolArgumentError) as info:
        registry.validate_arguments("nook:web_search", arguments)
    assert "query" in str(info.value)


def test_validate_search_rejects_out_of_range_max_results():
    registry = _registry_with_real_schemas()
    with pytest.raises(ToolArgumentError, match="max_results"):
        registry.validate_arguments(
            "nook:web_search", {"query": "a", "max_results": 99}
        )


@pytest.mark.parametrize(
    "url", ["http://example.com/a", "https://example.com:8443/x?y=1", " https://a.io "]
)
def test_validate_fetch_accepts_http_and_https(url):
    registry = _registry_with_real_schemas()
    cleaned = registry.validate_arguments("nook:web_fetch", {"url": url})
    assert cleaned == {"url": url.strip()}


@pytest.mark.parametrize(
    "arguments",
    [
        {},
        {"url": ""},
        {"url": "ftp://example.com/file"},
        {"url": "file:///etc/passwd"},
        {"url": "https://"},
        {"url": "http:///path"},
        {"url": "example.com"},
        {"url": 7},
    ],
)
def test_validate_fetch_rejects_bad_url(arguments):
    registry = _registry_with_real_schemas()
    with pytest.raises(ToolArgumentError) as info:
        registry.validate_arguments("nook:web_fetch", arguments)
    assert "url" in str(info.value)


def test_validate_unknown_tool_raises():
    registry = _registry_with_real_schemas()
    with pytest.raises(ToolArgumentError, match="nook:nope"):
        registry.validate_arguments("nook:nope", {})


def test_tool_argument_error_message_is_short_and_plain():
    registry = _registry_with_real_schemas()
    with pytest.raises(ToolArgumentError) as info:
        registry.validate_arguments("nook:web_fetch", {"url": "ftp://x.com"})
    message = str(info.value)
    assert "\n" not in message
    assert "pydantic" not in message.lower()
