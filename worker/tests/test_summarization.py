from deepagents.backends import StateBackend
from deepagents.middleware.summarization import SummarizationMiddleware
from langchain_core.messages import AnyMessage, HumanMessage
from langchain_core.messages.utils import count_tokens_approximately
from langchain_openai import ChatOpenAI
from pydantic import SecretStr

from nook_worker.agent import build_deep_agent
from nook_worker.config import WorkerConfig


def test_summarization_threshold_configuration():
    config = WorkerConfig(
        base_url="http://localhost:8000/v1",
        api_key=SecretStr("dummy"),
        model="dummy-model",
        summarize_at_tokens=500_000,
    )
    model = ChatOpenAI(
        base_url=config.base_url,
        api_key=config.api_key,
        model=config.model,
        use_responses_api=False,
    )
    agent = build_deep_agent(model, config)
    assert agent is not None


def test_summarization_trigger_behavior():
    backend = StateBackend()
    model = ChatOpenAI(
        base_url="http://localhost:8000/v1",
        api_key=SecretStr("dummy"),
        model="dummy-model",
        use_responses_api=False,
    )
    middleware = SummarizationMiddleware(
        model=model,
        backend=backend,
        trigger=("tokens", 25),
        keep=("messages", 1),
    )

    under_threshold: list[AnyMessage] = [HumanMessage(content="Hello.")]
    assert middleware._determine_cutoff_index(under_threshold) == 0

    over_threshold: list[AnyMessage] = [
        HumanMessage(
            content=(
                "This message has enough tokens to easily exceed the twenty-five "
                "token limit."
            )
        ),
        HumanMessage(content="Second message."),
        HumanMessage(content="Third message."),
    ]
    cutoff_index = middleware._determine_cutoff_index(over_threshold)
    assert cutoff_index == 2


def test_image_token_counting_constant():
    image_block = {
        "type": "image_url",
        "image_url": {"url": "data:image/png;base64," + "A" * 100_000},
    }
    msg_with_image = HumanMessage(content=[image_block])  # type: ignore[arg-type]
    msg_without_image = HumanMessage(content=[{"type": "text", "text": ""}])  # type: ignore[arg-type]

    tokens_with_image = count_tokens_approximately([msg_with_image])
    tokens_without_image = count_tokens_approximately([msg_without_image])

    diff = tokens_with_image - tokens_without_image
    assert diff == 85
