import asyncio
import sys
from typing import Any

from ddgs.exceptions import DDGSException
from pydantic import BaseModel, Field

from nook_worker.tools.common import tool_error, tool_result


def _is_no_results_error(error: DDGSException) -> bool:
    return str(error).strip() in {"No results found", "No results found."}


class WebSearchInput(BaseModel):
    query: str = Field(description="The search query string")
    max_results: int = Field(
        default=5, ge=1, le=10, description="Maximum number of results (1 to 10)"
    )


def _run_ddgs_sync(query: str, max_results: int) -> list[dict[str, Any]]:
    from ddgs import DDGS

    try:
        client = DDGS(timeout=10, verify=True)
        results = client.text(
            query=query,
            max_results=min(max(max_results, 1), 10),
            backend="duckduckgo",
        )
        return [
            {
                "title": str(r.get("title", "")).strip(),
                "href": str(r.get("href", "")).strip(),
                "body": str(r.get("body", "")).strip(),
            }
            for r in results
        ]
    except DDGSException as exc:
        if _is_no_results_error(exc):
            return []
        raise


async def execute_web_search(query: str, max_results: int = 5) -> str:
    """Search the web using DuckDuckGo backend only.

    Enforces backend="duckduckgo" explicitly.
    """
    cleaned_query = query.strip()
    if not cleaned_query:
        return tool_error("Empty search query")

    try:
        formatted = await asyncio.to_thread(_run_ddgs_sync, cleaned_query, max_results)
        return tool_result(formatted)
    except DDGSException as exc:
        if _is_no_results_error(exc):
            return tool_result([])
        return tool_error("Search provider error: request failed")
    except Exception as exc:
        sys.stderr.write(f"Search provider error: {exc}\n")
        sys.stderr.flush()
        return tool_error("Search provider error: request failed")
