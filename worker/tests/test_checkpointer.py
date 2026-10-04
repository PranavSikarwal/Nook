from typing import cast
from uuid import uuid4

import psycopg
import pytest
from langchain_core.runnables import RunnableConfig
from langgraph.checkpoint.base import CheckpointMetadata, empty_checkpoint
from nook_worker.checkpointer import (
    create_pool,
    delete_thread_memory,
    setup_checkpointer,
)
from nook_worker.config import WorkerConfig


def _is_postgres_reachable(url: str) -> bool:
    try:
        with psycopg.connect(url, connect_timeout=2) as conn:
            conn.close()
        return True
    except (psycopg.Error, OSError):
        return False


_CONFIG = WorkerConfig()
_POSTGRES_REACHABLE = _is_postgres_reachable(_CONFIG.database_url)


@pytest.mark.skipif(
    not _POSTGRES_REACHABLE,
    reason="Postgres database is not reachable",
)
@pytest.mark.asyncio
async def test_checkpointer_setup_and_delete_thread():
    pool = create_pool(_CONFIG.database_url)
    async with pool:
        checkpointer = await setup_checkpointer(pool)
        thread_id = str(uuid4())
        config = cast(
            RunnableConfig,
            {"configurable": {"thread_id": thread_id, "checkpoint_ns": ""}},
        )

        cp = empty_checkpoint()
        metadata = cast(CheckpointMetadata, {"step": 1})
        await checkpointer.aput(config, cp, metadata, {})

        saved = await checkpointer.aget(config)
        assert saved is not None

        await delete_thread_memory(checkpointer, thread_id)

        after_delete = await checkpointer.aget(config)
        assert after_delete is None
