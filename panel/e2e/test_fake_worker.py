import json
import queue
import subprocess
import sys
import threading
from pathlib import Path
from uuid import uuid4

import pytest
from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[2]
FAKE_WORKER = Path(__file__).with_name("fake_worker.py")
SCHEMA = json.loads((ROOT / "contracts" / "daemon-worker.schema.json").read_text())
WORKER_EVENT_TYPES = {
    "ready",
    "message_started",
    "text_delta",
    "tool_call_started",
    "tool_call_finished",
    "authorize_tool",
    "message_finished",
    "title_ready",
    "deleted",
    "error",
}
TIMEOUT = 5


def validator_for(definition: str) -> Draft7Validator:
    return Draft7Validator(
        {"$ref": f"#/definitions/{definition}", "definitions": SCHEMA["definitions"]}
    )


def assert_valid_request(message: dict) -> None:
    Draft7Validator(SCHEMA).validate(message)


class FakeWorker:
    def __init__(self) -> None:
        self.process = subprocess.Popen(
            [sys.executable, str(FAKE_WORKER)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        self.lines: queue.Queue[str | None] = queue.Queue()
        self.seen: list[dict] = []
        threading.Thread(target=self.pump, daemon=True).start()

    def pump(self) -> None:
        assert self.process.stdout
        for line in self.process.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def send(self, message: dict) -> None:
        assert_valid_request(message)
        assert self.process.stdin
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def next_event(self) -> dict:
        line = self.lines.get(timeout=TIMEOUT)
        assert line is not None, "worker closed stdout"
        event = json.loads(line)
        assert event["type"] in WORKER_EVENT_TYPES, event
        Draft7Validator(SCHEMA).validate(event)
        self.seen.append(event)
        return event

    def expect(self, event_type: str) -> dict:
        event = self.next_event()
        assert event["type"] == event_type, event
        return event

    def expect_silence(self) -> None:
        with pytest.raises(queue.Empty):
            self.lines.get(timeout=0.5)

    def run(self, text: str, chat_id: str | None = None) -> dict:
        request = {
            "type": "run",
            "request_id": str(uuid4()),
            "chat_id": chat_id or str(uuid4()),
            "text": text,
            "attachments": [],
        }
        self.send(request)
        return request

    def collect_until_finished(self) -> list[dict]:
        events = []
        while True:
            events.append(self.next_event())
            if events[-1]["type"] == "message_finished":
                return events

    def authorize(self, request: dict, decision: str, **extra: str) -> None:
        self.send(
            {
                "type": "authorization_result",
                "request_id": request["request_id"],
                "call_id": self.last_call_id,
                "decision": decision,
                **extra,
            }
        )

    @property
    def last_call_id(self) -> str:
        return next(
            e["call_id"] for e in reversed(self.seen) if e["type"] == "authorize_tool"
        )

    def close(self) -> None:
        if self.process.poll() is None:
            self.send({"type": "shutdown"})
            self.process.wait(timeout=TIMEOUT)
        assert self.process.stdin
        self.process.stdin.close()


@pytest.fixture
def worker():
    fake = FakeWorker()
    assert fake.expect("ready")["version"] == "e2e"
    yield fake
    fake.close()


def joined_text(events: list[dict]) -> str:
    return "".join(e["text"] for e in events if e["type"] == "text_delta")


def test_fetch_prompt_requests_authorization_with_run_ids(worker):
    url = "https://docs.example.com/x"
    run = worker.run(f"E2E_APPROVAL:{url}")
    started = worker.expect("message_started")
    request = worker.expect("authorize_tool")

    assert request["request_id"] == run["request_id"]
    assert request["message_id"] == started["message_id"]
    assert request["chat_id"] == run["chat_id"]
    assert request["tool_name"] == "nook:web_fetch"
    assert json.loads(request["arguments"]) == {"url": url}
    assert request["call_id"]
    worker.expect_silence()


def test_search_prompt_requests_web_search(worker):
    worker.run("E2E_SEARCH:nook overlay")
    worker.expect("message_started")
    request = worker.expect("authorize_tool")

    assert request["tool_name"] == "nook:web_search"
    assert json.loads(request["arguments"]) == {"query": "nook overlay"}


def test_allow_runs_tool_and_completes(worker):
    url = "https://docs.example.com/x"
    run = worker.run(f"E2E_APPROVAL:{url}")
    worker.expect("message_started")
    request = worker.expect("authorize_tool")
    worker.authorize(run, "allow")

    events = worker.collect_until_finished()
    types = [e["type"] for e in events]
    assert types[:2] == ["tool_call_started", "tool_call_finished"]
    assert types[-1] == "message_finished"
    assert events[-1]["status"] == "complete"
    assert events[0]["call_id"] == events[1]["call_id"] == request["call_id"]
    assert events[0]["name"] == "nook:web_fetch"
    assert events[0]["arguments"] == request["arguments"]
    assert f"Approved deterministic fetch for {url}." in joined_text(events)
    assert all(e["request_id"] == run["request_id"] for e in events)


def test_deny_reports_reason_and_skips_tool(worker):
    run = worker.run("E2E_APPROVAL:https://deny.example.test/page")
    worker.expect("message_started")
    worker.expect("authorize_tool")
    worker.authorize(run, "deny", reason="no matching grant")

    events = worker.collect_until_finished()
    assert {e["type"] for e in events} == {"text_delta", "message_finished"}
    text = joined_text(events)
    assert "Approval denied" in text
    assert "no matching grant" in text
    assert events[-1]["status"] == "complete"


def test_deny_without_reason_still_reports_denial(worker):
    run = worker.run("E2E_APPROVAL:https://deny.example.test/page")
    worker.expect("message_started")
    worker.expect("authorize_tool")
    worker.authorize(run, "deny")

    assert "Approval denied" in joined_text(worker.collect_until_finished())


def test_result_for_another_call_or_request_is_ignored(worker):
    run = worker.run("E2E_APPROVAL:https://docs.example.com/x")
    worker.expect("message_started")
    request = worker.expect("authorize_tool")

    worker.send(
        {
            "type": "authorization_result",
            "request_id": run["request_id"],
            "call_id": "call_other",
            "decision": "allow",
        }
    )
    worker.send(
        {
            "type": "authorization_result",
            "request_id": str(uuid4()),
            "call_id": request["call_id"],
            "decision": "allow",
        }
    )
    worker.expect_silence()

    worker.authorize(run, "allow")
    assert worker.collect_until_finished()[-1]["status"] == "complete"


def test_cancel_during_wait_finishes_cancelled(worker):
    run = worker.run("E2E_APPROVAL:https://docs.example.com/x")
    started = worker.expect("message_started")
    worker.expect("authorize_tool")
    worker.send({"type": "cancel", "request_id": run["request_id"]})

    finished = worker.expect("message_finished")
    assert finished["status"] == "cancelled"
    assert finished["message_id"] == started["message_id"]
    worker.expect_silence()


def test_late_result_after_cancel_is_ignored(worker):
    run = worker.run("E2E_APPROVAL:https://docs.example.com/x")
    worker.expect("message_started")
    worker.expect("authorize_tool")
    worker.send({"type": "cancel", "request_id": run["request_id"]})
    worker.expect("message_finished")
    worker.authorize(run, "allow")
    worker.expect_silence()


def test_concurrent_runs_get_unique_call_ids_and_resolve_independently(worker):
    first = worker.run("E2E_APPROVAL:https://a.example.test/1")
    second = worker.run("E2E_APPROVAL:https://b.example.test/2")
    requests = {}
    while len(requests) < 2:
        event = worker.next_event()
        if event["type"] == "authorize_tool":
            requests[event["request_id"]] = event

    assert requests[first["request_id"]]["call_id"] != requests[second["request_id"]]["call_id"]

    worker.send(
        {
            "type": "authorization_result",
            "request_id": second["request_id"],
            "call_id": requests[second["request_id"]]["call_id"],
            "decision": "deny",
        }
    )
    events = worker.collect_until_finished()
    assert all(e["request_id"] == second["request_id"] for e in events)
    worker.expect_silence()

    worker.send(
        {
            "type": "authorization_result",
            "request_id": first["request_id"],
            "call_id": requests[first["request_id"]]["call_id"],
            "decision": "allow",
        }
    )
    events = worker.collect_until_finished()
    assert all(e["request_id"] == first["request_id"] for e in events)


def test_same_url_twice_uses_distinct_call_ids(worker):
    calls = []
    for _ in range(2):
        run = worker.run("E2E_APPROVAL:https://docs.example.com/x")
        worker.expect("message_started")
        calls.append(worker.expect("authorize_tool")["call_id"])
        worker.authorize(run, "allow")
        worker.collect_until_finished()
    assert calls[0] != calls[1]


def test_never_emits_approval_requested(worker):
    run = worker.run("E2E_APPROVAL:https://docs.example.com/x")
    worker.expect("message_started")
    worker.expect("authorize_tool")
    worker.authorize(run, "allow")
    worker.collect_until_finished()
    assert "approval_requested" not in {e["type"] for e in worker.seen}


def test_plain_reply(worker):
    worker.run("E2E_REPLY:hello")
    worker.expect("message_started")
    assert worker.expect("text_delta")["text"] == "hello"
    assert worker.expect("message_finished")["status"] == "complete"


def test_scripted_error(worker):
    run = worker.run("E2E_ERROR:boom")
    worker.expect("message_started")
    error = worker.expect("error")
    assert error["request_id"] == run["request_id"]
    assert error["error"]["message"] == "boom"


def test_title_and_delete_chat(worker):
    request_id, chat_id = str(uuid4()), str(uuid4())
    worker.send(
        {"type": "title", "request_id": request_id, "chat_id": chat_id, "first_message": "hi"}
    )
    assert worker.expect("title_ready")["request_id"] == request_id
    worker.send({"type": "delete_chat", "request_id": request_id, "chat_id": chat_id})
    assert worker.expect("deleted")["chat_id"] == chat_id


def test_shutdown_exits_while_waiting_for_authorization(worker):
    worker.run("E2E_APPROVAL:https://docs.example.com/x")
    worker.expect("message_started")
    worker.expect("authorize_tool")
    worker.send({"type": "shutdown"})
    assert worker.process.wait(timeout=TIMEOUT) == 0
