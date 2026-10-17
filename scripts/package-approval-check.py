#!/usr/bin/env python3
"""Runs the packaged Daemon and Worker through one allowed and one denied tool call.

The script plays the Panel and the model endpoint. It starts the packaged
Daemon with the packaged Worker and policy, points the Worker at a local fake
model, and sends two chat messages through the Daemon socket. The fake model
asks for `web_fetch` once per chat. The first chat allows the call and the
second denies it.

Usage:
    package-approval-check.py --resources DIR --database-url URL

DIR holds `daemon/nookd`, `worker/release_entry`, and `policy/`. URL names an
empty PostgreSQL database that the caller created for the run. The Daemon
creates its tables there. The caller drops the database afterwards. Unix
systems only.
"""

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

FETCH_URL = "https://package-check.invalid/page"
FINAL_TEXT = "PACKAGE-CHECK-DONE"
DENIAL_TEXT = "Request denied. I stopped here."
STEP_TIMEOUT = 120


class FakeModel(ThreadingHTTPServer):
    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), FakeModelHandler)
        self.lock = threading.Lock()
        self.requests: list[dict] = []

    def tool_results_seen(self) -> int:
        with self.lock:
            return sum(
                1
                for body in self.requests
                for message in body.get("messages", [])
                if message.get("role") == "tool"
            )


class FakeModelHandler(BaseHTTPRequestHandler):
    server: FakeModel

    def log_message(self, *_args) -> None:
        # The default handler prints one line per request to standard error.
        return None

    def do_POST(self) -> None:
        length = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(length) or b"{}")
        with self.server.lock:
            self.server.requests.append(body)
        wants_tool = bool(body.get("tools")) and not any(
            message.get("role") == "tool" for message in body.get("messages", [])
        )
        if wants_tool:
            delta = {
                "role": "assistant",
                "tool_calls": [
                    {
                        "index": 0,
                        "id": f"call_{uuid.uuid4().hex[:8]}",
                        "type": "function",
                        "function": {
                            "name": "web_fetch",
                            "arguments": json.dumps({"url": FETCH_URL}),
                        },
                    }
                ],
            }
            finish = "tool_calls"
            message = {
                "role": "assistant",
                "content": None,
                "tool_calls": delta["tool_calls"],
            }
        else:
            delta = {"role": "assistant", "content": FINAL_TEXT}
            finish = "stop"
            message = {"role": "assistant", "content": FINAL_TEXT}
        if body.get("stream"):
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.end_headers()
            for choice in (
                {"index": 0, "delta": delta, "finish_reason": None},
                {"index": 0, "delta": {}, "finish_reason": finish},
            ):
                chunk = {
                    "id": "chatcmpl-check",
                    "object": "chat.completion.chunk",
                    "created": 0,
                    "model": "package-check",
                    "choices": [choice],
                }
                self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
            self.wfile.write(b"data: [DONE]\n\n")
            return
        payload = json.dumps(
            {
                "id": "chatcmpl-check",
                "object": "chat.completion",
                "created": 0,
                "model": "package-check",
                "choices": [{"index": 0, "message": message, "finish_reason": finish}],
                "usage": {
                    "prompt_tokens": 1,
                    "completion_tokens": 1,
                    "total_tokens": 2,
                },
            }
        ).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


class PanelConnection:
    def __init__(self, path: Path) -> None:
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(STEP_TIMEOUT)
        self.sock.connect(str(path))
        self.buffer = b""

    def send(self, message: dict) -> None:
        self.sock.sendall(json.dumps(message).encode() + b"\n")

    def read(self) -> dict:
        while b"\n" not in self.buffer:
            data = self.sock.recv(65536)
            if not data:
                raise RuntimeError("The Daemon closed the socket")
            self.buffer += data
        line, _, self.buffer = self.buffer.partition(b"\n")
        return json.loads(line)

    def close(self) -> None:
        self.sock.close()


def run_chat(socket_path: Path, text: str, action: str) -> dict:
    """Sends one message, answers the approval with `action`, and returns a summary."""
    request_id = str(uuid.uuid4())
    chat_id = str(uuid.uuid4())
    panel = PanelConnection(socket_path)
    summary = {"approval": None, "text": "", "status": None, "tool_started": False}
    try:
        panel.send(
            {
                "type": "send_message",
                "id": request_id,
                "chat_id": chat_id,
                "text": text,
                "attachments": [],
            }
        )
        while True:
            message = panel.read()
            kind = message.get("type")
            if kind == "error":
                raise RuntimeError(f"Daemon error: {message['error']}")
            if message.get("id") not in (None, request_id):
                continue
            if kind == "approval_requested":
                summary["approval"] = message
                panel.send(
                    {
                        "type": "approval_decision",
                        "id": request_id,
                        "chat_id": chat_id,
                        "call_id": message["call_id"],
                        "action": action,
                    }
                )
            elif kind == "text_delta":
                summary["text"] += message["text"]
            elif kind == "tool_call_started":
                summary["tool_started"] = True
            elif kind == "message_finished":
                summary["status"] = message["status"]
                return summary
    finally:
        panel.close()


def wait_for_socket(path: Path, daemon: subprocess.Popen) -> None:
    deadline = time.time() + 60
    while time.time() < deadline:
        if daemon.poll() is not None:
            raise RuntimeError(f"The Daemon exited with {daemon.returncode}")
        if path.exists():
            try:
                PanelConnection(path).close()
                return
            except OSError:
                pass
        time.sleep(0.2)
    raise RuntimeError("The Daemon did not open its socket in 60 seconds")


def check(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)
    print(f"ok: {message}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--resources", required=True, type=Path)
    parser.add_argument("--database-url", required=True)
    args = parser.parse_args()
    args.resources = args.resources.resolve()

    daemon_exe = args.resources / "daemon" / "nookd"
    worker_exe = args.resources / "worker" / "release_entry"
    policy_dir = args.resources / "policy"
    for path in (daemon_exe, worker_exe, policy_dir):
        if not path.exists():
            print(f"Missing {path}", file=sys.stderr)
            return 2

    model = FakeModel()
    threading.Thread(target=model.serve_forever, daemon=True).start()
    work = Path(tempfile.mkdtemp(prefix="nook-pkg-"))
    app_dir = work / "app"
    app_dir.mkdir()
    (app_dir / "config.toml").write_text(
        f'base_url = "http://127.0.0.1:{model.server_address[1]}/v1"\n'
        'model = "package-check"\n'
        f'database_url = "{args.database_url}"\n'
    )
    env = {
        **os.environ,
        "NOOK_APP_DIR": str(app_dir),
        "NOOK_POLICY_DIR": str(policy_dir),
        "NOOK_WORKER_COMMAND": str(worker_exe),
        "NOOK_API_KEY": "package-check-key",
        "NOOK_DISABLE_KEYCHAIN": "1",
    }
    for name in ("NOOK_BASE_URL", "NOOK_MODEL", "NOOK_DATABASE_URL"):
        env.pop(name, None)
    log = open(work / "daemon.log", "w")
    daemon = subprocess.Popen(
        [str(daemon_exe)], env=env, cwd=work, stdout=log, stderr=log
    )
    failed = False
    try:
        socket_path = app_dir / "daemon.sock"
        wait_for_socket(socket_path, daemon)

        allowed = run_chat(socket_path, "Fetch the page and tell me.", "allow_once")
        check(allowed["approval"] is not None, "the allowed chat asked for approval")
        check(
            allowed["approval"]["tool_name"] == "nook:web_fetch",
            "the request names web_fetch",
        )
        check(allowed["tool_started"], "the tool started after Allow")
        check(allowed["status"] == "complete", "the allowed reply finished")
        check(FINAL_TEXT in allowed["text"], "the reply resumed with the model answer")
        seen_after_allow = model.tool_results_seen()
        check(seen_after_allow >= 1, "the model received the tool result")

        denied = run_chat(socket_path, "Fetch the page and tell me.", "deny")
        check(denied["approval"] is not None, "the denied chat asked for approval")
        check(not denied["tool_started"], "the tool never started after Deny")
        check(denied["status"] == "complete", "the denied reply finished")
        check(DENIAL_TEXT in denied["text"], "the reply says the request was denied")
        check(
            model.tool_results_seen() == seen_after_allow,
            "the model received no tool result after Deny",
        )
    except Exception as error:
        failed = True
        print(f"FAILED: {error}", file=sys.stderr)
        log.flush()
        tail = (work / "daemon.log").read_text().splitlines()[-40:]
        print("--- Daemon log tail ---", file=sys.stderr)
        print("\n".join(tail), file=sys.stderr)
    finally:
        daemon.terminate()
        try:
            daemon.wait(timeout=15)
        except subprocess.TimeoutExpired:
            daemon.kill()
        model.shutdown()
        log.close()
        shutil.rmtree(work, ignore_errors=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
