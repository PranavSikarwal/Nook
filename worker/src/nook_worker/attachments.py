import base64
from pathlib import Path
from typing import Any

from pypdf import PdfReader

from nook_worker.protocol import Attachment

MAX_FILE_BYTES = 10 * 1024 * 1024
MAX_EXTRACTED_CHARS = 200_000
TRUNCATED_MARKER = "\n[truncated]"


class AttachmentError(Exception):
    def __init__(self, message: str, attachment: Attachment) -> None:
        super().__init__(message)
        self.attachment = attachment


def _process_image(path: Path, mime: str, att: Attachment) -> dict[str, Any]:
    try:
        raw_bytes = path.read_bytes()
    except OSError as err:
        raise AttachmentError(f"Cannot read image file: {err}", att) from err

    encoded = base64.b64encode(raw_bytes).decode("ascii")
    return {
        "type": "image_url",
        "image_url": {"url": f"data:{mime};base64,{encoded}"},
    }


def _process_text(path: Path, name: str, att: Attachment) -> dict[str, Any]:
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except OSError as err:
        raise AttachmentError(f"Cannot read text file: {err}", att) from err

    if len(content) > MAX_EXTRACTED_CHARS:
        content = content[:MAX_EXTRACTED_CHARS] + TRUNCATED_MARKER

    formatted = f"Attachment {name}:\n```\n{content}\n```"
    return {"type": "text", "text": formatted}


def _process_pdf(path: Path, name: str, att: Attachment) -> dict[str, Any]:
    try:
        reader = PdfReader(str(path))
        extracted_pages: list[str] = []
        total_chars = 0
        for page in reader.pages:
            page_text = page.extract_text() or ""
            extracted_pages.append(page_text)
            total_chars += len(page_text)
            if total_chars >= MAX_EXTRACTED_CHARS:
                break
        content = "\n".join(extracted_pages)
    except Exception as err:
        raise AttachmentError(f"Cannot read PDF file: {err}", att) from err

    if len(content) > MAX_EXTRACTED_CHARS:
        content = content[:MAX_EXTRACTED_CHARS] + TRUNCATED_MARKER

    formatted = f"Attachment {name} (PDF extracted text):\n```\n{content}\n```"
    return {"type": "text", "text": formatted}


def process_attachment(att: Attachment) -> dict[str, Any]:
    path = Path(att.path)
    if not path.is_file():
        raise AttachmentError(f"Attachment file not found: {att.path}", att)

    if path.stat().st_size > MAX_FILE_BYTES:
        raise AttachmentError(
            f"Attachment exceeds 10 MB limit ({path.stat().st_size} bytes): {att.name}",
            att,
        )

    match att.kind:
        case "image":
            return _process_image(path, att.mime, att)
        case "text":
            return _process_text(path, att.name, att)
        case "pdf":
            return _process_pdf(path, att.name, att)
        case _:
            raise AttachmentError(f"Unsupported attachment kind: {att.kind}", att)


def process_attachments_and_text(
    text: str,
    attachments: list[Attachment],
) -> str | list[dict[str, Any]]:
    if not attachments:
        return text

    blocks: list[dict[str, Any]] = []
    if text:
        blocks.append({"type": "text", "text": text})

    for att in attachments:
        block = process_attachment(att)
        blocks.append(block)

    return blocks
