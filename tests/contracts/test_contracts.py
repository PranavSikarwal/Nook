"""Contract tests for Nook inter-process protocols.

Validates that all example JSON lines match their respective schemas, all message
types have examples, and invalid payloads are rejected.
"""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft7Validator, FormatChecker, ValidationError

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CONTRACTS_DIR = REPO_ROOT / "contracts"
EXAMPLES_DIR = CONTRACTS_DIR / "examples"

format_checker = FormatChecker()


@format_checker.checks("date-time")
def _check_datetime(val: Any) -> bool:
    if not isinstance(val, str):
        return False
    try:
        datetime.fromisoformat(val.replace("Z", "+00:00"))
        return True
    except (ValueError, TypeError):
        return False


def load_json(path: Path) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_ndjson(path: Path) -> list[dict[str, Any]]:
    lines: list[dict[str, Any]] = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            stripped = line.strip()
            if stripped:
                lines.append(json.loads(stripped))
    return lines


def extract_expected_types(schema: dict[str, Any]) -> set[str]:
    """Extract expected message types by resolving top-level oneOf references."""
    expected: set[str] = set()
    definitions = schema.get("definitions", {})
    for item in schema.get("oneOf", []):
        ref = item.get("$ref", "")
        if ref.startswith("#/definitions/"):
            def_name = ref.split("/")[-1]
            def_body = definitions.get(def_name, {})
            props = def_body.get("properties", {})
            type_prop = props.get("type", {})
            if "const" in type_prop:
                expected.add(str(type_prop["const"]))
    return expected


def format_validation_error(error: ValidationError) -> str:
    """Format a ValidationError including its context for detailed failure reporting."""
    details = [error.message]
    if error.context:
        sub_messages = [
            f"  - path {list(sub.path)}: {sub.message}"
            for sub in error.context
            if sub.path or sub.message
        ]
        if sub_messages:
            details.append("Underlying reasons:\n" + "\n".join(sub_messages[:5]))
    return "\n".join(details)


def test_panel_daemon_examples_valid() -> None:
    schema = load_json(CONTRACTS_DIR / "panel-daemon.schema.json")
    examples = load_ndjson(EXAMPLES_DIR / "panel-daemon.ndjson")
    validator = Draft7Validator(schema, format_checker=format_checker)

    assert len(examples) == 23, f"Expected 23 example lines, found {len(examples)}"

    for idx, obj in enumerate(examples, 1):
        errors = list(validator.iter_errors(obj))
        assert not errors, (
            f"Validation failure in panel-daemon.ndjson line {idx}:\n"
            f"{format_validation_error(errors[0]) if errors else ''}"
        )


def test_daemon_worker_examples_valid() -> None:
    schema = load_json(CONTRACTS_DIR / "daemon-worker.schema.json")
    examples = load_ndjson(EXAMPLES_DIR / "daemon-worker.ndjson")
    validator = Draft7Validator(schema, format_checker=format_checker)

    assert len(examples) == 16, f"Expected 16 example lines, found {len(examples)}"

    for idx, obj in enumerate(examples, 1):
        errors = list(validator.iter_errors(obj))
        assert not errors, (
            f"Validation failure in daemon-worker.ndjson line {idx}:\n"
            f"{format_validation_error(errors[0]) if errors else ''}"
        )


def test_panel_daemon_all_types_covered() -> None:
    schema = load_json(CONTRACTS_DIR / "panel-daemon.schema.json")
    examples = load_ndjson(EXAMPLES_DIR / "panel-daemon.ndjson")
    expected = extract_expected_types(schema)
    actual = {str(obj.get("type")) for obj in examples}

    assert actual == expected, f"Missing types: {expected - actual}"


def test_daemon_worker_all_types_covered() -> None:
    schema = load_json(CONTRACTS_DIR / "daemon-worker.schema.json")
    examples = load_ndjson(EXAMPLES_DIR / "daemon-worker.ndjson")
    expected = extract_expected_types(schema)
    actual = {str(obj.get("type")) for obj in examples}

    assert actual == expected, f"Missing types: {expected - actual}"


@pytest.mark.parametrize(
    "invalid_payload",
    [
        {"type": "ping", "id": "invalid-uuid"},
        {"type": "send_message", "id": "11111111-1111-1111-1111-111111111111"},
        {"type": "nonexistent_type", "id": "11111111-1111-1111-1111-111111111111"},
        {
            "type": "cancel",
            "id": "11111111-1111-1111-1111-111111111111",
            "target_id": "not-a-uuid",
        },
        {
            "type": "chats",
            "id": "11111111-1111-1111-1111-111111111111",
            "chats": [
                {
                    "chat_id": "22222222-2222-2222-2222-222222222222",
                    "title": "Title",
                    "updated_at": "not-a-valid-datetime",
                }
            ],
        },
    ],
)
def test_panel_daemon_invalid_payloads_rejected(
    invalid_payload: dict[str, Any],
) -> None:
    schema = load_json(CONTRACTS_DIR / "panel-daemon.schema.json")
    validator = Draft7Validator(schema, format_checker=format_checker)

    errors = list(validator.iter_errors(invalid_payload))
    assert errors, f"Expected validation failure for invalid payload: {invalid_payload}"


@pytest.mark.parametrize(
    "invalid_payload",
    [
        {
            "type": "run",
            "request_id": "not-a-uuid",
            "chat_id": "22222222-2222-2222-2222-222222222222",
            "text": "Hello",
            "attachments": [],
        },
        {
            "type": "run",
            "request_id": "11111111-1111-1111-1111-111111111111",
            "chat_id": "22222222-2222-2222-2222-222222222222",
            # missing "text"
            "attachments": [],
        },
        {
            "type": "title",
            "request_id": "11111111-1111-1111-1111-111111111111",
            # missing "chat_id" and "first_message"
        },
        {
            "type": "message_finished",
            "request_id": "11111111-1111-1111-1111-111111111111",
            "message_id": "22222222-2222-2222-2222-222222222222",
            "status": "in_progress",  # invalid status (only "complete" or "cancelled")
        },
        {
            "type": "unknown_worker_event",
            "request_id": "11111111-1111-1111-1111-111111111111",
        },
        {
            "type": "run",
            "request_id": "11111111-1111-1111-1111-111111111111",
            "chat_id": "22222222-2222-2222-2222-222222222222",
            "text": "Hello",
            "attachments": [
                {
                    "id": "33333333-3333-3333-3333-333333333333",
                    "kind": "image",
                    "name": "large.png",
                    "mime": "image/png",
                    "size_bytes": 20000000,  # exceeds 10MB limit (10485760 bytes)
                    "path": "/path/to/large.png",
                }
            ],
        },
    ],
)
def test_daemon_worker_invalid_payloads_rejected(
    invalid_payload: dict[str, Any],
) -> None:
    schema = load_json(CONTRACTS_DIR / "daemon-worker.schema.json")
    validator = Draft7Validator(schema, format_checker=format_checker)

    errors = list(validator.iter_errors(invalid_payload))
    assert errors, f"Expected validation failure for invalid payload: {invalid_payload}"
