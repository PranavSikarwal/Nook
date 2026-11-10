# Live E2E diagnostic plan

The live native E2E job reaches the configured model endpoint but returns an
empty assistant message. The next run must capture the Worker exception and the
Daemon process output without exposing credentials.

## Scope

The workflow will capture stderr from the Daemon and Worker in the temporary
E2E run directory. The existing artifact upload will include those files when a
live run fails.

The workflow will not log `NOOK_BASE_URL`, `NOOK_MODEL`, `NOOK_API_KEY`, request
headers, or model response bodies.

## Steps

1. Update the Daemon supervisor to write Worker stderr to a file when
   `NOOK_E2E_RUN_DIR` is set.
2. Update the Tauri app to write Daemon stderr to a file in the same directory.
3. Pass `NOOK_E2E_RUN_DIR` from the app process to the Daemon.
4. Add focused tests for the diagnostic path selection where practical.
5. Run the affected Rust tests and the deterministic native E2E test locally.
6. Commit and push the diagnostic change.
7. Dispatch the native E2E workflow for PR #24 and inspect the uploaded logs.

## Worker installation

The first diagnostic run showed that the workflow created `worker/.venv` and
installed `requirements.txt`, but did not install the local `nook-worker`
package. The Daemon starts `.venv/bin/python -m nook_worker`, so each workflow
job must install the local package after the locked dependencies and verify the
module import before starting the native suite.

## Completion criteria

A failed live job uploads `daemon-stderr.log` and `worker-stderr.log` under its
existing artifact. The logs identify the Worker failure while omitting the
configured endpoint URL, model name, and API key. Both workflow jobs install
and import `nook_worker` before running their tests.
