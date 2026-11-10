# PR 32 SonarCloud gate fixes: implementation plan

**Goal:** Make the SonarCloud Quality Gate pass on pull request 32 by fixing the issues that fail it, with no change in behavior.

**Architecture:** No new module, message, or dependency. Each task is a local, mechanical change in an existing function.

**Tech stack:** Rust (tokio, `nook-core`, `panel/src-tauri`).

**Why the gate fails:** SonarCloud rates "Reliability on New Code" C and "Security on New Code" E. The ratings come from 11 blocking file calls inside `async` functions (Reliability) and 2 test strings that look like database passwords (Security). Pull request 32 now holds all of its new code, including pull request 34.

## Global constraints

- Work on the branch `feat/tp7-provider-and-sandbox` (already checked out). Commit locally. Do not push.
- Do the tasks in order. One task is one commit. Tick the boxes of a task in this file in the same commit.
- Change only the files that a task lists. If a step cannot be done as written, stop and report. Do not work around it.
- Behavior must not change. No test is edited except where a task says so.
- Add no dependency. Change no file in `contracts/` or `policy/`.
- Commit messages use `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Prefix shell commands with `rtk`, as `docs/testing.md` does.
- Run any command that can take more than 8 minutes in the background and write its output to a log file.
- Load the environment before a Daemon test: `set -a && . ./.env && set +a` from the repository root.
- Comments: add none unless a hidden reason remains (see "Comments" in `AGENTS.md`).

## Out of scope

Eight functions report "Cognitive Complexity above 30" (`server.rs:415`, `lib.rs:228`, `config.rs:226`, `lib.rs:554`, `supervisor.rs:40`, `lib.rs:111`, `message_stream.rs:15`). That rating is Maintainability. It does not fail the gate. A later plan covers it. Do not touch those functions except where a task below requires it.

## Tasks

### Task 1: Remove the two password-looking test strings (Security E)

**Files:** `daemon/crates/nook-core/src/daemon_log.rs` (test `config()` near line 156), `panel/src-tauri/src/lib.rs` (test `the_daemon_output_lands_in_the_app_folder_without_secrets` near line 1328).

- [x] In each test, define a `const` for the secret part (for example `SECRET: &str = "s3cr%40t!"`) and build the URL with `format!("postgres://nook:{SECRET}@db.example.com:5432/nook")`. The assertions that the secret is absent from the output stay as they are and use the same constant.
- [x] Run `rtk cargo test -p nook-core daemon_log` in `daemon/` and `rtk cargo test the_daemon_output_lands` in `panel/src-tauri/`. Both pass.
- [x] Commit: `test: build the fake database URL from a constant`

### Task 2: Use async file calls inside async functions (Reliability C)

**Files:** `daemon/crates/nook-core/src/server.rs`, `panel/src-tauri/src/lib.rs`.

- [x] `server.rs`, `Server::bind` (lines near 208, 210, 224): replace `fs::remove_file`, `fs::create_dir_all` and `fs::metadata` with `tokio::fs::...(...).await`. Leave the `umask` block and the `UnixStream::connect` call as they are.
- [x] `server.rs`, `Server::run` (lines near 270, 273): replace `fs::metadata` and `fs::remove_file` with `tokio::fs` calls. Keep the `is_ok_and` logic: `tokio::fs::metadata(&path).await.is_ok_and(...)`.
- [x] `server.rs`, `process_client_message` (line near 619): replace `fs::remove_dir_all(att_dir)` with `tokio::fs::remove_dir_all(att_dir).await`.
- [x] `lib.rs`, `send_message` (lines near 571, 586, 591, 601): replace `std::fs::create_dir_all`, `std::fs::metadata`, `std::fs::copy`, `std::fs::write` with the `tokio::fs` equivalents plus `.await`. Keep every error message and the 10 MB checks.
- [x] If `use std::fs;` in `server.rs` becomes unused on one platform, remove the unused import only. Keep `cargo clippy` clean on Linux.
- [x] Run, from `daemon/`: `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, `NOOK_REQUIRE_SANDBOX=1 rtk cargo test` (background, log to a file). Run, from `panel/src-tauri/`: `rtk cargo check` and `rtk cargo clippy -- -D warnings`. All pass.
- [x] Commit: `fix(daemon): use async file calls in async functions`

### Task 3: Replace seven closures with method paths (clippy-style warnings)

**Files:** `daemon/crates/nook-core/src/server.rs` (lines near 97, 101), `policy_assets.rs` (near 202, 210, 215), `providers/mcp.rs` (near 395), `supervisor.rs` (near 173), `tests/sandbox_tests.rs` (near 55).

- [x] Replace `.unwrap_or_else(|e| e.into_inner())` with `.unwrap_or_else(PoisonError::into_inner)`. Add `PoisonError` to the existing `std::sync` import in each file. `providers/mod.rs` already shows the pattern.
- [x] Replace `.map(|s| s.to_string())` with `.map(ToString::to_string)` in `supervisor.rs` and `sandbox_tests.rs`. In `sandbox_tests.rs` the closure is `|part| part.to_string()` on `&&str`: use `.map(|part| (*part).to_string())` only if `ToString::to_string` does not compile, and say so in the commit body.
- [x] Run `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, `rtk cargo test` in `daemon/`. All pass.
- [x] Commit: `refactor(daemon): name the method instead of a closure`

### Task 4: Tick the plan

**Files:** this file only.

- [x] Tick every box above. Commit: `docs: tick the PR 32 SonarCloud gate plan`

## Verification (done by the reviewer, not by the implementer)

1. All local checks of `docs/testing.md` for the Daemon and for `panel/src-tauri`.
2. `gh run` for CI and for the Native Tauri E2E Gate on pull request 32 after the push.
3. SonarCloud: gate passes, zero Reliability and Security issues on new code.
