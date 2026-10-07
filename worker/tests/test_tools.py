import ipaddress
import json
from unittest.mock import AsyncMock, patch

import pytest

from nook_worker.tools.fetch import (
    PinnedAsyncHTTPTransport,
    PinnedAsyncNetworkBackend,
    execute_web_fetch,
    is_ip_blocked,
)
from nook_worker.tools.search import execute_web_search


def test_ip_blocking():
    assert is_ip_blocked(ipaddress.ip_address("127.0.0.1"))
    assert is_ip_blocked(ipaddress.ip_address("::1"))
    assert is_ip_blocked(ipaddress.ip_address("10.0.0.1"))
    assert is_ip_blocked(ipaddress.ip_address("192.168.1.1"))
    assert is_ip_blocked(ipaddress.ip_address("172.16.0.1"))
    assert is_ip_blocked(ipaddress.ip_address("169.254.169.254"))
    assert not is_ip_blocked(ipaddress.ip_address("1.1.1.1"))
    assert not is_ip_blocked(ipaddress.ip_address("8.8.8.8"))


@pytest.mark.asyncio
@pytest.mark.parametrize(
    "url",
    [
        "http://localhost:8000/secret",
        "http://127.0.0.1:8080",
        "http://192.168.1.1/admin",
    ],
)
async def test_fetch_blocks_private_addresses(url: str):
    res = json.loads(await execute_web_fetch(url))
    assert "error" in res
    assert (
        "Blocked target address" in res["error"]
        or "Failed to resolve host" in res["error"]
    )


@pytest.mark.asyncio
async def test_fetch_rejects_non_http_schemes():
    res = json.loads(await execute_web_fetch("file:///etc/passwd"))
    assert "error" in res
    assert "Unsupported URL scheme" in res["error"]


@pytest.mark.asyncio
async def test_web_search_empty_query():
    res = json.loads(await execute_web_search("   "))
    assert "error" in res
    assert "Empty search query" in res["error"]


@pytest.mark.asyncio
async def test_web_search_error_handling_sanitizes_message():
    with patch(
        "nook_worker.tools.search._run_ddgs_sync",
        side_effect=RuntimeError("internal private IP 192.168.1.1 error details"),
    ):
        res = json.loads(await execute_web_search("some valid query"))
        assert "error" in res
        assert res["error"] == "Search provider error: request failed"
        assert "192.168.1.1" not in res["error"]


@pytest.mark.asyncio
async def test_pinned_async_network_backend_connects_to_target_ip():
    backend = PinnedAsyncNetworkBackend(target_ip="93.184.215.14")
    mock_base = AsyncMock()
    backend._base = mock_base

    await backend.connect_tcp("example.com", 443, timeout=5.0)

    mock_base.connect_tcp.assert_awaited_once_with(
        "93.184.215.14",
        443,
        timeout=5.0,
        local_address=None,
        socket_options=None,
    )


@pytest.mark.asyncio
async def test_pinned_async_http_transport_configures_backend():
    transport = PinnedAsyncHTTPTransport(pinned_ip="93.184.215.14")
    # Verify pool has the pinned network backend
    backend = getattr(transport._pool, "_network_backend", None)
    assert isinstance(backend, PinnedAsyncNetworkBackend)
    assert backend.target_ip == "93.184.215.14"
