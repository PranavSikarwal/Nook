from langchain_core.language_models.chat_models import BaseChatModel
from langchain_core.messages import HumanMessage, SystemMessage

_TITLE_SYSTEM_PROMPT = (
    "Generate a concise title of at most six words summarizing the user's question. "
    "Reply with only the title text, with no quotes, formatting, or punctuation."
)


def clean_title_text(raw: str) -> str:
    cleaned = raw.strip().strip("\"'`").rstrip(".:;,!?").strip()

    words = cleaned.split()
    if not words:
        return "New Chat"

    if len(words) > 6:
        words = words[:6]

    return " ".join(words)


def _extract_content_text(content: object) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts: list[str] = []
        for item in content:
            if isinstance(item, dict) and item.get("type") == "text":
                parts.append(str(item.get("text", "")))
            elif isinstance(item, str):
                parts.append(item)
        return "".join(parts)
    return str(content)


async def generate_title(model: BaseChatModel, first_message: str) -> str:
    messages = [
        SystemMessage(content=_TITLE_SYSTEM_PROMPT),
        HumanMessage(content=first_message),
    ]
    response = await model.ainvoke(messages)
    raw_content = _extract_content_text(response.content)
    return clean_title_text(raw_content)
