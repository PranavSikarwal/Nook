from typing import Any

from langgraph.checkpoint.postgres.aio import AsyncPostgresSaver
from psycopg.rows import dict_row
from psycopg_pool import AsyncConnectionPool


def create_pool(conninfo: str) -> AsyncConnectionPool[Any]:
    return AsyncConnectionPool(
        conninfo=conninfo,
        kwargs={
            "autocommit": True,
            "row_factory": dict_row,
            "options": "-c search_path=worker,public",
        },
    )


async def setup_checkpointer(pool: AsyncConnectionPool[Any]) -> AsyncPostgresSaver:
    async with pool.connection() as conn:
        await conn.execute("CREATE SCHEMA IF NOT EXISTS worker;")
    saver = AsyncPostgresSaver(pool)  # type: ignore[arg-type]
    await saver.setup()
    return saver


async def delete_thread_memory(saver: AsyncPostgresSaver, thread_id: str) -> None:
    await saver.adelete_thread(thread_id)
