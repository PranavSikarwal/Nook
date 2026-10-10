import json
import os
import subprocess
import sys


def run_smoke_test() -> int:
    executable = os.environ.get("NOOK_SMOKE_EXECUTABLE")
    if executable:
        arguments = [executable, "--smoke-worker"]
    else:
        arguments = [
            sys.executable,
            "-m",
            "nook_worker.release_entry",
            "--smoke-worker",
        ]
    process = subprocess.Popen(
        arguments,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert process.stdout is not None
    assert process.stdin is not None
    try:
        line = process.stdout.readline()
        event = json.loads(line)
        if event.get("type") != "ready":
            raise RuntimeError("Worker did not emit the ready event.")
        process.stdin.write(json.dumps({"type": "shutdown"}) + "\n")
        process.stdin.flush()
        return_code = process.wait(timeout=10)
        if return_code != 0:
            raise RuntimeError(f"Worker exited with status {return_code}.")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
    print("Worker protocol smoke test passed.")
    return 0


def main() -> int:
    return run_smoke_test()


if __name__ == "__main__":
    raise SystemExit(main())
