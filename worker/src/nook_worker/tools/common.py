import json
from typing import Any


def tool_error(message: str) -> str:
    """Format a consistent JSON error string for tool responses."""
    return json.dumps({"error": message})


def tool_result(data: dict[str, Any] | list[Any]) -> str:
    """Format a JSON success string for tool responses."""
    return json.dumps(data, ensure_ascii=False)
