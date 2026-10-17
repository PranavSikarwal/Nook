import asyncio
import json
import sys
from typing import Any

import httpx

from nook_worker.tools.common import clean_text, clean_url, tool_error, tool_result
from nook_worker.tools.media_search import MediaSearchInput, StrictMediaSearchInput

OPEN_LIBRARY_SEARCH_URL = "https://openlibrary.org/search.json"
OPEN_LIBRARY_BOOK_URL = "https://openlibrary.org"
MAX_RESPONSE_BYTES = 1024 * 1024
TIMEOUT_SECONDS = 15.0
FIELDS = "key,title,author_name,first_publish_year,edition_count,isbn,subject"

BookSearchInput = MediaSearchInput
StrictBookSearchInput = StrictMediaSearchInput


def _book_item(raw: dict[str, Any]) -> dict[str, Any] | None:
    key = raw.get("key")
    if not isinstance(key, str) or not key.startswith("/works/"):
        return None
    authors = raw.get("author_name")
    isbns = raw.get("isbn")
    subjects = raw.get("subject")
    year = raw.get("first_publish_year")
    editions = raw.get("edition_count")
    return {
        "title": clean_text(raw.get("title"), 200),
        "authors": [clean_text(a, 100) for a in authors[:5]]
        if isinstance(authors, list)
        else [],
        "first_published": year if isinstance(year, int) else None,
        "editions": editions if isinstance(editions, int) else None,
        "isbn": clean_text(isbns[0], 20) if isinstance(isbns, list) and isbns else "",
        "subjects": [clean_text(s, 60) for s in subjects[:5]]
        if isinstance(subjects, list)
        else [],
        "url": clean_url(f"{OPEN_LIBRARY_BOOK_URL}{key}"),
    }


async def _fetch_json(
    query: str, limit: int, transport: httpx.AsyncBaseTransport | None = None
) -> dict[str, Any]:
    async with (
        httpx.AsyncClient(
            transport=transport,
            timeout=TIMEOUT_SECONDS,
            follow_redirects=False,
            headers={"User-Agent": "NookDesktopAssistant/0.1.0"},
        ) as client,
        client.stream(
            "GET",
            OPEN_LIBRARY_SEARCH_URL,
            params={"q": query, "limit": limit, "fields": FIELDS},
        ) as response,
    ):
        if response.status_code != 200:
            raise ValueError(f"HTTP {response.status_code}")
        body = b""
        async for chunk in response.aiter_bytes():
            body += chunk
            if len(body) > MAX_RESPONSE_BYTES:
                raise ValueError("Response is too large")
    data = json.loads(body)
    if not isinstance(data, dict):
        raise ValueError("Unexpected response")
    return data


async def execute_book_search(query: str, max_results: int = 5) -> str:
    """Search books in the Open Library catalog. The result holds catalog data."""
    cleaned_query = query.strip()
    if not cleaned_query:
        return tool_error("Empty search query")
    try:
        async with asyncio.timeout(TIMEOUT_SECONDS * 2):
            data = await _fetch_json(cleaned_query, min(max(max_results, 1), 10))
    except Exception as exc:
        sys.stderr.write(f"Book search provider error: {type(exc).__name__}\n")
        sys.stderr.flush()
        return tool_error("Search provider error: request failed")
    docs = data.get("docs")
    if not isinstance(docs, list):
        return tool_error("Search provider error: unexpected response")
    items = (_book_item(doc) for doc in docs if isinstance(doc, dict))
    return tool_result([item for item in items if item is not None])
