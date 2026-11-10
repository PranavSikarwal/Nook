# Native E2E merge gate plan

The merge gate will run a manually dispatched, macOS native Tauri WebdriverIO
suite. It will use deterministic Worker behavior for broad scenario coverage and
one live Worker/model/news check. The workflow will report its result on the
exact PR head commit so repository branch protection can require it.

## Goals

- Run real Tauri IPC, Panel, Rust Daemon, Worker process, and isolated PostgreSQL.
- Cover approval, chat state, history, settings isolation, request lifecycle, and
  startup behavior without spending model requests on every case.
- Use one live India news query to verify the configured model and web-search
  path end to end.
- Run only by manual dispatch, not on every push or PR synchronization.
- Preserve diagnostic artifacts and clean up only run-owned resources.
- Publish a status check on the tested PR head SHA and fail if that SHA changes
  before the workflow finishes.

## Out of scope

- Browser Playwright preview tests. They do not exercise Tauri IPC.
- Linux and Windows native execution in this gate. The current driver setup and
  daemon IPC test target macOS.
- Automatic merging. A passing status is a gate, not merge authorization.

## Gate design

The workflow accepts a PR number. It reads the PR head SHA from GitHub, checks
out that exact commit, and runs on `macos-14`. The test runner creates a unique
`nook_e2e_<run-id>` database and a temporary app-data root. It refuses to use
normal Nook data and drops only its marked database after success. On failure,
it preserves logs and artifacts.

The deterministic phase starts the real Tauri Panel and Rust Daemon. It starts
a protocol-compatible fake Worker process instead of the model-backed Worker.
The fake Worker emits controlled events and waits for approval decisions. This
lets the suite test each UI and Daemon state transition without external model
calls. The fake Worker must use the same line-based protocol as the production
Worker and must not be enabled in production builds.

The live phase starts the real Python Worker and makes one model request for
current India news. It grants the web-search approval required by the flow and
checks for a completed assistant response with source links. It does not repeat
the live request for each approval or chat scenario.

A protected GitHub environment supplies the live endpoint URL, model name, and
API key. The workflow must not print those values. Repository administrators
must configure the environment reviewers and secrets before the live gate can
run.

After the tests, the workflow posts `pending`, then `success` or `failure` to the
GitHub commit status API for the exact tested SHA. It verifies that the PR still
points to that SHA. If the branch changed while tests ran, it posts failure and
requires a new run.

## Scenario coverage

The deterministic native phase will cover these user-visible behaviors.

| ID | Scenario | Expected result |
| --- | --- | --- |
| D01 | Launch Panel while Daemon startup is delayed | Input and send remain disabled until daemon ping succeeds. |
| D02 | Start a new chat | Transcript, approval, and request state do not leak from the previous chat. |
| D03 | Open and close History | Drawer opens and closes without browser-console errors. |
| D04 | Select a saved chat | The selected row and transcript identify the same chat. |
| D05 | Delete an unselected test chat | Only that test chat disappears. |
| D06 | Delete the selected test chat | The deleted chat disappears and the Panel shows a valid empty or remaining state. |
| D07 | Submit a plain prompt | User message and assistant terminal event use the same request and message identity. |
| D08 | Cancel a streaming response | The matching assistant message reaches cancelled and does not accept later stale events. |
| D09 | Submit a second prompt while streaming | Send is unavailable, draft remains intact, first request completes, then the draft can be sent. |
| D10 | Request a protected web fetch | One approval card appears on the matching assistant message. |
| D11 | Deny approval | The Worker receives deny, the card clears, and the request reaches a terminal result. |
| D12 | Allow once | The Worker resumes once and the card clears. |
| D13 | Allow for chat and host | A later matching request in the same chat does not ask again. |
| D14 | Use the grant from another chat | The other chat receives its own approval. |
| D15 | Use the grant for another host | The new host receives its own approval. |
| D16 | Draft a URL while approval is pending | The URL stays unsent and does not create another approval. |
| D17 | Inspect History controls | Selection and deletion are separate buttons, with no nested-button markup. |
| D18 | Reload the Panel | Only chats from the isolated test database restore. |
| D19 | Open and save settings | Settings change only the isolated config and never touch the normal app config or Keychain. |
| D20 | Deliver late events after cancellation or a new request | Events with another request ID do not alter the active message or streaming state. |
| D21 | Reject a decision with the wrong chat ID | The legitimate pending approval remains available. |
| D22 | Fail to forward a decision to the Worker | The daemon retains the pending approval and returns a retryable error. |

The deterministic suite must run D01 through D22 without model credentials. The
live phase covers one additional scenario.

| ID | Scenario | Expected result |
| --- | --- | --- |
| L01 | Ask for current news in India | Real Worker, configured model, search, approval, and fetch complete with sourced answer links. |

## Implementation tasks

1. Add a protocol-compatible fake Worker for deterministic native scenarios.
   It must support scripted streaming, approvals, allow and deny decisions,
   cancellation, and controlled errors. Add unit tests for its protocol behavior.
2. Expand the WebdriverIO suite into one deterministic spec per scenario group.
   Keep each case independent through a fresh chat or isolated run database.
3. Make the E2E runner select deterministic or live mode explicitly. Use fake
   Worker settings in deterministic mode. Require all model endpoint settings only
   in live mode.
4. Add a manual GitHub Actions workflow that validates the PR number, fetches
   the exact PR head SHA, builds the native app, installs macOS dependencies,
   provisions isolated PostgreSQL, and runs deterministic tests followed by L01.
5. Add artifact upload on success and failure. Artifacts must omit secrets and
   include WebdriverIO, WebView, Daemon, Worker, and runner logs.
6. Post a commit status named `Native Tauri E2E Gate` to the tested SHA. Recheck
   the PR head before marking success. Document status failures and retries.
7. Run the gate on PR #22's current source before enabling the branch rule.
8. Configure `main` branch protection to require `Native Tauri E2E Gate`, require
   the PR branch to be up to date, and prohibit direct pushes. Keep merge rights
   with maintainers.
9. Re-run CI, Pyright, Rust tests, frontend build and lint, the manual native
   gate, and SonarCloud analysis.

## Credentials and access

The workflow needs a protected GitHub environment, for example
`native-e2e-live`, with:

- `NOOK_BASE_URL`
- `NOOK_MODEL`
- `NOOK_API_KEY`

The database is created on the macOS runner for each run. The live model secret
is read only by the live test process. The workflow must suppress shell tracing
and must not include environment dumps in artifacts.

The workflow also needs `contents: read`, `pull-requests: read`, and
`statuses: write` permissions. The repository owner must configure required
reviewers for the protected environment and add the named status check to branch
protection. These GitHub settings cannot be enforced by a workflow file alone.

## Completion criteria

- D01 through D22 pass against the native Tauri app, real Daemon, and isolated
  database, with deterministic Worker replies.
- L01 passes once against the real Worker, configured model, and web tools.
- The workflow runs only on manual dispatch and identifies the exact PR commit.
- A stale PR SHA cannot receive a passing status from a run on an older SHA.
- Artifacts preserve failures and contain no credentials.
- GitHub branch protection requires the reported gate status before merge.
- The branch protection rule does not allow a failed, pending, or missing gate to
  merge.
