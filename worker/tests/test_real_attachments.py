import base64
from pathlib import Path
from uuid import uuid4

import httpx
import pytest

from nook_worker.agent import RealAgentRunner, build_deep_agent, create_model
from nook_worker.config import WorkerConfig
from nook_worker.protocol import Attachment, RunRequest, TextDeltaEvent

RED_PNG_B64 = "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAIAAAD8GO2jAAAAKElEQVR4nO3NsQ0AAAzCMP5/un0CNkuZ41wybXsHAAAAAAAAAAAAxR4yw/wuPL6QkAAAAABJRU5ErkJggg=="
_CONFIG = WorkerConfig()


def _is_endpoint_authenticated(url: str, key: str) -> bool:
    if not url or not key:
        return False
    try:
        with httpx.Client(timeout=3) as client:
            resp = client.get(
                f"{url}/models",
                headers={"Authorization": f"Bearer {key}"},
            )
            return resp.status_code == 200
    except (httpx.HTTPError, OSError):
        return False


_ENDPOINT_READY = _is_endpoint_authenticated(
    _CONFIG.base_url, _CONFIG.api_key.get_secret_value()
)


@pytest.mark.skipif(
    not _ENDPOINT_READY,
    reason="Live model endpoint not available or not authenticated",
)
@pytest.mark.asyncio
async def test_real_agent_attachments(tmp_path: Path):
    config = WorkerConfig()
    model = create_model(config)
    agent = build_deep_agent(model, config)
    runner = RealAgentRunner(agent)

    # 1. Image test
    img_path = tmp_path / "red.png"
    img_path.write_bytes(base64.b64decode(RED_PNG_B64))
    img_att = Attachment(
        id=uuid4(),
        kind="image",
        name="red.png",
        mime="image/png",
        size_bytes=img_path.stat().st_size,
        path=str(img_path),
    )

    req_img = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="What is the dominant color in this image? Answer with just the color name.",
        attachments=[img_att],
    )
    events_img = [e async for e in runner.run(req_img)]
    reply_img = "".join(
        [e.text for e in events_img if isinstance(e, TextDeltaEvent)]
    ).lower()
    assert "red" in reply_img

    # 2. Text file test
    txt_path = tmp_path / "secret.txt"
    txt_path.write_text("The secret password is pineapple-482.", encoding="utf-8")
    txt_att = Attachment(
        id=uuid4(),
        kind="text",
        name="secret.txt",
        mime="text/plain",
        size_bytes=txt_path.stat().st_size,
        path=str(txt_path),
    )
    req_txt = RunRequest(
        request_id=uuid4(),
        chat_id=uuid4(),
        text="What is the password mentioned in the attached text file?",
        attachments=[txt_att],
    )
    events_txt = [e async for e in runner.run(req_txt)]
    reply_txt = "".join([e.text for e in events_txt if isinstance(e, TextDeltaEvent)])
    assert "pineapple-482" in reply_txt
