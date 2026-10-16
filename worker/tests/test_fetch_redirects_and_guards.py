import asyncio
import json
import socket
from collections.abc import Callable, Iterator
from typing import Any
from unittest.mock import patch

import httpx
import pytest

from nook_worker.tools.fetch import (
    MAX_REDIRECTS,
    execute_web_fetch,
    resolve_and_validate_host,
)

PUBLIC_IP = "93.184.215.14"


def redirect_to(location: str) -> httpx.Response:
    return httpx.Response(302, headers={"location": location})


def page(text: str = "hello") -> httpx.Response:
    return httpx.Response(
        200, headers={"content-type": "text/plain"}, content=text.encode()
    )


class FakeNetwork:
    def __init__(self, routes: dict[str, httpx.Response]) -> None:
        self.routes = {
            str(httpx.URL(url)): response for url, response in routes.items()
        }
        self.requested_urls: list[str] = []

    def handler(self, request: httpx.Request) -> httpx.Response:
        self.requested_urls.append(str(request.url))
        return self.routes[str(request.url)]

    def transport_factory(self, pinned_ip: str, **_: Any) -> httpx.MockTransport:
        return httpx.MockTransport(self.handler)

    async def resolve(self, hostname: str, port: int) -> list[str]:
        return [PUBLIC_IP]


NetworkBuilder = Callable[[dict[str, httpx.Response]], FakeNetwork]


@pytest.fixture
def network_for() -> Iterator[NetworkBuilder]:
    def build(routes: dict[str, httpx.Response]) -> FakeNetwork:
        network = FakeNetwork(routes)
        patcher_transport = patch(
            "nook_worker.tools.fetch.PinnedAsyncHTTPTransport",
            side_effect=network.transport_factory,
        )
        patcher_resolve = patch(
            "nook_worker.tools.fetch.resolve_and_validate_host",
            side_effect=network.resolve,
        )
        patcher_transport.start()
        patcher_resolve.start()
        patchers.extend([patcher_transport, patcher_resolve])
        return network

    patchers: list[Any] = []
    yield build
    for patcher in patchers:
        patcher.stop()


async def test_same_host_redirect_is_followed(network_for: NetworkBuilder):
    network = network_for(
        {
            "https://example.com/a": redirect_to("/b"),
            "https://example.com/b": page("final"),
        }
    )

    result = json.loads(await execute_web_fetch("https://example.com/a"))

    assert result["content"] == "final"
    assert network.requested_urls == ["https://example.com/a", "https://example.com/b"]


async def test_http_to_https_redirect_on_same_host_is_followed(
    network_for: NetworkBuilder,
):
    network = network_for(
        {
            "http://example.com/": redirect_to("https://example.com/"),
            "https://example.com/": page("secure"),
        }
    )

    result = json.loads(await execute_web_fetch("http://example.com/"))

    assert result["content"] == "secure"
    assert network.requested_urls == ["http://example.com/", "https://example.com/"]


async def test_redirect_to_other_port_on_same_host_is_followed(
    network_for: NetworkBuilder,
):
    network = network_for(
        {
            "https://example.com/": redirect_to("https://example.com:8443/x"),
            "https://example.com:8443/x": page("ported"),
        }
    )

    result = json.loads(await execute_web_fetch("https://example.com/"))

    assert result["content"] == "ported"
    assert len(network.requested_urls) == 2


async def test_cross_host_redirect_is_refused_without_a_second_request(
    network_for: NetworkBuilder,
):
    network = network_for(
        {"https://example.com/": redirect_to("https://other.example.org/x")}
    )

    result = json.loads(await execute_web_fetch("https://example.com/"))

    assert result == {
        "error": "Redirected to other.example.org. Ask for that URL directly so it can be approved."
    }
    assert network.requested_urls == ["https://example.com/"]


async def test_subdomain_redirect_is_a_different_host(network_for: NetworkBuilder):
    network = network_for(
        {"https://example.com/": redirect_to("https://www.example.com/")}
    )

    result = json.loads(await execute_web_fetch("https://example.com/"))

    assert result == {
        "error": "Redirected to www.example.com. Ask for that URL directly so it can be approved."
    }
    assert network.requested_urls == ["https://example.com/"]


async def test_redirect_with_uppercase_host_and_trailing_dot_is_same_host(
    network_for: NetworkBuilder,
):
    network = network_for(
        {
            "https://example.com/": redirect_to("https://EXAMPLE.COM./next"),
            "https://EXAMPLE.COM./next": page("same"),
        }
    )

    result = json.loads(await execute_web_fetch("https://example.com/"))

    assert result["content"] == "same"
    assert len(network.requested_urls) == 2


async def test_original_host_with_trailing_dot_matches_plain_redirect(
    network_for: NetworkBuilder,
):
    network_for(
        {
            "https://example.com./": redirect_to("https://example.com/next"),
            "https://example.com/next": page("same"),
        }
    )

    result = json.loads(await execute_web_fetch("https://example.com./"))

    assert result["content"] == "same"


async def test_internationalized_host_matches_its_punycode_form(
    network_for: NetworkBuilder,
):
    network_for(
        {
            "https://bücher.example/": redirect_to("https://xn--bcher-kva.example/x"),
            "https://xn--bcher-kva.example/x": page("idna"),
        }
    )

    result = json.loads(await execute_web_fetch("https://bücher.example/"))

    assert result["content"] == "idna"


@pytest.mark.parametrize(
    "location",
    ["ftp://example.com/file", "file:///etc/passwd", "javascript:alert(1)"],
)
async def test_redirect_to_non_http_scheme_is_refused(
    network_for: NetworkBuilder, location: str
):
    network = network_for({"https://example.com/": redirect_to(location)})

    result = json.loads(await execute_web_fetch("https://example.com/"))

    assert "error" in result
    assert network.requested_urls == ["https://example.com/"]


async def test_redirect_loop_on_same_host_stops_at_the_limit(
    network_for: NetworkBuilder,
):
    network = network_for({"https://example.com/": redirect_to("https://example.com/")})

    result = json.loads(await execute_web_fetch("https://example.com/"))

    assert "Too many redirects" in result["error"]
    assert len(network.requested_urls) == MAX_REDIRECTS + 1


@pytest.mark.parametrize(
    "url",
    [
        "http://127.0.0.1/",
        "http://10.1.2.3/",
        "http://192.168.0.10/",
        "http://169.254.169.254/latest/meta-data/",
        "http://[::1]/",
        "http://[::ffff:127.0.0.1]/",
        "http://0.0.0.0/",
        "http://100.100.100.200/",
    ],
)
async def test_literal_private_addresses_are_refused_before_any_connection(url: str):
    with patch("nook_worker.tools.fetch.PinnedAsyncHTTPTransport") as transport:
        result = json.loads(await execute_web_fetch(url))

    assert "Blocked target address" in result["error"]
    transport.assert_not_called()


def fake_getaddrinfo(*addresses: str) -> Callable[..., Any]:
    async def getaddrinfo(host: str, port: int, **_: Any) -> list[Any]:
        return [
            (
                socket.AF_INET6 if ":" in address else socket.AF_INET,
                socket.SOCK_STREAM,
                6,
                "",
                (address, port),
            )
            for address in addresses
        ]

    return getaddrinfo


async def test_hostname_resolving_to_private_ip_is_refused_before_any_connection(
    monkeypatch: pytest.MonkeyPatch,
):
    monkeypatch.setattr(
        asyncio.get_running_loop(), "getaddrinfo", fake_getaddrinfo("10.0.0.5")
    )
    with patch("nook_worker.tools.fetch.PinnedAsyncHTTPTransport") as transport:
        result = json.loads(await execute_web_fetch("https://internal.example/"))

    assert "Blocked target address '10.0.0.5'" in result["error"]
    transport.assert_not_called()


async def test_hostname_resolving_to_public_and_private_ips_is_refused(
    monkeypatch: pytest.MonkeyPatch,
):
    monkeypatch.setattr(
        asyncio.get_running_loop(),
        "getaddrinfo",
        fake_getaddrinfo(PUBLIC_IP, "127.0.0.1"),
    )

    with pytest.raises(ValueError, match="Blocked target address '127.0.0.1'"):
        await resolve_and_validate_host("rebind.example", 443)


async def test_hostname_resolving_to_public_ip_passes_the_guard(
    monkeypatch: pytest.MonkeyPatch,
):
    monkeypatch.setattr(
        asyncio.get_running_loop(), "getaddrinfo", fake_getaddrinfo(PUBLIC_IP)
    )

    assert await resolve_and_validate_host("example.com", 443) == [PUBLIC_IP]


async def test_literal_public_ip_passes_the_guard():
    assert await resolve_and_validate_host("8.8.8.8", 443) == ["8.8.8.8"]
