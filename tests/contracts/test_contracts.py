"""Contract tests for Nook inter-process protocols.

Validates that all example JSON lines match their respective schemas, all message
types have examples, and invalid payloads are rejected.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft7Validator, ValidationError

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CONTRACTS_DIR = REPO_ROOT / "contracts"
EXAMPLES_DIR = CONTRACTS_DIR / "examples"


def load_json(path: Path) -> dict[str, Any]:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def load_ndjson(path: Path) -> list[dict[str, Any]]:
    lines: list[dict[str, Any]] = []
    with open(path, encoding="utf-8") as f:
        for idx, line in enumerate(f, 1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                lines.append(json.loads(stripped))
            except json.JSONDecodeError as err:
                pytest.fail(f"Invalid JSON in {path}:{idx}: {err}")
    return lines


def extract_expected_types(schema: dict[str, Any]) -> set[str]:
    expected: set[str] = set()
    definitions = schema.get("definitions", {})
    for def_body in definitions.values():
        if isinstance(def_body, dict):
            props = def_body.get("properties", {})
            type_prop = props.get("type", {})
            if isinstance(type_prop, dict) and "const" in type_prop:
                expected.add(str(type_prop["const"]))
    return expected


def test_panel_daemon_examples_valid() -> None:
    schema = load_json(CONTRACTS_DIR / "panel-daemon.schema.json")
    examples = load_ndjson(EXAMPLES_DIR / "panel-daemon.ndjson")
    validator = Draft7Validator(schema)

    assert len(examples) == 21, f"Expected 21 example lines, found {len(examples)}"

    for idx, obj in enumerate(examples, 1):
        errors = list(validator.iter_errors(obj))
        assert not errors, (
            f"Validation failure in panel-daemon.ndjson line {idx}: {errors[0].message}"
        )


def test_daemon_worker_examples_valid() -> None:
    schema = load_json(CONTRACTS_DIR / "daemon-worker.schema.json")
    examples = load_ndjson(EXAMPLES_DIR / "daemon-worker.ndjson")
    validator = Draft7Validator(schema)

    assert len(examples) == 14, f"Expected 14 example lines, found {len(examples)}"

    for idx, obj in enumerate(examples, 1):
        errors = list(validator.iter_errors(obj))
        assert not errors, (
            f"Validation failure in daemon-worker.ndjson line {idx}: {errors[0].message}"
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
    ],
)
def test_panel_daemon_invalid_payloads_rejected(
    invalid_payload: dict[str, Any],
) -> None:
    schema = load_json(CONTRACTS_DIR / "panel-daemon.schema.json")
    validator = Draft7Validator(schema)

    with pytest.raises(ValidationError):
        validator.validate(invalid_payload)
