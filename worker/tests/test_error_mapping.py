from uuid import UUID

import httpx
import openai
import psycopg

from nook_worker.agent import map_exception_to_error_info
from nook_worker.attachments import AttachmentError
from nook_worker.protocol import Attachment


def test_map_connection_error():
    err = openai.APIConnectionError(request=None)  # type: ignore[arg-type]
    info = map_exception_to_error_info(err)
    assert info.code == "endpoint_unreachable"
    assert info.retryable is True


def test_map_http_status_error():
    req = httpx.Request("POST", "http://localhost:8000")
    resp = httpx.Response(404, request=req)
    err = openai.APIStatusError(message="Not found", response=resp, body=None)  # type: ignore[arg-type]
    info = map_exception_to_error_info(err)
    assert info.code == "endpoint_error"
    assert info.retryable is False


def test_map_database_error():
    err = psycopg.OperationalError("connection refused")
    info = map_exception_to_error_info(err)
    assert info.code == "database_unavailable"
    assert info.retryable is True


def test_map_attachment_error():
    att = Attachment(
        id=UUID("33333333-3333-3333-3333-333333333333"),
        kind="text",
        name="test.txt",
        mime="text/plain",
        size_bytes=10,
        path="/nonexistent",
    )
    err = AttachmentError("file not found", att)
    info = map_exception_to_error_info(err)
    assert info.code == "attachment_invalid"
    assert info.retryable is False


def test_map_generic_error():
    err = RuntimeError("something broke")
    info = map_exception_to_error_info(err)
    assert info.code == "internal"
    assert info.retryable is False
