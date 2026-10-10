import sys

from nook_worker.agent import AgentRunner
from nook_worker.main import _StdinReader, run_worker
from nook_worker.main import main as worker_main
from nook_worker.release_smoke import run_smoke_test


def main() -> int:
    if sys.argv[1:] == ["--smoke-test"]:
        return run_smoke_test()
    if sys.argv[1:] == ["--smoke-worker"]:
        import asyncio

        from nook_worker.fake_agent import FakeAgentRunner

        asyncio.run(_run_smoke_worker(FakeAgentRunner()))
        return 0
    worker_main()
    return 0


async def _run_smoke_worker(runner: AgentRunner) -> None:
    import asyncio

    is_tty = sys.stdin.isatty()
    reader = asyncio.StreamReader()
    if not is_tty:
        loop = asyncio.get_running_loop()
        protocol = asyncio.StreamReaderProtocol(reader)
        await loop.connect_read_pipe(lambda: protocol, sys.stdin)

    async def write_line(line: str) -> None:
        await asyncio.to_thread(print, line, flush=True)

    await run_worker(
        stdin_reader=_StdinReader(is_tty, reader),
        write_line=write_line,
        runner=runner,
    )


if __name__ == "__main__":
    raise SystemExit(main())
