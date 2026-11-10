# PR 32 review response: implementation plan

**Goal:** Fix the seven findings of the review at https://github.com/PranavSikarwal/Nook/pull/32#pullrequestreview-5842455039 without changing behavior, and prove it with tests.

**Architecture:** No new module, message, or dependency. Small changes inside existing files.

**Spec:** The review above. Read its summary and its inline comments first. Then read the "Engineering rules" in `AGENTS.md`.

## Decisions

| # | Finding | Decision |
| --- | --- | --- |
| 1 | `handle_send_message` still high (clippy 48) | Fix with one `fail_request` helper for the five early exits. Do not move the `MessageFinished` arm or the title dispatch: SonarCloud already passes, and "Engineering rules, Design 8" says to build only what the step needs. |
| 2 | No tests for the moved logic | Add tests for `check_send_request`, `attachment_kind` and `store_attachment`. Do not add tests for the folder search order: they need process-wide environment and working-directory changes, and those races would make the suite flaky. |
| 3 | `parse_env_line` lone-quote change | Keep the change. Add the test `parse_env_line_keeps_a_lone_quote`. |
| 4 | `Option<bool>` whose `bool` nobody reads | Return `bool` and rename the function to `child_is_running`. |
| 5 | Blocking `exists()` in async functions | Use `tokio::fs::try_exists(..).await.unwrap_or(false)` in the async functions. Leave `exists()` in non-async functions. |
| 6 | "Database connection not available" in three places | One `database_unavailable()` in `server.rs`. |
| 7 | Helpers split `impl Config` in two | Move `read_only` into the first `impl Config`. Put the two free functions after it. |

## Global constraints

- Work on the branch `feat/tp7-provider-and-sandbox` (already checked out). Commit locally. Do not push.
- Do the tasks in order. One task is one commit. Tick the boxes of a task in this file in the same commit.
- Change only the files a task lists. If a step cannot be done as written, stop and report. Do not work around it.
- Behavior must not change: same messages, same order, same `#[cfg(...)]` gates.
- Add no dependency. Change no file in `contracts/` or `policy/`.
- Comments: add none unless a hidden reason remains (see "Comments" in `AGENTS.md`).
- Commit messages use `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Prefix shell commands with `rtk`.
- Do not start any command in the background. Run each check in the foreground with a time limit, for example `timeout 280 rtk cargo test -p nook-core config`. If a command times out, run it again (the build cache continues). The harness crashes when you start a background task.
- Load the environment before a Daemon test: `set -a && . ./.env && set +a` from the repository root.
- Time box: 60 minutes. Post one status line after each commit.

## Tasks

### Task 1: `config.rs` (findings 3 and 7)

- [x] Add the test `parse_env_line_keeps_a_lone_quote` to the existing tests: `assert_eq!(parse_env_line("KEY=\""), Some(("KEY", "\"")));`. Run it. It passes, because the code already has the guard. This is a test that pins existing behavior.
- [x] Move `read_only` into the first `impl Config` block (right after `load_env_file`). Remove the second `impl Config {` line and its closing brace if they become empty. Place `first_env_file` and `parse_env_line` after the impl.
- [x] Run `timeout 280 rtk cargo test -p nook-core config` in `daemon/`. All pass.
- [ ] Commit: `refactor(daemon): keep Config in one impl block and pin the lone quote case`

### Task 2: `server.rs` (findings 5 and 6)

- [x] Add `pub(crate) fn database_unavailable() -> ErrorInfo` in `server.rs` with code `DatabaseUnavailable`, message `Database connection not available`, `retryable: true`. Use it in `handle_list_chats` and `handle_get_chat`.
- [x] Replace `socket_path.exists()` in `Server::bind` and `att_dir.exists()` in `handle_delete_chat` with `tokio::fs::try_exists(..).await.unwrap_or(false)`. Keep the `#[cfg(unix)]` gate.
- [x] Run `timeout 280 rtk cargo clippy --all-targets -- -D warnings` and `timeout 280 rtk cargo test -p nook-core --test socket_ownership_tests --test folders_tests` in `daemon/`. All pass.
- [ ] Commit: `refactor(daemon): one database-unavailable error and async existence checks`

### Task 3: `message_stream.rs` (findings 1, 2, 6)

- [ ] Write the tests first, in a `#[cfg(test)] mod tests` in `message_stream.rs`: `check_send_request` returns `InvalidRequest` for empty text and no attachments; returns `AttachmentInvalid` with `Attachments count exceeds maximum limit of 5` for 6 attachments; returns the `validate_attachment` message for an invalid attachment; the empty check wins over the count check. Build `Attachment` values like the existing tests in the crate do (search `tests/` and `validate_attachment` for a helper). Run them: they pass, because they pin existing behavior.
- [ ] Replace the `cleanup` closure with `async fn cleanup_request(state: &AppState, id: Uuid)` holding the same five lines.
- [ ] Add `async fn fail_request(state: &AppState, id: Uuid, error: ErrorInfo, tx_out: &mpsc::Sender<DaemonMessage>)` that calls `cleanup_request` and then sends `DaemonMessage::Error { id, error }`. Use it in every early exit of `handle_send_message` that does "clean up, then send an error": the `check_send_request` failure, the pool, `ensure_chat`, `save_user_message`, supervisor, and `send_request` branches. Use `database_unavailable()` from `server.rs` for the pool branch.
- [ ] Keep the order of the cleanup and the send. Keep every message text.
- [ ] Run `timeout 280 rtk cargo clippy --all-targets -- -D warnings` and `timeout 280 rtk cargo test -p nook-core` in `daemon/`. All pass. Run `timeout 280 rtk cargo clippy -- -W clippy::cognitive_complexity` and report the number for `handle_send_message` (it was 48).
- [ ] Commit: `refactor(daemon): fail a send request in one function and test its checks`

### Task 4: `panel/src-tauri/src/lib.rs` (findings 2, 4, 5)

- [ ] Write the tests first, in the existing test module or a new one: `attachment_kind` gives `Image` for `image/png`, the error `Unsupported image format: image/bmp` for `image/bmp`, `Pdf` for `application/pdf`, `Text` for `text/plain`, and `Unsupported attachment MIME type: application/zip` for `application/zip`. `store_attachment` with neither `file_path` nor `data_base64` returns `Attachment must provide either file_path or data_base64`; base64 data larger than 10 MB returns `Attachment exceeds 10MB limit: <name>`; a small base64 payload writes a file in a temp dir and returns the right `size_bytes`. Use `tempfile` (already a dev dependency) and `#[tokio::test]`. They pass, because they pin existing behavior.
- [ ] Change `running_child_is_ready` to `child_is_running(state, socket_path) -> bool`: `true` when a spawned child is still running (the caller returns), `false` otherwise. The caller becomes `if child_is_running(state, &socket_path).await { return; }`. Keep every `eprintln!` and the `wait_for_daemon_ready` call.
- [ ] Replace `src.exists()` in `store_attachment` and `socket_path.exists()` in `ensure_daemon_started` (inside the `#[cfg(unix)]` line) with `tokio::fs::try_exists(..).await.unwrap_or(false)`. Leave `exists()` in non-async functions.
- [ ] Run, in `panel/src-tauri/`: `timeout 280 rtk cargo clippy -- -D warnings` and `timeout 280 rtk cargo test` (re-run after a timeout). All pass.
- [ ] Commit: `refactor(panel): test the attachment rules and name the child check for what it does`

### Task 5: Verify and record

- [ ] Run each of these in the foreground with `timeout 290`, and re-run after a timeout until it finishes: in `daemon/`: `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, `NOOK_REQUIRE_SANDBOX=1 rtk cargo test`; in `panel/src-tauri/`: `rtk cargo fmt --check`, `rtk cargo clippy -- -D warnings`, `rtk cargo test`. All pass with zero warnings.
- [ ] Add a section "Review response" at the end of this file: one line per finding with the commit hash that fixes it, and the clippy number for `handle_send_message`.
- [ ] Tick every box above. Commit: `docs: record the PR 32 review response`
