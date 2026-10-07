import ipaddress
import json

import pytest

from nook_worker.tools.fetch import execute_web_fetch, is_ip_blocked
from nook_worker.tools.search import execute_web_search


def test_ip_blocking():
    # Loopback
    assert is_ip_blocked(ipaddress.ip_address("127.0.0.1"))
    assert is_ip_blocked(ipaddress.ip_address("::1"))

    # Private RFC 1918
    assert is_ip_blocked(ipaddress.ip_address("10.0.0.1"))
    assert is_ip_blocked(ipaddress.ip_address("192.168.1.1"))
    assert is_ip_blocked(ipaddress.ip_address("172.16.0.1"))

    # Link-local / AWS metadata
    assert is_ip_blocked(ipaddress.ip_address("169.254.169.254"))

    # Public IP (should NOT be blocked)
    assert not is_ip_blocked(ipaddress.ip_address("1.1.1.1"))
    assert not is_ip_blocked(ipaddress.ip_address("8.8.8.8"))


@pytest.mark.asyncio
async def test_fetch_blocks_private_and_localhost():
    # Attempting localhost
    res_str = await execute_web_fetch("http://localhost:8000/secret")
    res = json.loads(res_str)
    assert "error" in res
    assert (
        "Blocked target address" in res["error"]
        or "Failed to resolve host" in res["error"]
    )

    # Attempting 127.0.0.1
    res_str2 = await execute_web_fetch("http://127.0.0.1:8080")
    res2 = json.loads(res_str2)
    assert "error" in res2
    assert "Blocked target address" in res2["error"]

    # Attempting private 192.168.1.1
    res_str3 = await execute_web_fetch("http://192.168.1.1/admin")
    res3 = json.loads(res_str3)
    assert "error" in res3
    assert "Blocked target address" in res3["error"]


@pytest.mark.asyncio
async def test_fetch_rejects_non_http_schemes():
    res_str = await execute_web_fetch("file:///etc/passwd")
    res = json.loads(res_str)
    assert "error" in res
    assert "Unsupported URL scheme" in res["error"]


@pytest.mark.asyncio
async def test_web_search_empty_query():
    res_str = await execute_web_search("   ")
    res = json.loads(res_str)
    assert "error" in res
    assert "Empty search query" in res["error"]
