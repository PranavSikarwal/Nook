import base64
from pathlib import Path
from uuid import uuid4

import pytest
from nook_worker.attachments import (
    AttachmentError,
    process_attachment,
    process_attachments_and_text,
)
from nook_worker.protocol import Attachment


def test_process_image_attachment(tmp_path: Path):
    img_file = tmp_path / "test.png"
    sample_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"
    img_file.write_bytes(sample_bytes)

    att = Attachment(
        id=uuid4(),
        kind="image",
        name="test.png",
        mime="image/png",
        size_bytes=len(sample_bytes),
        path=str(img_file),
    )

    block = process_attachment(att)
    assert block["type"] == "image_url"
    expected_b64 = base64.b64encode(sample_bytes).decode("ascii")
    assert block["image_url"]["url"] == f"data:image/png;base64,{expected_b64}"


def test_process_text_attachment(tmp_path: Path):
    txt_file = tmp_path / "notes.txt"
    txt_file.write_text("Meeting notes:\n- Item 1\n- Item 2", encoding="utf-8")

    att = Attachment(
        id=uuid4(),
        kind="text",
        name="notes.txt",
        mime="text/plain",
        size_bytes=txt_file.stat().st_size,
        path=str(txt_file),
    )

    block = process_attachment(att)
    assert block["type"] == "text"
    assert "Attachment notes.txt:" in block["text"]
    assert "Meeting notes:" in block["text"]


def test_process_text_attachment_truncation(tmp_path: Path):
    txt_file = tmp_path / "large.txt"
    content = "A" * 300_000
    txt_file.write_text(content, encoding="utf-8")

    att = Attachment(
        id=uuid4(),
        kind="text",
        name="large.txt",
        mime="text/plain",
        size_bytes=txt_file.stat().st_size,
        path=str(txt_file),
    )

    block = process_attachment(att)
    assert block["type"] == "text"
    assert "[truncated]" in block["text"]
    # 200,000 characters from content + header + marker
    assert len(block["text"]) < 201_000


def test_process_attachment_invalid_path(tmp_path: Path):
    att = Attachment(
        id=uuid4(),
        kind="text",
        name="missing.txt",
        mime="text/plain",
        size_bytes=100,
        path=str(tmp_path / "nonexistent.txt"),
    )
    with pytest.raises(AttachmentError):
        process_attachment(att)


def test_process_attachment_exceeds_max_size(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    txt_file = tmp_path / "oversized.txt"
    txt_file.write_text("Hello", encoding="utf-8")

    from unittest.mock import MagicMock

    stat_mock = MagicMock()
    stat_mock.st_size = 11 * 1024 * 1024
    monkeypatch.setattr(Path, "stat", lambda _: stat_mock)

    att = Attachment(
        id=uuid4(),
        kind="text",
        name="oversized.txt",
        mime="text/plain",
        size_bytes=100,
        path=str(txt_file),
    )
    with pytest.raises(AttachmentError) as exc_info:
        process_attachment(att)
    assert "exceeds 10 MB limit" in str(exc_info.value)


def test_process_attachments_and_text_combined(tmp_path: Path):
    txt_file = tmp_path / "snippet.py"
    txt_file.write_text("print('hello')", encoding="utf-8")

    att = Attachment(
        id=uuid4(),
        kind="text",
        name="snippet.py",
        mime="text/x-python",
        size_bytes=txt_file.stat().st_size,
        path=str(txt_file),
    )

    content = process_attachments_and_text(
        text="Can you review this code?",
        attachments=[att],
    )
    assert isinstance(content, list)
    assert content[0] == {"type": "text", "text": "Can you review this code?"}
    assert content[1]["type"] == "text"
    assert "print('hello')" in content[1]["text"]
