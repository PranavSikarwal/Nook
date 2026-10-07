import pytest
from pydantic import BaseModel

from nook_worker.registry import ToolDefinition, ToolRegistry


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
        approval_tier="allow",
        executor=dummy_executor,
    )
    registry.register(tool)

    assert registry.get("nook:web_search") == tool
    assert registry.get("nonexistent") is None
    assert len(registry.list_tools()) == 1


@pytest.mark.asyncio
async def test_tool_registry_denied_tier():
    registry = ToolRegistry()
    tool = ToolDefinition(
        name="nook:blocked_tool",
        description="Blocked tool",
        argument_schema=DummyArgs,
        capabilities={},
        approval_tier="deny",
        executor=dummy_executor,
    )
    registry.register(tool)

    # When runner inspects approval_tier == "deny", execution is blocked
    defn = registry.get("nook:blocked_tool")
    assert defn is not None
    assert defn.approval_tier == "deny"
