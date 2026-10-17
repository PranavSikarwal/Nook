import json
from typing import Any


def tool_error(message: str) -> str:
    """Format a consistent JSON error string for tool responses."""
    return json.dumps({"error": message})


def tool_result(data: dict[str, Any] | list[Any]) -> str:
    """Format a JSON success string for tool responses."""
    return json.dumps(data, ensure_ascii=False)


def clean_text(value: Any, limit: int) -> str:
    """Return untrusted text as one trimmed line of at most `limit` characters."""
    text = " ".join(str(value if value is not None else "").split())
    return text if len(text) <= limit else text[: limit - 1].rstrip() + "…"


def clean_url(value: Any) -> str:
    """Return an http or https URL, or an empty string for anything else."""
    text = str(value if value is not None else "").strip()
    return (
        text if text.startswith(("http://", "https://")) and len(text) <= 2048 else ""
    )
