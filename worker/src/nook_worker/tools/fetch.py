import asyncio
import ipaddress
import socket
from typing import Any
from urllib.parse import urljoin, urlparse

import httpcore
import httpx
from bs4 import BeautifulSoup
from pydantic import BaseModel, Field

from nook_worker.tools.common import tool_error, tool_result

MAX_REDIRECTS = 5
TIMEOUT_SECONDS = 15.0
MAX_RESPONSE_BYTES = 2 * 1024 * 1024  # 2 MB
CHUNK_SIZE = 64 * 1024  # 64 KB
ALLOWED_CONTENT_TYPES = [
    "text/html",
    "text/plain",
    "text/markdown",
    "application/json",
]


class WebFetchInput(BaseModel):
    url: str = Field(description="The public HTTP or HTTPS URL to fetch")


class PinnedAsyncHTTPTransport(httpx.AsyncHTTPTransport):
    """Transport that forces TCP connections to a validated IP while preserving TLS SNI and Host."""

    def __init__(self, pinned_ip: str, **kwargs: Any) -> None:
        super().__init__(**kwargs)

        class PinnedBackend(httpcore.AsyncNetworkBackend):
            def __init__(self, target_ip: str) -> None:
                self._target_ip = target_ip
                self._base: Any = httpcore.AnyIOBackend()

            async def connect_tcp(
                self,
                host: str,
                port: int,
                timeout: float | None = None,
                local_address: str | None = None,
                socket_options: Any = None,
            ) -> httpcore.AsyncNetworkStream:
                return await self._base.connect_tcp(
                    self._target_ip,
                    port,
                    timeout=timeout,
                    local_address=local_address,
                    socket_options=socket_options,
                )

        # pyright: ignore[reportAttributeAccessIssue]
        self._pool._network_backend = PinnedBackend(pinned_ip)


def is_ip_blocked(ip: ipaddress.IPv4Address | ipaddress.IPv6Address) -> bool:
    """Return True if IP is loopback, link-local, private, multicast, or unspecified."""
    return (
        ip.is_loopback
        or ip.is_private
        or ip.is_link_local
        or ip.is_multicast
        or ip.is_unspecified
        or ip.is_reserved
    )


async def resolve_and_validate_host(hostname: str, port: int) -> list[str]:
    """Resolve hostname and ensure all returned IP addresses are safe public IPs."""
    loop = asyncio.get_running_loop()
    try:
        addr_info = await loop.getaddrinfo(
            hostname,
            port,
            type=socket.SOCK_STREAM,
        )
    except socket.gaierror as exc:
        raise ValueError(f"Failed to resolve host '{hostname}': {exc}") from exc

    validated_ips: list[str] = []
    for _family, _, _, _, sockaddr in addr_info:
        ip_str = str(sockaddr[0])
        try:
            ip_obj = ipaddress.ip_address(ip_str)
        except ValueError as exc:
            raise ValueError(f"Invalid IP address '{ip_str}'") from exc

        if is_ip_blocked(ip_obj):
            raise ValueError(f"Blocked target address '{ip_str}' for host '{hostname}'")
        if ip_str not in validated_ips:
            validated_ips.append(ip_str)

    if not validated_ips:
        raise ValueError(f"No IP addresses resolved for host '{hostname}'")

    return validated_ips


def extract_readable_text(content_type: str, content: bytes) -> str:
    """Extract readable text from HTML, Markdown, Plain Text, or JSON."""
    if "text/html" in content_type:
        soup = BeautifulSoup(content, "html.parser")
        for element in soup(["script", "style", "nav", "footer", "header", "noscript"]):
            element.decompose()
        return soup.get_text(separator="\n", strip=True)
    return content.decode("utf-8", errors="replace").strip()


async def _read_streamed_body(response: httpx.Response) -> bytes:
    """Read response body up to MAX_RESPONSE_BYTES chunks.

    Fails immediately if response exceeds MAX_RESPONSE_BYTES.
    """
    content_length = response.headers.get("content-length")
    if content_length:
        try:
            cl_val = int(content_length.strip())
            if cl_val > MAX_RESPONSE_BYTES or cl_val < 0:
                raise ValueError(
                    f"Response exceeds size limit of {MAX_RESPONSE_BYTES} bytes"
                )
        except ValueError as exc:
            if "exceeds size limit" in str(exc):
                raise

    body_chunks: list[bytes] = []
    total_bytes = 0
    async for chunk in response.aiter_bytes(chunk_size=CHUNK_SIZE):
        total_bytes += len(chunk)
        if total_bytes > MAX_RESPONSE_BYTES:
            raise ValueError(
                f"Response body exceeded size limit of {MAX_RESPONSE_BYTES} bytes"
            )
        body_chunks.append(chunk)

    return b"".join(body_chunks)


def _parse_and_check_url(url: str) -> tuple[str, str, int]:
    """Parse URL and check scheme and hostname."""
    parsed = urlparse(url)
    if parsed.scheme not in ("http", "https"):
        raise ValueError(
            f"Unsupported URL scheme '{parsed.scheme}'. Only http and https are allowed."
        )

    hostname = parsed.hostname
    if not hostname:
        raise ValueError(f"Invalid URL '{url}': missing hostname")

    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    return parsed.scheme, hostname, port


async def _fetch_single_url(
    target_url: str,
) -> tuple[str | None, str | None, dict[str, Any] | None]:
    """Perform one HTTP fetch step with IP pinning and redirect validation.

    Returns (redirect_url, error_message, success_result).
    """
    try:
        _, hostname, port = _parse_and_check_url(target_url)
        validated_ips = await resolve_and_validate_host(hostname, port)
    except ValueError as exc:
        return None, str(exc), None

    pinned_ip = validated_ips[0]
    transport = PinnedAsyncHTTPTransport(pinned_ip=pinned_ip, verify=True)
    headers = {
        "User-Agent": "NookDesktopAssistant/0.1.0",
        "Accept": "text/html,text/plain,text/markdown,application/json;q=0.9,*/*;q=0.5",
    }

    try:
        async with (
            httpx.AsyncClient(
                transport=transport,
                timeout=TIMEOUT_SECONDS,
                follow_redirects=False,
            ) as client,
            client.stream("GET", target_url, headers=headers) as response,
        ):
            if response.is_redirect:
                location = response.headers.get("location")
                if not location:
                    return None, "Redirect received with no Location header", None
                return urljoin(target_url, location), None, None

            if response.status_code >= 400:
                return (
                    None,
                    f"HTTP {response.status_code}: {response.reason_phrase}",
                    None,
                )

            raw_content_type = response.headers.get("content-type", "").lower()
            media_type = raw_content_type.split(";")[0].strip()
            if media_type not in ALLOWED_CONTENT_TYPES:
                return (
                    None,
                    f"Unsupported Content-Type '{raw_content_type}'. Must be text, markdown, HTML, or JSON.",
                    None,
                )

            body_bytes = await _read_streamed_body(response)
            extracted = extract_readable_text(media_type, body_bytes)
            return (
                None,
                None,
                {
                    "url": target_url,
                    "status_code": response.status_code,
                    "content": extracted[:100000],
                },
            )
    except asyncio.CancelledError:
        raise
    except ValueError as exc:
        return None, str(exc), None
    except httpx.TimeoutException:
        return None, "Request timed out", None
    except httpx.HTTPError as exc:
        return None, f"HTTP request failed: {type(exc).__name__}", None
    except Exception:
        return None, "An error occurred while fetching the requested URL", None


async def execute_web_fetch(url: str) -> str:
    """Fetch a public HTTP or HTTPS web page with SSRF protection, DNS pinning,

    streaming size limits, and redirect validation.
    """
    current_url = url.strip()

    for _ in range(MAX_REDIRECTS + 1):
        redirect_url, error_msg, result = await _fetch_single_url(current_url)
        if error_msg:
            return tool_error(error_msg)
        if result:
            return tool_result(result)
        if redirect_url:
            current_url = redirect_url

    return tool_error(f"Too many redirects (exceeded limit of {MAX_REDIRECTS})")
