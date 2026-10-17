from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

from pydantic import BaseModel, ValidationError


class ToolArgumentError(ValueError):
    pass


def _describe_validation_error(error: ValidationError) -> str:
    first = error.errors()[0]
    field = ".".join(str(part) for part in first["loc"]) or "arguments"
    message = first["msg"].removeprefix("Value error, ")
    return f"Invalid argument '{field}': {message}"


@dataclass
class ToolDefinition:
    name: str
    description: str
    argument_schema: type[BaseModel]
    capabilities: dict[str, Any]
    executor: Callable[..., Awaitable[str]]


class ToolRegistry:
    def __init__(self) -> None:
        self._tools: dict[str, ToolDefinition] = {}

    def register(self, tool: ToolDefinition) -> None:
        self._tools[tool.name] = tool

    def get(self, name: str) -> ToolDefinition | None:
        return self._tools.get(name)

    def validate_arguments(
        self, tool_name: str, arguments: dict[str, Any]
    ) -> dict[str, Any]:
        tool = self._tools.get(tool_name)
        if tool is None:
            raise ToolArgumentError(f"Tool '{tool_name}' is not registered.")
        try:
            return tool.argument_schema.model_validate(arguments).model_dump()
        except ValidationError as err:
            raise ToolArgumentError(_describe_validation_error(err)) from err

    def list_tools(self) -> list[ToolDefinition]:
        return list(self._tools.values())

    def clear(self) -> None:
        self._tools.clear()


# Global default registry
default_registry = ToolRegistry()
