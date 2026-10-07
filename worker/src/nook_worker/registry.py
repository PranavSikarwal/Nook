from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

from pydantic import BaseModel


@dataclass
class ToolDefinition:
    name: str
    description: str
    argument_schema: type[BaseModel]
    capabilities: dict[str, Any]
    approval_tier: str
    executor: Callable[..., Awaitable[str]]


class ToolRegistry:
    def __init__(self) -> None:
        self._tools: dict[str, ToolDefinition] = {}

    def register(self, tool: ToolDefinition) -> None:
        self._tools[tool.name] = tool

    def get(self, name: str) -> ToolDefinition | None:
        return self._tools.get(name)

    def list_tools(self) -> list[ToolDefinition]:
        return list(self._tools.values())

    def clear(self) -> None:
        self._tools.clear()


# Global default registry
default_registry = ToolRegistry()
