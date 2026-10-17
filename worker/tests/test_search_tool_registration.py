import pytest
from pydantic import SecretStr

from nook_worker.agent import build_deep_agent, create_model
from nook_worker.config import WorkerConfig
from nook_worker.registry import ToolArgumentError, default_registry

SEARCH_TOOLS = [
    "nook:web_search",
    "nook:news_search",
    "nook:video_search",
    "nook:book_search",
]


@pytest.fixture(scope="module", autouse=True)
def agent():
    config = WorkerConfig(
        base_url="http://127.0.0.1:9/v1", api_key=SecretStr("unused"), model="unused"
    )
    return build_deep_agent(create_model(config), config)


@pytest.mark.parametrize("name", [*SEARCH_TOOLS, "nook:web_fetch"])
def test_every_worker_tool_is_registered(name: str):
    assert default_registry.get(name) is not None


@pytest.mark.parametrize("name", SEARCH_TOOLS)
def test_a_search_tool_trims_its_query_and_defaults_the_count(name: str):
    arguments = default_registry.validate_arguments(name, {"query": "  cats  "})

    assert arguments == {"query": "cats", "max_results": 5}


@pytest.mark.parametrize("name", SEARCH_TOOLS)
@pytest.mark.parametrize(
    "arguments",
    [
        {},
        {"query": "  "},
        {"query": "x", "max_results": 0},
        {"query": "x", "max_results": 11},
        {"query": 5},
    ],
)
def test_a_search_tool_rejects_bad_arguments(name: str, arguments: dict):
    with pytest.raises(ToolArgumentError):
        default_registry.validate_arguments(name, arguments)
