import asyncio
import sys
from typing import Any, Literal

from ddgs.exceptions import DDGSException
from pydantic import BaseModel, Field, field_validator

from nook_worker.tools.common import clean_text, clean_url, tool_error, tool_result

SearchKind = Literal["news", "videos"]

TITLE_LIMIT = 200
TEXT_LIMIT = 500
SHORT_LIMIT = 100


class MediaSearchInput(BaseModel):
    query: str = Field(description="The search query string")
    max_results: int = Field(
        default=5, ge=1, le=10, description="Maximum number of results (1 to 10)"
    )


class StrictMediaSearchInput(MediaSearchInput):
    @field_validator("query")
    @classmethod
    def _query_not_blank(cls, value: str) -> str:
        cleaned = value.strip()
        if not cleaned:
            raise ValueError("query must not be empty")
        return cleaned


def _is_no_results_error(error: DDGSException) -> bool:
    return str(error).strip() in {"No results found", "No results found."}


def _news_item(raw: dict[str, Any]) -> dict[str, Any] | None:
    url = clean_url(raw.get("url"))
    if not url:
        return None
    return {
        "title": clean_text(raw.get("title"), TITLE_LIMIT),
        "url": url,
        "source": clean_text(raw.get("source"), SHORT_LIMIT),
        "date": clean_text(raw.get("date"), SHORT_LIMIT),
        "snippet": clean_text(raw.get("body"), TEXT_LIMIT),
    }


def _video_item(raw: dict[str, Any]) -> dict[str, Any] | None:
    url = clean_url(raw.get("content"))
    if not url:
        return None
    return {
        "title": clean_text(raw.get("title"), TITLE_LIMIT),
        "url": url,
        "description": clean_text(raw.get("description"), TEXT_LIMIT),
        "duration": clean_text(raw.get("duration"), SHORT_LIMIT),
        "publisher": clean_text(raw.get("publisher"), SHORT_LIMIT),
        "uploader": clean_text(raw.get("uploader"), SHORT_LIMIT),
        "published": clean_text(raw.get("published"), SHORT_LIMIT),
    }


_CONVERTERS = {"news": _news_item, "videos": _video_item}


def _run_ddgs_sync(
    kind: SearchKind, query: str, max_results: int
) -> list[dict[str, Any]]:
    from ddgs import DDGS

    try:
        client = DDGS(timeout=10, verify=True)
        search = getattr(client, kind)
        results = search(
            query=query,
            max_results=min(max(max_results, 1), 10),
            backend="duckduckgo",
        )
    except DDGSException as exc:
        if _is_no_results_error(exc):
            return []
        raise
    convert = _CONVERTERS[kind]
    items = (convert(result) for result in results)
    return [item for item in items if item is not None]


async def _execute(kind: SearchKind, query: str, max_results: int) -> str:
    cleaned_query = query.strip()
    if not cleaned_query:
        return tool_error("Empty search query")
    try:
        results = await asyncio.wait_for(
            asyncio.to_thread(_run_ddgs_sync, kind, cleaned_query, max_results), 20
        )
        return tool_result(results)
    except DDGSException as exc:
        if _is_no_results_error(exc):
            return tool_result([])
        return tool_error("Search provider error: request failed")
    except Exception as exc:
        sys.stderr.write(f"Search provider error: {type(exc).__name__}\n")
        sys.stderr.flush()
        return tool_error("Search provider error: request failed")


async def execute_news_search(query: str, max_results: int = 5) -> str:
    """Search recent news with DuckDuckGo."""
    return await _execute("news", query, max_results)


async def execute_video_search(query: str, max_results: int = 5) -> str:
    """Search videos with DuckDuckGo."""
    return await _execute("videos", query, max_results)
