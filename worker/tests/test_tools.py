import ipaddress
import json

import pytest
from nook_worker.tools.fetch import execute_web_fetch, is_ip_blocked
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
    assert "Blocked target address" in res["error"] or "Failed to resolve host" in res["error"]


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
