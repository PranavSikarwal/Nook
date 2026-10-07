import json
from typing import Any

from pydantic import BaseModel, Field


class WebSearchInput(BaseModel):
    query: str = Field(description="The search query string")
    max_results: int = Field(
        default=5, ge=1, le=10, description="Maximum number of results (1 to 10)"
    )


async def execute_web_search(query: str, max_results: int = 5) -> str:
    """Search the web using DuckDuckGo backend only.

    Enforces backend="duckduckgo" explicitly.
    """
    from ddgs import DDGS

    cleaned_query = query.strip()
    if not cleaned_query:
        return json.dumps({"error": "Empty search query"})

    try:
        # DDGS runs synchronously, execute in standard runner or call directly
        client = DDGS(timeout=10, verify=True)
        results = client.text(
            query=cleaned_query,
            max_results=min(max(max_results, 1), 10),
            backend="duckduckgo",
        )

        formatted: list[dict[str, Any]] = []
        for r in results:
            formatted.append(
                {
                    "title": str(r.get("title", "")).strip(),
                    "href": str(r.get("href", "")).strip(),
                    "body": str(r.get("body", "")).strip(),
                }
            )

        return json.dumps(formatted, ensure_ascii=False)
    except Exception as exc:
        return json.dumps({"error": f"Search provider error: {exc}"})
