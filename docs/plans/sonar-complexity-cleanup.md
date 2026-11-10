# Cognitive Complexity cleanup: implementation plan

**Goal:** SonarCloud reports zero open issues on pull request 32. Seven functions each report "Cognitive Complexity above 30". Split each one into small named functions, with no change in behavior.

**Architecture:** No new module, message, dependency or pattern. Each function becomes a short sequence of calls to private helpers in the same file. Names state the step. This follows "Engineering rules, Design 1" in `AGENTS.md`.

**Tech stack:** Rust (tokio, `nook-core`, `panel/src-tauri`).

## The seven functions

| # | Function | File | Complexity now | Task |
| --- | --- | --- | --- | --- |
| 1 | `Config::load_env_file` | `daemon/crates/nook-core/src/config.rs` (line 226) | 36 | 1 |
| 2 | `find_worker_directory` | `daemon/crates/nook-core/src/supervisor.rs` (line 40) | 32 | 2 |
| 3 | `find_nookd_binary` | `panel/src-tauri/src/lib.rs` (line 111) | 32 | 3 |
| 4 | `handle_send_message` | `daemon/crates/nook-core/src/message_stream.rs` (line 15) | 31 | 4 |
| 5 | `send_message` | `panel/src-tauri/src/lib.rs` (line 554) | 36 | 5 |
| 6 | `ensure_daemon_started` | `panel/src-tauri/src/lib.rs` (line 228) | 42 | 6 |
| 7 | `process_client_message` | `daemon/crates/nook-core/src/server.rs` (line 420) | 46 | 7 |

Sonar allows 30. Aim for 20 or less in the original function and in every new helper.

## Global constraints

- Work on the branch `feat/tp7-provider-and-sandbox` (already checked out). Commit locally. Do not push.
- Do the tasks in order. One task is one commit. Tick the boxes of a task in this file in the same commit.
- Change only the file a task lists (and its test module). If a step cannot be done as written, stop and report. Do not work around it.
- Behavior must not change: same order of checks, same error messages, same messages sent, same `#[cfg(...)]` gates. Move code into a helper without rewriting its logic.
- Keep each helper private (`fn`, not `pub`) unless a test in another file needs it. Name a helper for the step it does, such as `worker_dir_from_env`. Do not add a "helper", "util" or "manager" name.
- Add no dependency. Change no file in `contracts/` or `policy/`.
- Comments: add none unless a hidden reason remains (see "Comments" in `AGENTS.md`). Move an existing comment with the code it explains.
- Commit messages use `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Prefix shell commands with `rtk`.
- Compiling the Tauri crate takes 5 to 10 minutes. During a task, use `rtk cargo check` and a targeted `rtk cargo test <name>`. Run the full suites once, in Task 8.
- Run any command that can take more than 8 minutes in the background with output to a log file, then poll the log. Commands die after 10 minutes.
- Time box: 90 minutes for the whole plan.
- Load the environment before a Daemon test: `set -a && . ./.env && set +a` from the repository root.

## Local complexity check (a rough proxy)

Clippy has a lint that counts complexity with a different method than Sonar. Use it to see whether a function got simpler: `rtk cargo clippy -- -W clippy::cognitive_complexity` in the crate's directory. A function that the lint no longer names is a good sign. Do not commit any lint setting. Sonar is the final judge, and the reviewer reads it after the push.

## Tasks

### Task 1: `Config::load_env_file`

Split the nested loops into three steps.

- [ ] Write the failing tests first, in the `tests` module of `config.rs`. Test `parse_env_line` (below): a plain `KEY=value` line, a double-quoted value, a single-quoted value, a line with spaces around `=`, a comment line, an empty line, and a line with no `=`. Run them and see them fail because the function does not exist.
- [ ] Add `fn parse_env_line(line: &str) -> Option<(&str, &str)>`. It trims the line, returns `None` for an empty line, a `#` comment, or a line without `=`, and removes one pair of matching quotes around the value.
- [ ] Add `fn first_env_file() -> Option<PathBuf>` that returns the first existing path of `.env`, `../.env`, `../../.env`.
- [ ] Rewrite `load_env_file` as: find the file, read it, and for each parsed line set the variable only when it is not set yet.
- [ ] Run `rtk cargo test -p nook-core config` in `daemon/`. All pass, and the existing config tests still pass.
- [ ] Commit: `refactor(daemon): split the env file loader into small steps`

### Task 2: `find_worker_directory`

Turn the four search stages into four functions, and chain them in the same order.

- [ ] Add `worker_dir_from_env() -> Option<PathBuf>` (the `NOOK_WORKER_DIR` check), `worker_dir_near_working_directory() -> Option<PathBuf>` (the `worker`, `../worker`, `../../worker` check), `worker_dir_near_executable() -> Option<PathBuf>` (everything under `current_exe`), and `worker_dir_in_home() -> Option<PathBuf>`.
- [ ] If a "directory with `pyproject.toml`" test appears twice, extract `fn is_worker_project(dir: &Path) -> bool`. Keep the exact conditions: the working-directory stage accepts `pyproject.toml` or `src/nook_worker`; the others accept `pyproject.toml` only.
- [ ] `find_worker_directory` becomes one expression: `worker_dir_from_env().or_else(worker_dir_near_working_directory).or_else(worker_dir_near_executable).or_else(worker_dir_in_home)`.
- [ ] Run `rtk cargo clippy --all-targets -- -D warnings` and `rtk cargo test -p nook-core supervisor` in `daemon/`. All pass.
- [ ] Commit: `refactor(daemon): search for the worker folder one place at a time`

### Task 3: `find_nookd_binary`

Same method as Task 2. Keep the order: environment override, next to the executable, development paths, system locations.

- [ ] Add `fn nookd_file_name() -> &'static str` that returns `"nookd.exe"` on Windows and `"nookd"` elsewhere. Use it instead of the two `#[cfg]` `let bin_name` blocks.
- [ ] Add `nookd_from_env()`, `nookd_near_executable()`, `nookd_in_build_folders()`, and `nookd_in_system_locations()`, each returning `Option<PathBuf>`. Keep the macOS-only `/Applications` part behind `#[cfg(target_os = "macos")]` inside `nookd_in_system_locations`.
- [ ] `find_nookd_binary` becomes one `or_else` chain.
- [ ] Run `rtk cargo check` and `rtk cargo clippy -- -D warnings` in `panel/src-tauri/`. Both pass.
- [ ] Commit: `refactor(panel): search for the daemon binary one place at a time`

### Task 4: `handle_send_message`

- [ ] Add `fn check_send_request(text: &str, attachments: &[Attachment]) -> Result<(), ErrorInfo>`. It holds the three checks in the same order: empty request, more than 5 attachments, each `validate_attachment`. It returns the same `ErrorInfo` values as today.
- [ ] In the handler, replace the three blocks with one `if let Err(error) = check_send_request(&text, &attachments) { cleanup(&state).await; let _ = tx_out.send(DaemonMessage::Error { id, error }).await; return; }`.
- [ ] If the handler is still above 30 after this, extract the same pattern for the `state.pool.get()` failure: `async fn reject(state, id, tx_out, cleanup, error)` is not allowed because `cleanup` is a closure. Instead stop and report the number from the Sonar proxy.
- [ ] Run `rtk cargo test -p nook-core` in `daemon/` for the message and socket tests (`rtk cargo test -p nook-core message`). All pass.
- [ ] Commit: `refactor(daemon): check a send request in one function`

### Task 5: `send_message` (Panel)

- [ ] Add `fn attachment_kind(mime: &str) -> Result<AttachmentKind, String>` with the exact current rules and messages.
- [ ] Add `async fn store_attachment(att: AttachmentInput, att_dir: &Path) -> Result<Attachment, String>` that holds the body of the `for att in attachments` loop: copy or decode, the 10 MB checks, the kind, and the `Attachment` value.
- [ ] Add `async fn store_attachments(attachments: Vec<AttachmentInput>, att_dir: &Path) -> Result<Vec<Attachment>, String>` or keep the loop in `send_message` with a single call to `store_attachment`.
- [ ] Add `async fn forward_events(...)` for the spawned task that reads lines and clears `active_request_id`, only if `send_message` is still above 30. Keep the clearing logic as it is.
- [ ] Run `rtk cargo check`, `rtk cargo clippy -- -D warnings`, and `rtk cargo test` in `panel/src-tauri/`. All pass.
- [ ] Commit: `refactor(panel): store each attachment in its own function`

### Task 6: `ensure_daemon_started` (Panel)

- [ ] Add `async fn running_child_is_ready(state: &DaemonState, socket_path: &Path) -> Option<bool>` or an enum `ChildCheck { Ready, Starting, Gone }` for the block "Check if an existing spawned process is still running". Keep every `eprintln!` message and the `Ok(None)`, `Ok(Some)`, `Err` handling.
- [ ] Add `fn daemon_command(bin_path: PathBuf) -> tokio::process::Command` for the `PATH`, `NOOK_APP_DIR`, forwarded variables, and the `e2e` feature variables. Keep the `#[cfg(feature = "e2e")]` loop.
- [ ] Add `fn start_stderr_capture(child: &mut tokio::process::Child)` for the "stderr to log file or to the daemon log" block.
- [ ] Keep the `#[cfg(any(unix, windows))]` and `#[cfg(not(any(unix, windows)))]` structure unchanged.
- [ ] Run `rtk cargo check` and `rtk cargo clippy -- -D warnings` in `panel/src-tauri/`. Both pass.
- [ ] Commit: `refactor(panel): split the daemon start into small steps`

### Task 7: `process_client_message`

The function is a `match` on `ClientMessage`. Move the long arms into functions and keep the short arms. `send_permissions`, `send_roots` and `send_invalid_request` in the same file are the pattern to copy.

- [ ] Move each long arm into `async fn handle_<name>(state: &AppState, id: Uuid, ..., tx_out: &mpsc::Sender<DaemonMessage>)`: `get_settings`, `set_settings`, `list_chats`, `get_chat`, `delete_chat`, `cancel`. Pass only what the arm uses. Keep the order of the database write, the memory change and the message sent.
- [ ] Each arm in the `match` becomes one call, `handle_x(...).await`.
- [ ] Run `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings` in `daemon/`, and `NOOK_REQUIRE_SANDBOX=1 rtk cargo test -p nook-core --test socket_ownership_tests --test folders_tests` (background, log to a file). All pass.
- [ ] Commit: `refactor(daemon): handle each client message in its own function`

### Task 8: Verify and tick

- [ ] In `daemon/`, in the background with logs: `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, `NOOK_REQUIRE_SANDBOX=1 rtk cargo test`. All pass with zero warnings.
- [ ] In `panel/src-tauri/`, in the background with logs: `rtk cargo fmt --check`, `rtk cargo clippy -- -D warnings`, `rtk cargo test`. All pass with zero warnings.
- [ ] Tick every box above. Commit: `docs: tick the complexity cleanup plan`

## Verification (done by the reviewer, not by the implementer)

1. The reviewer reads every diff and re-runs the Daemon and `panel/src-tauri` checks.
2. The reviewer pushes and reads the SonarCloud check: zero open issues on new code.
3. CI and the Native Tauri E2E Gate pass on the pushed head.
