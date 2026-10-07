from collections.abc import AsyncIterator
from typing import Any, Protocol
from uuid import UUID, uuid4

import httpx
import openai
import psycopg
from deepagents import (
    GeneralPurposeSubagentProfile,
    HarnessProfile,
    create_deep_agent,
    register_harness_profile,
)
from deepagents.backends import StateBackend
from deepagents.middleware.summarization import SummarizationMiddleware
from langchain_core.messages import AIMessageChunk, HumanMessage
from langchain_core.tools import StructuredTool
from langchain_openai import ChatOpenAI
from langgraph.checkpoint.base import BaseCheckpointSaver

from nook_worker.attachments import AttachmentError, process_attachments_and_text
from nook_worker.config import WorkerConfig
from nook_worker.protocol import (
    ErrorInfo,
    EventMessage,
    MessageFinishedEvent,
    MessageStartedEvent,
    RunRequest,
    TextDeltaEvent,
)
from nook_worker.registry import ToolDefinition, default_registry
from nook_worker.tools.fetch import WebFetchInput, execute_web_fetch
from nook_worker.tools.search import WebSearchInput, execute_web_search

SYSTEM_PROMPT = (
    "You are Nook, a concise desktop assistant. "
    "You have access to web_search and web_fetch tools to look up current information. "
    "Always answer in Markdown."
)


class AgentRunner(Protocol):
    def run(
        self, request: RunRequest, message_id: UUID | None = None
    ) -> AsyncIterator[EventMessage]: ...


def map_exception_to_error_info(exc: Exception) -> ErrorInfo:
    match exc:
        case (
            openai.APIConnectionError()
            | openai.APITimeoutError()
            | httpx.ConnectError()
            | httpx.TimeoutException()
            | httpx.NetworkError()
        ):
            return ErrorInfo(
                code="endpoint_unreachable",
                message=str(exc),
                retryable=True,
            )
        case openai.APIStatusError() | httpx.HTTPStatusError():
            return ErrorInfo(
                code="endpoint_error",
                message=str(exc),
                retryable=False,
            )
        case psycopg.OperationalError():
            return ErrorInfo(
                code="database_unavailable",
                message=str(exc),
                retryable=True,
            )
        case AttachmentError():
            return ErrorInfo(
                code="attachment_invalid",
                message=str(exc),
                retryable=False,
            )
        case ValueError():
            return ErrorInfo(
                code="invalid_request",
                message=str(exc),
                retryable=False,
            )
        case _:
            return ErrorInfo(
                code="internal",
                message=str(exc),
                retryable=False,
            )


def create_model(config: WorkerConfig) -> ChatOpenAI:
    return ChatOpenAI(
        base_url=config.base_url,
        api_key=config.api_key,
        model=config.model,
        use_responses_api=False,
        profile={"max_input_tokens": config.max_input_tokens},
        streaming=True,
    )


def register_agent_profile(model_name: str) -> None:
    profile = HarnessProfile(
        excluded_tools=frozenset({"execute"}),
        general_purpose_subagent=GeneralPurposeSubagentProfile(enabled=False),
    )
    register_harness_profile("openai", profile)
    register_harness_profile(f"openai:{model_name}", profile)


def build_deep_agent(
    model: ChatOpenAI,
    config: WorkerConfig,
    checkpointer: BaseCheckpointSaver | None = None,
) -> Any:
    register_agent_profile(config.model)
    backend = StateBackend()
    custom_summarization = SummarizationMiddleware(
        model=model,
        backend=backend,
        trigger=("tokens", config.summarize_at_tokens),
    )

    tool_specs = [
        (
            "nook:web_search",
            "web_search",
            "Search the web using DuckDuckGo. Returns titles, links, and snippets.",
            WebSearchInput,
            {"network": True, "read_only": True},
            "allow",
            execute_web_search,
        ),
        (
            "nook:web_fetch",
            "web_fetch",
            "Fetch a public web page over HTTP or HTTPS and extract readable text.",
            WebFetchInput,
            {"network": True, "read_only": True},
            "ask_once_per_host",
            execute_web_fetch,
        ),
    ]

    tools = []
    for reg_name, tool_name, desc, schema, caps, tier, fn in tool_specs:
        default_registry.register(
            ToolDefinition(
                name=reg_name,
                description=desc,
                argument_schema=schema,
                capabilities=caps,
                approval_tier=tier,
                executor=fn,
            )
        )
        tools.append(
            StructuredTool.from_function(
                coroutine=fn,
                name=tool_name,
                description=desc,
                args_schema=schema,
            )
        )

    return create_deep_agent(
        model=model,
        tools=tools,
        backend=backend,
        checkpointer=checkpointer,
        middleware=[custom_summarization],
        system_prompt=SYSTEM_PROMPT,
    )


def _extract_text_delta(chunk: Any) -> str:
    if not isinstance(chunk, AIMessageChunk) or not chunk.content:
        return ""
    if isinstance(chunk.content, str):
        return chunk.content
    if isinstance(chunk.content, list):
        parts: list[str] = []
        for item in chunk.content:
            if isinstance(item, dict) and item.get("type") == "text":
                parts.append(str(item.get("text", "")))
            elif isinstance(item, str):
                parts.append(item)
        return "".join(parts)
    return ""


class RealAgentRunner:
    def __init__(self, agent: Any) -> None:
        self._agent = agent

    async def run(
        self, request: RunRequest, message_id: UUID | None = None
    ) -> AsyncIterator[EventMessage]:
        content = process_attachments_and_text(
            text=request.text,
            attachments=request.attachments,
        )
        input_message = HumanMessage(content=content)  # type: ignore[arg-type]

        msg_id = message_id or uuid4()
        yield MessageStartedEvent(
            request_id=request.request_id,
            message_id=msg_id,
        )

        stream_config = {
            "configurable": {
                "thread_id": str(request.chat_id),
            }
        }

        async for chunk, _metadata in self._agent.astream(
            {"messages": [input_message]},
            stream_mode="messages",
            config=stream_config,
        ):
            delta = _extract_text_delta(chunk)
            if delta:
                yield TextDeltaEvent(
                    request_id=request.request_id,
                    message_id=msg_id,
                    text=delta,
                )

        yield MessageFinishedEvent(
            request_id=request.request_id,
            message_id=msg_id,
            status="complete",
        )
