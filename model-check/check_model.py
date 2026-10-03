"""Checks whether a hosted OpenAI-compatible model works as a Deep Agents backend.

Reads NOOK_BASE_URL, NOOK_API_KEY and NOOK_MODEL from the environment. Each step
runs only if the previous one passed.
"""

import json
import os
import sys
from collections.abc import Callable

from deepagents import create_deep_agent
from langchain.agents.middleware import TodoListMiddleware
from langchain_core.messages import AIMessage
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI
from openai import OpenAI
from openai.types.chat import ChatCompletionMessageParam, ChatCompletionToolParam
from pydantic import SecretStr

WEATHER_TOOL: ChatCompletionToolParam = {
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "Get the current weather for a city.",
        "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"],
        },
    },
}
WEATHER_MESSAGES: list[ChatCompletionMessageParam] = [
    {"role": "user", "content": "What is the weather in Paris right now?"}
]
SECRET_CODE = "7341-ZK"
AGENT_RECURSION_LIMIT = 30
RED_PNG_B64 = "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAIAAAD8GO2jAAAAKElEQVR4nO3NsQ0AAAzCMP5/un0CNkuZ41wybXsHAAAAAAAAAAAAxR4yw/wuPL6QkAAAAABJRU5ErkJggg=="


class CheckFailed(Exception):
    def __init__(self, reason: str, raw: object = None):
        super().__init__(reason)
        self.raw = raw


def load_config() -> tuple[str, str, str]:
    names = ("NOOK_BASE_URL", "NOOK_API_KEY", "NOOK_MODEL")
    missing = [name for name in names if not os.environ.get(name)]
    if missing:
        sys.exit(f"Missing environment variables: {', '.join(missing)}")
    return (os.environ[names[0]], os.environ[names[1]], os.environ[names[2]])


def parse_weather_arguments(arguments: str) -> dict:
    try:
        parsed = json.loads(arguments)
    except json.JSONDecodeError as error:
        raise CheckFailed(
            f"tool arguments are not valid JSON: {error}", arguments
        ) from error
    if not isinstance(parsed, dict) or not parsed.get("city"):
        raise CheckFailed("tool arguments have no 'city' field", arguments)
    return parsed


def check_raw_tool_call(base_url: str, api_key: str, model: str) -> str:
    client = OpenAI(base_url=base_url, api_key=api_key)
    response = client.chat.completions.create(
        model=model, messages=WEATHER_MESSAGES, tools=[WEATHER_TOOL]
    )
    message = response.choices[0].message
    if not message.tool_calls:
        raise CheckFailed("response has no tool_calls", response.model_dump())
    call = message.tool_calls[0]
    func = getattr(call, "function", None)
    if not func:
        raise CheckFailed("tool call has no function", response.model_dump())
    arguments = parse_weather_arguments(func.arguments)
    return f"{func.name}({arguments})"


def check_streaming_tool_call(base_url: str, api_key: str, model: str) -> str:
    client = OpenAI(base_url=base_url, api_key=api_key)
    stream = client.chat.completions.create(
        model=model, messages=WEATHER_MESSAGES, tools=[WEATHER_TOOL], stream=True
    )
    chunks = []
    calls: dict[int, dict[str, str]] = {}
    for chunk in stream:
        chunks.append(chunk.model_dump())
        if not chunk.choices:
            continue
        for delta in chunk.choices[0].delta.tool_calls or []:
            entry = calls.setdefault(delta.index, {"name": "", "arguments": ""})
            if delta.function and delta.function.name:
                entry["name"] += delta.function.name
            if delta.function and delta.function.arguments:
                entry["arguments"] += delta.function.arguments
    if not calls:
        raise CheckFailed("stream has no tool-call deltas", chunks)
    first = calls[min(calls)]
    arguments = parse_weather_arguments(first["arguments"])
    return f"{first['name']}({arguments}) assembled from stream"


@tool
def get_secret_code(label: str) -> str:
    """Return the secret code for a label. The code cannot be guessed."""
    return SECRET_CODE


def tool_names_called(messages: list) -> list[str]:
    return [
        call["name"]
        for message in messages
        if isinstance(message, AIMessage)
        for call in message.tool_calls
    ]


def check_deep_agent(base_url: str, api_key: str, model: str) -> str:
    llm = ChatOpenAI(
        base_url=base_url,
        api_key=SecretStr(api_key),
        model=model,
        use_responses_api=False,
    )
    agent = create_deep_agent(
        model=llm,
        tools=[get_secret_code],
        middleware=[TodoListMiddleware()],
        system_prompt="You are a careful assistant. Plan with write_todos before acting.",
    )
    result = agent.invoke(
        {
            "messages": [
                {
                    "role": "user",
                    "content": (
                        "Make a todo list for this task, then do it. Look up the secret "
                        "code for the label 'nook' with get_secret_code, and tell me the code."
                    ),
                }
            ]
        },
        {"recursion_limit": AGENT_RECURSION_LIMIT},
    )
    messages = result["messages"]
    called = tool_names_called(messages)
    raw = [message.model_dump() for message in messages]
    if "get_secret_code" not in called:
        raise CheckFailed(f"agent never called get_secret_code (calls: {called})", raw)
    if "write_todos" not in called:
        raise CheckFailed(f"agent never called write_todos (calls: {called})", raw)
    if not result.get("todos"):
        raise CheckFailed("agent state has an empty todo list", raw)
    final = messages[-1]
    if not isinstance(final, AIMessage) or not final.content:
        raise CheckFailed("last message is not a non-empty AI answer", raw)
    if SECRET_CODE not in str(final.content):
        raise CheckFailed(f"final answer does not contain {SECRET_CODE}", raw)
    return f"tool calls: {called}; todos: {len(result['todos'])}"


def check_image_input(base_url: str, api_key: str, model: str) -> str:
    client = OpenAI(base_url=base_url, api_key=api_key)
    response = client.chat.completions.create(
        model=model,
        messages=[
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What is the single dominant color of this image? Reply with just the color name.",
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": f"data:image/png;base64,{RED_PNG_B64}",
                        },
                    },
                ],
            }
        ],
    )
    message = response.choices[0].message
    content = str(message.content or "").strip().lower()
    if not content:
        raise CheckFailed("response content is empty", response.model_dump())
    if "red" not in content:
        raise CheckFailed(
            f"response does not identify the color red: {content!r}",
            response.model_dump(),
        )
    return f"identified color: {content}"


def main() -> int:
    config = load_config()
    steps: list[tuple[str, Callable[[str, str, str], str]]] = [
        ("1. raw tool call", check_raw_tool_call),
        ("2. streaming tool call", check_streaming_tool_call),
        ("3. deep agent", check_deep_agent),
        ("4. image input", check_image_input),
    ]
    for name, step in steps:
        try:
            detail = step(*config)
        except CheckFailed as failure:
            print(f"FAIL {name}: {failure}")
            print(json.dumps(failure.raw, indent=2, default=str))
            return 1
        except Exception as error:  # noqa: BLE001
            print(f"FAIL {name}: {type(error).__name__}: {error}")
            return 1
        print(f"PASS {name}: {detail}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
