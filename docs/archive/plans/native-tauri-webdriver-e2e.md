# Native Tauri WebDriver end-to-end testing plan

## Goal

Run Nook's macOS desktop app through WebdriverIO and Tauri's embedded WebDriver.
The tests will use the Panel's ordinary Tauri commands, Unix socket, Rust Daemon,
Python Worker, model endpoint, and web tools. Each run will use its own app-data
directory and Postgres database.

This plan does not add a browser-to-daemon bridge. A browser cannot access the
Tauri runtime or the Daemon's Unix socket. Vite preview therefore uses a canned
frontend fallback, which is useful for UI checks but cannot verify the shipped
IPC path.

## Current evidence

The current implementation supports the native approach.

- `panel/src-tauri/src/lib.rs` starts `nookd` and connects through
  `Config::default_socket_path()`.
- `daemon/crates/nookd/src/main.rs` loads `Config`, starts the Worker, runs
  migrations, and binds that same default socket path.
- `Config::app_dir()` owns the config, socket, and attachments locations in
  `daemon/crates/nook-core/src/config.rs`.
- `nookd` reads `NOOK_BASE_URL`, `NOOK_MODEL`, `NOOK_DATABASE_URL`, and
  `NOOK_API_KEY` from its inherited environment.
- The Panel does not yet expose a test-only app-data location.

`@wdio/tauri-service` version `1.5.0` supports an embedded WebDriver provider on
macOS. Its published setup guide requires `tauri-plugin-wdio`,
`tauri-plugin-wdio-webdriver`, `wdio:default`, `withGlobalTauri: true`, and the
`@wdio/tauri-plugin` frontend import. Verify the installed packages before using
these APIs because the package controls the exact registration and feature
requirements.

## Scope

The native suite will cover the Panel, Tauri IPC, Daemon, Worker, database, and
configured external model and web tools.

It will test:

- app startup and Daemon readiness;
- Chat creation, restoration, selection, and deletion;
- message send, streaming, completion, cancellation, and rapid-send behavior;
- approval display, allow once, deny, and Chat and host grant scope;
- web search and URL fetch behavior;
- settings isolation;
- WebView console errors and Daemon and Worker logs; and
- the preview findings recorded in `.scratch/phase-2-tools-and-shortcuts/`.

The first implementation and execution target is macOS. It does not establish
native coverage for Linux or Windows. Windows currently has no Unix socket Daemon
IPC implementation.

## Test environment contract

The runner must validate this contract before it builds or launches Nook.

| Input | Requirement | Purpose |
| --- | --- | --- |
| `NOOK_APP_DIR` | Required and unique for the run | Config, socket, and attachments root |
| `NOOK_E2E_DATABASE_URL` | Required | Administrative Postgres connection used to create the run database |
| `NOOK_DATABASE_URL` | Set by the runner to a database it created | Daemon data store |
| `NOOK_BASE_URL` | Required only for live Worker scenarios | OpenAI-compatible model endpoint |
| `NOOK_MODEL` | Required only for live Worker scenarios | Model selection |
| `NOOK_API_KEY` | Required only for live Worker scenarios | Endpoint credential |
| `NOOKD_PATH` | Set by the runner to the build artifact | Prevents binary discovery ambiguity |

The runner creates a database named `nook_e2e_<run-id>`. It records the run ID in
an ownership table before the Panel starts. It refuses to use a database name
outside the `nook_e2e_` prefix. It drops only a database whose ownership marker
matches the current run ID.

The runner creates `NOOK_APP_DIR` beneath a temporary directory. It refuses the
default macOS Nook support directory and refuses a path inside it. It writes a
minimal test config into that directory. It does not read Keychain data. Live
runs fail before startup if `NOOK_API_KEY` is absent.

Test chats use a title prefix such as `[e2e <run-id>]`. The suite can delete them,
but database teardown remains the primary cleanup path. The runner kills only the
Panel, Daemon, Worker, and driver processes that it launched.

## Implementation slices

Each slice has one main risk and a direct completion check.

### 1. Add an isolated app-data root

Add an optional `NOOK_APP_DIR` override to `Config::app_dir()` in
`daemon/crates/nook-core/src/config.rs`.

Requirements:

- Treat a blank variable as absent.
- Use the variable value as the full root path when it is set.
- Keep every platform's existing default path when it is absent.
- Test the override and the normal default-path behavior without relying on the
  developer's real home directory.

Completion check: the Rust unit tests prove config, socket, and attachments paths
share the override root, and the Panel and Daemon derive the same socket path.

### 2. Add WebDriver-only Tauri support

Add the published WebdriverIO Tauri plugins to the Panel with an explicit
production-build policy. Prefer a Cargo feature that enables the embedded driver
only in E2E builds. If the plugin cannot compile behind a feature, document the
constraint and make its production exposure explicit before proceeding.

Requirements:

- Register `tauri-plugin-wdio` when the E2E build enables it.
- Register `tauri-plugin-wdio-webdriver` for embedded-driver builds.
- Add `wdio:default` only to the E2E capability configuration.
- Set `withGlobalTauri` only in the E2E Tauri configuration if Tauri supports a
  configuration overlay. Otherwise document why it must be enabled generally.
- Import `@wdio/tauri-plugin` only in E2E frontend builds.

Completion check: an E2E build opens an embedded WebDriver session, finds the
main window, and confirms the plugin API is available. The normal production
build still completes without the WebDriver server.

### 3. Create the isolated runner

Create a dedicated Panel E2E package, configuration, and scripts. The runner
builds the Daemon and Panel, creates the database and app-data directory, then
launches the app with the explicit environment contract.

Requirements:

- Validate all input paths and database names before work starts.
- Run one native app per worker. Start with one worker because a shared endpoint,
  global shortcut, and app-level state make parallel behavior ambiguous.
- Collect a run directory with WebdriverIO output, screenshots, driver output,
  Panel logs, Daemon logs, Worker logs, and a JSON environment manifest with
  secrets removed.
- Preserve artifacts when a test fails.
- Remove only the resources marked as owned after a successful run.
- Add separate `smoke`, `local`, and `live` tags. `live` tests require endpoint
  variables and never fall back to mocked or canned replies.

Completion check: the smoke suite starts the app, calls a harmless command,
creates a Chat, records artifact paths, and tears down its database and app-data
directory.

### 4. Add native scenario tests

Write accessible, stable selectors before expanding scenario coverage. Use page
objects only for repeated panel operations, such as opening history or sending a
message. Wait for explicit terminal UI states or protocol events. Do not use
fixed sleep delays as correctness checks.

Use the scenario matrix below. Run tests in groups that match their external
requirements.

Completion check: all non-live scenarios run against the real Panel and Daemon.
Live scenarios use the real Worker and endpoint when test credentials are set.

### 5. Verify and fix reported failures

Run the native cases for the preview findings. Preserve artifacts and record a
pass, fail, or blocked outcome in the related local issue.

- Issue 07: pending approval after switching to a new Chat.
- Issue 08: rapid plain sends and rapid URL sends while approval is pending.
- Issue 09: nested History buttons and independent select and delete actions.

Fix a finding only after the native test demonstrates the failure. Add a
regression test before or with the fix. A preview-only finding that does not
reproduce remains documented as preview-only evidence.

## Native run findings

Native live runs found a startup race and several test setup problems. The test
setup problems are recorded here so later runs can distinguish them from Nook
behavior.

| Finding | Evidence | State |
| --- | --- | --- |
| macOS rejected the first temporary socket path | Daemon reported `path must be shorter than SUN_LEN`. | Runner now uses `/tmp/ne<run-id>` for `NOOK_APP_DIR`. |
| The Panel did not pass its E2E environment to `nookd` | The Daemon used default config values and no test socket appeared. | Panel now forwards the isolated config, database, endpoint, and Keychain variables to its Daemon child. |
| The Panel accepted sends before Daemon readiness | A native test delayed `nookd` startup by eight seconds. The input and send action stayed disabled until `ping_daemon` succeeded. | Resolved. See issue 10. |
| WebdriverIO reported a window title or label mismatch | The current native smoke run targets Tauri label `main` and passed without a title or label warning. | Resolved. The earlier warning did not reproduce. |
| Worker virtual-environment path warnings appeared | A native run with an intentionally incorrect inherited `VIRTUAL_ENV` passed, and the runner found no mismatch warning in captured logs. The supervisor now removes `VIRTUAL_ENV` for `uv run --project`. | Resolved. |
| Repeated approvals appeared after a startup error | The transcript had a connection error and repeated web-fetch approval cards. The native live test now drafts a second URL while approval is pending and confirms only one user message is present. | The UI does not send another request while streaming. No duplicate approval was observed in the passing native test. |
| The live assertion matched an approval URL as an answer URL | The test saw `https://` in an approval card before a completed Worker response. | Resolved. The test checks the assistant message status and its own source links. |

The native live news scenario passed. It verified a completed sourced answer,
approval handling, a second URL draft during streaming, a follow-up response,
History selection and deletion, and the absence of nested buttons. The native
smoke test verified that the Panel blocks sends until Daemon ping succeeds. A
live run with an incorrect inherited `VIRTUAL_ENV` also passed without a path
mismatch warning.

## Scenario matrix

| ID | Tag | Scenario | Expected result |
| --- | --- | --- | --- |
| N01 | smoke | Launch Panel and wait for Daemon | Main window opens and Daemon ping succeeds. |
| N02 | local | Start a new Chat | Empty transcript has no stale reply or approval state. |
| N03 | local | Open and close History | Drawer state changes without console errors. |
| N04 | local | Select a saved Chat | Transcript and title match the selected Chat. |
| N05 | local | Delete a test-created Chat | The row disappears and other test-created Chats remain. |
| N06 | local | Delete the selected Chat | The Panel selects a valid remaining state. |
| N07 | live | Send a plain question | One user message and one terminal assistant reply appear. |
| N08 | live | Cancel a streaming reply | The reply reaches the documented cancelled terminal state. |
| N09 | live | Send a second plain prompt while streaming | Each accepted request has one terminal state. A rejected send stays visibly unsent. |
| N10 | live | Trigger a fetch approval | One approval card identifies the originating Chat and call. |
| N11 | live | Start a new Chat with approval pending | The old approval never appears in the new Chat. |
| N12 | live | Deny an approval | The UI follows the decided rejection behavior and clears the card. |
| N13 | live | Allow once | The requested action resumes once and the card clears. |
| N14 | live | Allow for Chat | Later matching action in the same Chat uses the recorded scope only. |
| N15 | live | Change Chat after Chat grant | The other Chat does not inherit the grant. |
| N16 | live | Allow for host | A matching host request follows the documented host scope. |
| N17 | live | Use a different host after host grant | The other host still requests approval. |
| N18 | live | Send a URL while approval is pending | The UI displays no duplicate card for one pending call. |
| N19 | live | Run an India news query | The reply contains current fetched content and source links. The assertion does not require a fixed headline or ranking. |
| N20 | local | Open settings in an isolated run | Settings write only to the run's config file. |
| N21 | local | Inspect History action controls | Chat selection and deletion are separate accessible controls with no nested button warning. |
| N22 | local | Reload or restart the Panel | Saved test Chats restore from the run database only. |

## Execution order

Run tests in this order to find environment failures before application failures.

1. Run `N01` after each build or runner change.
2. Run `N02` through `N06`, `N20`, `N21`, and `N22` against the isolated
   Panel and Daemon.
3. Supply test endpoint variables through the process environment or a local
   ignored file. Do not put a secret in a tracked test fixture or chat message.
4. Run `N07` through `N19` one at a time at first. Repeat each confirmed
   regression case five times after it passes once.
5. Preserve artifacts for every failure. Add the native outcome to issues 07,
   08, and 09 before changing application logic.

## Completion criteria

The work is complete when all of these statements are true.

- The suite opens the real macOS Nook Tauri app through the embedded driver.
- The test run uses a separate app-data root, socket, attachments directory, and
  `nook_e2e_` database.
- The runner rejects a missing or unsafe configuration instead of falling back
  to the browser preview or normal Nook data.
- The smoke suite runs without model credentials.
- The live suite exercises the real Worker and endpoint without canned replies.
- The native results for issues 07, 08, and 09 are recorded with artifacts.
- Confirmed failures have regression tests and fixes. Non-reproducing failures
  remain marked as preview-only until new evidence appears.
- The suite records WebView, Panel, Daemon, and Worker failures in test artifacts.
- No browser-to-daemon bridge is added.

## Prerequisites

- macOS with the Tauri build requirements.
- Node.js, npm, and the WebdriverIO packages.
- Postgres that can create and drop `nook_e2e_` databases through the configured
  administrative URL.
- The embedded WebDriver Cargo and frontend plugins with the permissions required
  by the installed package versions.
- For live scenarios, a test model endpoint URL, model name, and API key exposed
  as environment variables.

If a prerequisite is unavailable, the runner stops and reports it. It does not
start the browser mock or use the normal Nook configuration.