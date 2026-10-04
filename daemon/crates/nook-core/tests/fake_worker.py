import json
import sys

# Print ready
sys.stdout.write(json.dumps({"type": "ready", "version": "0.1.0"}) + "\n")
sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except json.JSONDecodeError:
        continue

    req_type = req.get("type")
    req_id = req.get("request_id")

    if req_type == "shutdown":
        sys.exit(0)
    elif req_type == "run":
        msg_id = "44444444-4444-4444-4444-444444444444"
        sys.stdout.write(
            json.dumps(
                {
                    "type": "message_started",
                    "request_id": req_id,
                    "message_id": msg_id,
                }
            )
            + "\n"
        )
        sys.stdout.flush()

        sys.stdout.write(
            json.dumps(
                {
                    "type": "text_delta",
                    "request_id": req_id,
                    "message_id": msg_id,
                    "text": "Fake worker reply",
                }
            )
            + "\n"
        )
        sys.stdout.flush()

        sys.stdout.write(
            json.dumps(
                {
                    "type": "message_finished",
                    "request_id": req_id,
                    "message_id": msg_id,
                    "status": "complete",
                }
            )
            + "\n"
        )
        sys.stdout.flush()
    elif req_type == "title":
        sys.stdout.write(
            json.dumps(
                {
                    "type": "title_ready",
                    "request_id": req_id,
                    "title": "Fake Title",
                }
            )
            + "\n"
        )
        sys.stdout.flush()
    elif req_type == "delete_chat":
        sys.stdout.write(
            json.dumps(
                {
                    "type": "deleted",
                    "request_id": req_id,
                    "chat_id": req.get("chat_id"),
                }
            )
            + "\n"
        )
        sys.stdout.flush()
    elif req_type == "crash":
        sys.exit(1)
