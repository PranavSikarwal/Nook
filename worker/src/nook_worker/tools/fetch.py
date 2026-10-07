import asyncio
import ipaddress
import socket
from urllib.parse import urljoin, urlparse

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
    """Read response body up to MAX_RESPONSE_BYTES chunks."""
    content_length = response.headers.get("content-length")
    if content_length and int(content_length) > MAX_RESPONSE_BYTES:
        raise ValueError(f"Response exceeds size limit of {MAX_RESPONSE_BYTES} bytes")

    body_chunks: list[bytes] = []
    total_bytes = 0
    async for chunk in response.aiter_bytes(chunk_size=CHUNK_SIZE):
        total_bytes += len(chunk)
        if total_bytes > MAX_RESPONSE_BYTES:
            allowed_slice = chunk[: len(chunk) - (total_bytes - MAX_RESPONSE_BYTES)]
            body_chunks.append(allowed_slice)
            break
        body_chunks.append(chunk)

    return b"".join(body_chunks)


async def _perform_single_get(
    connect_url: str,
    hostname: str,
    headers: dict[str, str],
) -> tuple[httpx.Response | None, str | None]:
    """Execute a single HTTP stream request and check for redirect or error."""
    async with httpx.AsyncClient(
        verify=True,
        timeout=TIMEOUT_SECONDS,
        follow_redirects=False,
    ) as client:
        req = client.build_request(
            "GET",
            connect_url,
            headers=headers,
            extensions={"sni_hostname": hostname},
        )
        response = await client.send(req, stream=True)
        return response, None


async def execute_web_fetch(url: str) -> str:
    """Fetch a public HTTP or HTTPS web page with SSRF protection, DNS pinning,

    streaming size limits, and redirect validation.
    """
    current_url = url.strip()
    redirect_count = 0

    while redirect_count <= MAX_REDIRECTS:
        parsed = urlparse(current_url)
        if parsed.scheme not in ("http", "https"):
            return tool_error(
                f"Unsupported URL scheme '{parsed.scheme}'. Only http and https are allowed."
            )

        hostname = parsed.hostname
        if not hostname:
            return tool_error(f"Invalid URL '{current_url}': missing hostname")

        port = parsed.port or (443 if parsed.scheme == "https" else 80)

        try:
            validated_ips = await resolve_and_validate_host(hostname, port)
        except ValueError as exc:
            return tool_error(str(exc))

        pinned_ip = validated_ips[0]
        connect_host = f"[{pinned_ip}]" if ":" in pinned_ip else pinned_ip
        target_path = parsed.path or "/"
        if parsed.query:
            target_path = f"{target_path}?{parsed.query}"

        connect_url = f"{parsed.scheme}://{connect_host}:{port}{target_path}"
        headers = {
            "Host": hostname if port in (80, 443) else f"{hostname}:{port}",
            "User-Agent": "NookDesktopAssistant/0.1.0",
            "Accept": "text/html,text/plain,text/markdown,application/json;q=0.9,*/*;q=0.5",
        }

        try:
            response, _ = await _perform_single_get(connect_url, hostname, headers)
            if response is None:
                return tool_error("Failed to connect")

            try:
                if response.is_redirect:
                    location = response.headers.get("location")
                    if not location:
                        return tool_error("Redirect received with no Location header")
                    current_url = urljoin(current_url, location)
                    redirect_count += 1
                    continue

                if response.status_code >= 400:
                    return tool_error(
                        f"HTTP {response.status_code}: {response.reason_phrase}"
                    )

                content_type = response.headers.get("content-type", "").lower()
                if not any(t in content_type for t in ALLOWED_CONTENT_TYPES):
                    return tool_error(
                        f"Unsupported Content-Type '{content_type}'. Must be text, markdown, HTML, or JSON."
                    )

                body_bytes = await _read_streamed_body(response)
                extracted = extract_readable_text(content_type, body_bytes)
                return tool_result(
                    {
                        "url": current_url,
                        "status_code": response.status_code,
                        "content": extracted[:100000],
                    }
                )
            finally:
                await response.aclose()
        except asyncio.CancelledError:
            raise
        except ValueError as exc:
            return tool_error(str(exc))
        except httpx.TimeoutException:
            return tool_error("Request timed out")
        except httpx.HTTPError as exc:
            return tool_error(f"HTTP request failed: {type(exc).__name__}")
        except Exception:
            return tool_error("An error occurred while fetching the requested URL")

    return tool_error(f"Too many redirects (exceeded limit of {MAX_REDIRECTS})")
