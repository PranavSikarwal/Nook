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
- [x] Commit: `refactor(daemon): keep Config in one impl block and pin the lone quote case`

### Task 2: `server.rs` (findings 5 and 6)

- [x] Add `pub(crate) fn database_unavailable() -> ErrorInfo` in `server.rs` with code `DatabaseUnavailable`, message `Database connection not available`, `retryable: true`. Use it in `handle_list_chats` and `handle_get_chat`.
- [x] Replace `socket_path.exists()` in `Server::bind` and `att_dir.exists()` in `handle_delete_chat` with `tokio::fs::try_exists(..).await.unwrap_or(false)`. Keep the `#[cfg(unix)]` gate.
- [x] Run `timeout 280 rtk cargo clippy --all-targets -- -D warnings` and `timeout 280 rtk cargo test -p nook-core --test socket_ownership_tests --test folders_tests` in `daemon/`. All pass.
- [x] Commit: `refactor(daemon): one database-unavailable error and async existence checks`

### Task 3: `message_stream.rs` (findings 1, 2, 6)

- [x] Write the tests first, in a `#[cfg(test)] mod tests` in `message_stream.rs`: `check_send_request` returns `InvalidRequest` for empty text and no attachments; returns `AttachmentInvalid` with `Attachments count exceeds maximum limit of 5` for 6 attachments; returns the `validate_attachment` message for an invalid attachment; the empty check wins over the count check. Build `Attachment` values like the existing tests in the crate do (search `tests/` and `validate_attachment` for a helper). Run them: they pass, because they pin existing behavior.
- [x] Replace the `cleanup` closure with `async fn cleanup_request(state: &AppState, id: Uuid)` holding the same five lines.
- [x] Add `async fn fail_request(state: &AppState, id: Uuid, error: ErrorInfo, tx_out: &mpsc::Sender<DaemonMessage>)` that calls `cleanup_request` and then sends `DaemonMessage::Error { id, error }`. Use it in every early exit of `handle_send_message` that does "clean up, then send an error": the `check_send_request` failure, the pool, `ensure_chat`, `save_user_message`, supervisor, and `send_request` branches. Use `database_unavailable()` from `server.rs` for the pool branch.
- [x] Keep the order of the cleanup and the send. Keep every message text.
- [x] Run `timeout 280 rtk cargo clippy --all-targets -- -D warnings` and `timeout 280 rtk cargo test -p nook-core` in `daemon/`. All pass. Run `timeout 280 rtk cargo clippy -- -W clippy::cognitive_complexity` and report the number for `handle_send_message` (it was 48).
- [x] Commit: `refactor(daemon): fail a send request in one function and test its checks`

### Task 4: `panel/src-tauri/src/lib.rs` (findings 2, 4, 5)

- [x] Write the tests first, in the existing test module or a new one: `attachment_kind` gives `Image` for `image/png`, the error `Unsupported image format: image/bmp` for `image/bmp`, `Pdf` for `application/pdf`, `Text` for `text/plain`, and `Unsupported attachment MIME type: application/zip` for `application/zip`. `store_attachment` with neither `file_path` nor `data_base64` returns `Attachment must provide either file_path or data_base64`; base64 data larger than 10 MB returns `Attachment exceeds 10MB limit: <name>`; a small base64 payload writes a file in a temp dir and returns the right `size_bytes`. Use `tempfile` (already a dev dependency) and `#[tokio::test]`. They pass, because they pin existing behavior.
- [x] Change `running_child_is_ready` to `child_is_running(state, socket_path) -> bool`: `true` when a spawned child is still running (the caller returns), `false` otherwise. The caller becomes `if child_is_running(state, &socket_path).await { return; }`. Keep every `eprintln!` and the `wait_for_daemon_ready` call.
- [x] Replace `src.exists()` in `store_attachment` and `socket_path.exists()` in `ensure_daemon_started` (inside the `#[cfg(unix)]` line) with `tokio::fs::try_exists(..).await.unwrap_or(false)`. Leave `exists()` in non-async functions.
- [x] Run, in `panel/src-tauri/`: `timeout 280 rtk cargo clippy -- -D warnings` and `timeout 280 rtk cargo test` (re-run after a timeout). All pass.
- [x] Commit: `refactor(panel): test the attachment rules and name the child check for what it does`

### Task 5: Verify and record

- [x] Run each of these in the foreground with `timeout 290`, and re-run after a timeout until it finishes: in `daemon/`: `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, `NOOK_REQUIRE_SANDBOX=1 rtk cargo test`; in `panel/src-tauri/`: `rtk cargo fmt --check`, `rtk cargo clippy -- -D warnings`, `rtk cargo test`. All pass with zero warnings.
- [x] Add a section "Review response" at the end of this file: one line per finding with the commit hash that fixes it, and the clippy number for `handle_send_message`.
- [x] Tick every box above. Commit: `docs: record the PR 32 review response`

## Review response

Review: https://github.com/PranavSikarwal/Nook/pull/32#pullrequestreview-5842455039

| # | Finding | Result | Commit |
| --- | --- | --- | --- |
| 1 | `handle_send_message` still complex | The five repeated early exits use `fail_request`, and `cleanup` is `cleanup_request`. Clippy's number fell from 48 to 39. It is still above clippy's 25 and below the SonarCloud limit of 30. I did not move the `MessageFinished` arm or the title dispatch (see Decisions). | `7657d68` |
| 2 | No tests for the moved logic | Tests for `check_send_request`, `attachment_kind` and `store_attachment`. No test for the folder search order (see Decisions). | `7657d68`, `c3b713c` |
| 3 | `parse_env_line` lone quote | Kept, and pinned by `parse_env_line_keeps_a_lone_quote`. | `b4e5518` |
| 4 | `Option<bool>` that nobody reads | `child_is_running` returns `bool`. | `c3b713c` |
| 5 | Blocking `exists()` in async functions | `tokio::fs::try_exists` in `Server::bind`, `handle_delete_chat`, `store_attachment` and `ensure_daemon_started`. | `527eef6`, `c3b713c` |
| 6 | Database error in three places | One `database_unavailable()` in `server.rs`. | `527eef6`, `7657d68` |
| 7 | Helpers split `impl Config` | One `impl Config` block, free functions after it. | `b4e5518` |

Checks run by the reviewer of this plan: `cargo fmt --check` and `cargo clippy -D warnings` in `daemon/` and `panel/src-tauri/` pass; `NOOK_REQUIRE_SANDBOX=1 cargo test` in `daemon/` passes (364 tests); `cargo test` in `panel/src-tauri/` passes (10 tests).

One test, `check_send_request_empty_check_wins_over_count_check`, repeats the first test, because the empty check and the count check cannot both be true. It stays.

## Round 2: follow-up review

Review: https://github.com/PranavSikarwal/Nook/pull/32#pullrequestreview-5843854912

Verdict COMMENT with no P0, P1 or P2 open. Three P3 items remain.

| # | Finding | Decision |
| --- | --- | --- |
| 2 (open part) | No tests for the folder search order | Later cycle. The stages must take their root as a parameter first (`worker_dir_in_home(home: &Path)` and similar), then a temp dir tests the accept rules. This is a separate change and not part of this PR. |
| 8 | `check_send_request_empty_check_wins_over_count_check` duplicates the first test | Task 6. |
| 9 | Two `return true;` for the same state in `child_is_running` | Task 7. |

### Task 6: `message_stream.rs` (finding 8)

**Files:** `daemon/crates/nook-core/src/message_stream.rs` (test module only).

- [x] Replace `check_send_request_empty_check_wins_over_count_check` with this test, which checks an order that can happen (the count check runs before the per-attachment check):

```rust
#[test]
fn check_send_request_counts_attachments_before_validating_them() {
    let atts: Vec<_> = (0..6)
        .map(|i| make_attachment(&format!("f{i}.exe"), "application/x-msdownload", AttachmentKind::Image))
        .collect();
    let err = check_send_request("hello", &atts).unwrap_err();
    assert_eq!(err.message, "Attachments count exceeds maximum limit of 5");
}
```

- [x] Split `check_send_request_rejects_empty_text_and_no_attachments` into two tests, one for `""` and one for whitespace only (one behavior per test, `AGENTS.md` Tests 3).
- [x] Run `timeout 280 rtk cargo test -p nook-core message_stream` in `daemon/` and `timeout 280 rtk cargo fmt --check`. All pass.
- [x] Commit: `test(daemon): replace a duplicate send-request test with a real ordering test`

### Task 7: `child_is_running` (finding 9)

**Files:** `panel/src-tauri/src/lib.rs`.

- [ ] In the `Ok(None)` arm of `child_is_running`, keep one `return true;`: log the "running but not responding" line only when `wait_for_daemon_ready(..., 1500 ms)` returns false, then return `true`. Keep the text of the log line and the wait time.
- [ ] Run `timeout 280 rtk cargo fmt --check`, `timeout 280 rtk cargo clippy -- -D warnings` and `timeout 280 rtk cargo test` in `panel/src-tauri/` (re-run after a timeout). All pass.
- [ ] Commit: `refactor(panel): return once from the running-child branch`
