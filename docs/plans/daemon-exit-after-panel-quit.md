# Daemon exit after the Panel quits: implementation plan

**Goal:** When the Panel quits, the Daemon that the Panel started stops. When any Daemon gets SIGTERM or SIGINT, it exits within 5 seconds and removes its socket. This closes item 12 of `docs/plans/open-items-fixes.md` and goes into release v0.1.6.

**Problem:** After a normal quit, the installed Daemon (process 42719) kept the socket and ignored SIGTERM for over 15 seconds. A new Panel would have used the old Daemon. Two causes are likely, and one change fixes each.

1. The Panel never kills the Daemon on exit. `daemon_command` sets `kill_on_drop(true)`, but Tauri ends the process with `std::process::exit`. The managed state may not drop, so the kill may not run. `.run(|app, event| ...)` in `panel/src-tauri/src/lib.rs` has no `RunEvent::Exit` handler.
2. The Daemon may wait for something after a signal. The signal task in `daemon/crates/nookd/src/main.rs` writes a log line before it sends the shutdown message. If that write blocks, the Daemon never shuts down. After `Server::run` returns, `main` returns, and the Tokio runtime waits for blocking tasks such as the one in `providers/filesystem.rs`.

**Decision:** Do not wait for proof of the exact cause. Build a repro first (Task 1) so that each fix has a failing test. Fix every cause that the repro shows. Add a hard exit deadline in any case, because a Daemon that cannot stop is the worst outcome.

**Architecture:** No new message, module or dependency. The Daemon gets a small `shutdown` helper in `nookd`. The Panel gets one `RunEvent::Exit` arm that calls a function in `lib.rs`.

**Tech stack:** Rust (tokio, `nookd`, `nook-core`, `panel/src-tauri`).

## Global constraints

- Work on the branch `fix/daemon-exit-on-sigterm` (already checked out, from `origin/main`). Commit locally. Do not push.
- One task is one commit. Tick the boxes of a task in this file in the same commit.
- Change only the files that a task lists. If a step cannot be done as written, stop and report.
- Add no dependency. Change no file in `contracts/` or `policy/`.
- Commit messages use `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Prefix every shell command with `rtk`.
- Run every command in the foreground with a `timeout` of at most 540 seconds. Never start a background command and never wait for a task. If a command needs more time, split it, for example one test file at a time.
- Load the environment before a Daemon test: `set -a && . ./.env && set +a` from the repository root.
- Comments: add none unless a hidden reason remains. Start a trap with `Warning:`.
- Write the failing test first. Run it and see it fail for the expected reason. Then write the code.
- The disk was cleaned: the first build is cold and takes several minutes. Use `timeout 540` and rerun the same command if it times out. Cargo resumes where it stopped.

## Tasks

### Task 1: Reproduce the stuck Daemon (Linux)

**Files:** create `daemon/crates/nookd/tests/signal_shutdown_tests.rs`. Reuse the helpers of `check_policy_tests.rs` and `crates/nook-core/tests/common/` where they fit.

Write one test for each scenario. Each test starts the real `nookd` binary (`env!("CARGO_BIN_EXE_nookd")`) with a temporary socket path, a temporary config and the fake Worker (see `socket_ownership_tests.rs` and `supervisor_tests.rs` for how they set this up), waits until the socket accepts a connection, sends SIGTERM with `libc::kill` or `nix` if the crate already depends on it (else `Command::new("kill")`), and then asserts that the process exits within 5 seconds and that the socket file is gone.

- [x] `the_daemon_exits_on_sigterm_with_no_client`
- [x] `the_daemon_exits_on_sigterm_with_an_idle_client_connected`
- [x] `the_daemon_exits_on_sigterm_when_stderr_has_no_reader`: spawn with `Stdio::piped()` for stderr, then drop the read end before the signal.
- [x] `the_daemon_exits_on_sigterm_when_the_stderr_pipe_is_full`: spawn with `Stdio::piped()` for stderr, never read it, and write enough log lines first to fill the 64 KB pipe (set `RUST_LOG=trace` and connect and disconnect clients in a loop, or ask the Daemon for the setting that raises log volume).
- [x] `the_daemon_exits_on_sigint`
- [x] Run each test alone: `cd daemon && timeout 540 rtk cargo test -p nookd --test signal_shutdown_tests <name>`. Write the result of each (pass, fail, hang) into the section "Repro results" at the end of this file. A test that hangs must be killed by its own 5 second assertion, not by the harness.
- [x] Commit: `test(daemon): reproduce a Daemon that ignores SIGTERM`. A failing test is expected and allowed in this commit. Mark each failing test with `#[ignore = "fails until Task 2"]` only after the results are written down, and remove the `ignore` in Task 2.

If every scenario passes, write that in "Repro results", commit what you have, and stop. Report to the planner. Do not start Task 2.

### Task 2: Make the Daemon exit on a signal

**Files:** `daemon/crates/nookd/src/main.rs`, create `daemon/crates/nookd/src/shutdown.rs` if the logic is longer than 30 lines.

Do each sub-step only for a scenario that failed in Task 1, except the first one, which is always needed.

- [x] Hard deadline (always): when the first signal arrives, start a `std::thread` (not a Tokio task) that sleeps 5 seconds and then calls `std::process::exit(0)` after it removes the socket file if the path still exists. A second signal exits at once.
- [x] Send the shutdown message before the log line, and make the log line unable to block the signal task (for example log from `tokio::task::spawn_blocking` or drop the line). Do this only if the pipe scenarios failed.
- [x] After `server.run().await?` returns, call `std::process::exit(0)` through the same helper, so that blocking tasks cannot hold the runtime. Keep the exit code `0` for a signal and `1` for an error.
- [x] Remove the `ignore` marks. All five tests pass.
- [x] Run: `cd daemon && rtk cargo fmt --check`, `timeout 540 rtk cargo clippy --workspace --all-targets -- -D warnings`, and `timeout 540 rtk cargo test -p nookd`. Run `NOOK_REQUIRE_SANDBOX=1 timeout 540 rtk cargo test -p nook-core` as well, one test file at a time if it times out. All pass.
- [x] Commit: `fix(daemon): exit within 5 seconds of SIGTERM or SIGINT`
  - [x] Extra commit `fix(daemon): log through a writer thread so a full stderr cannot block`: route SafeStderr writes through a dedicated writer thread via a 1024-element sync_channel without modifying open file description flags.

### Task 3: Stop the Daemon when the Panel quits

**Files:** `panel/src-tauri/src/lib.rs`.

- [x] Write the failing test first: a unit test `quitting_the_panel_kills_the_daemon_it_started` in the existing test module. It puts a real child (`sleep 60`, or `ping -n 60 127.0.0.1` on Windows) into `DaemonState.spawned_process`, calls the new function `stop_spawned_daemon(&state)`, and asserts that the child has exited and that the slot is `None`. A second test, `quitting_the_panel_with_no_started_daemon_does_nothing`, asserts that the call returns without error when the slot is `None`.
- [x] Add `fn stop_spawned_daemon(state: &DaemonState)` that takes the lock with `try_lock` or `blocking_lock` (the exit event is not async), calls `start_kill()` on the child and sets the slot to `None`. Add a `tauri::RunEvent::Exit` arm in the `.run(|app, event| ...)` closure that gets `app.state::<DaemonState>()` and calls it. Remove the dead `let _ = (app, event);` line only if the new arm makes it unused.
- [x] A Daemon that the Panel did not start stays alive (the slot is `None` for it). Make sure that the test above covers that.
- [x] Run, from `panel/src-tauri/`: `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy -- -D warnings`, `timeout 540 rtk cargo test`. All pass.
- [x] Commit: `fix(panel): stop the Daemon that the Panel started when it quits`

### Task 4: Docs and tick

**Files:** `docs/plans/open-items-fixes.md` (row 12), `docs/testing.md` (one sentence that names `signal_shutdown_tests.rs`), `CHANGELOG.md` (under "Unreleased": one line in the "Fixed" list), this file.

- [x] Row 12 becomes: "Fixed. The Daemon exits within 5 seconds of SIGTERM, and the Panel stops the Daemon that it started when it quits." plus the commit hashes.
- [x] Tick every box. Commit: `docs: record the fix for the Daemon that stays alive`

## Verification (done by the planner, not by the implementer)

1. Every command above, run again in a clean state.
2. A manual check on Linux: start the Daemon, `kill -TERM`, see exit and no socket.
3. CI, SonarCloud and the Native Tauri E2E Gate on the pull request.
4. After the release: a live check on the installed v0.1.6 on macOS (quit the Panel, `pgrep nookd` shows nothing).

## Repro results

| Scenario | Result | Notes |
|---|---|---|
| `the_daemon_exits_on_sigterm_with_no_client` | pass | Exited immediately and removed socket file |
| `the_daemon_exits_on_sigterm_with_an_idle_client_connected` | pass | Exited immediately and removed socket file |
| `the_daemon_exits_on_sigterm_when_stderr_has_no_reader` | hang | Daemon hung on logging to broken stderr; killed after 5s by assertion |
| `the_daemon_exits_on_sigterm_when_the_stderr_pipe_is_full` | hang | Daemon hung on logging to full 64 KB stderr pipe; killed after 5s by assertion |
| `the_daemon_exits_on_sigint` | pass | Exited immediately and removed socket file |

## Review response (PR 37)

The reviewer found no P0 or P1. This section fixes the findings that matter before the merge. Findings 5, 6, 7 and 9 go to a follow-up pull request.

Same constraints as above: branch `fix/daemon-exit-on-sigterm`, one task per commit, foreground commands with `timeout 540`, prefix `rtk`, test first, no push.

### Task 5: Clean up before the Daemon exits (finding 1)

**Files:** `daemon/crates/nookd/src/main.rs`, `daemon/crates/nookd/src/shutdown.rs`.

- [ ] Keep a clone of the `Arc` state in `main` before `Server::bind`. After `server.run()` returns, call `supervisor.shutdown()` and the cleanup that stops running tool process groups (read `sandbox.rs` `GroupKill` and `providers/` to find an explicit call; if only `Drop` does it, drop the owner explicitly). Wrap the whole cleanup in `tokio::time::timeout(Duration::from_secs(2), ...)`. Then exit.
- [ ] If no explicit cleanup API exists for the tool process groups, stop and report. Do not invent a new public API.
- [ ] Add a `Warning:` comment on `shutdown::exit`: it skips destructors.
- [ ] Add the test `the_daemon_stops_its_worker_on_sigterm`: start `nookd` with the fake Worker, send SIGTERM, assert that the Worker process is gone within 5 seconds.
- [ ] Commit: `fix(daemon): stop the Worker and tool processes before exiting`

### Task 6: One ownership-checked socket removal and honest exit codes (finding 3), and the two forced-exit tests (finding 12)

**Files:** `daemon/crates/nook-core/src/server.rs`, `daemon/crates/nookd/src/shutdown.rs`, `daemon/crates/nookd/src/main.rs`, `daemon/crates/nookd/tests/signal_shutdown_tests.rs`.

- [ ] Move the "remove the socket only if the inode is still ours" logic of `Server::run` into one public function. Use it in `Server::run`, in the deadline thread and in the second-signal path. The deadline thread and the second-signal path exit with code `1`. The graceful path exits with `0`. `Server` must expose what the function needs (the inode) without a second source of truth.
- [ ] Add `a_second_signal_exits_the_daemon_at_once`: make shutdown hang (a client connection that never closes does not hang the accept loop; instead use an environment variable such as `NOOK_TEST_HANG_SHUTDOWN` only if the code already has a test hook; else send SIGTERM twice within 100 ms and assert the exit within 1 second and the exit code `0` or `1`).
- [ ] Add `a_stuck_shutdown_is_forced_after_the_deadline`: only if a hook for a stuck shutdown can be added in test code. Otherwise state in the plan why it cannot be tested through the public interface and skip it.
- [ ] Commit: `fix(daemon): remove the socket only when it is ours on every exit path`

### Task 7: The Panel asks the Daemon to stop before it kills it (finding 2)

**Files:** `panel/src-tauri/src/lib.rs`.

- [ ] Write the test first. `quitting_the_panel_stops_a_daemon_that_handles_sigterm`: a child `sh -c 'trap "exit 0" TERM; sleep 60 & wait'` exits by itself within the grace time, and the test records that it exited with status 0 (not killed by a signal 9). `quitting_the_panel_kills_a_daemon_that_ignores_sigterm`: a child `sh -c 'trap "" TERM; sleep 60 & wait'` is killed after the grace time.
- [ ] On Unix, `stop_spawned_daemon` sends SIGTERM with `libc::kill` (the Panel already depends on `libc`; check `panel/src-tauri/Cargo.toml` and add nothing new), polls `try_wait()` every 50 ms for at most 2 seconds, then calls `start_kill()`. Pass the grace time as a parameter so a test can use a short one. On Windows keep `start_kill()`.
- [ ] Commit: `fix(panel): ask the Daemon to stop before killing it`

### Task 8: Small fixes (findings 4, 8, 10)

**Files:** `daemon/crates/nookd/tests/signal_shutdown_tests.rs`, `daemon/crates/nookd/src/shutdown.rs`, `CHANGELOG.md`.

- [ ] Finding 4: the tests use the same database URL fallback as `daemon/crates/nook-core/tests/db_tests.rs`, in one helper function. No developer user name stays in the file.
- [ ] Finding 8: `handle_signal` logs with a direct `info!` call. Remove `spawn_blocking`. Make sure "Received ..." is logged before the shutdown message is sent.
- [ ] Finding 10: the changelog line starts with "On macOS and Linux,".
- [ ] Commit: `fix(daemon): use the shared database URL in the signal tests and log signals directly`

### Task 9: Tick and verify

- [ ] Tick the boxes. Run from `daemon/`: `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy --workspace --all-targets -- -D warnings`, `timeout 540 rtk cargo test -p nookd`, `NOOK_REQUIRE_SANDBOX=1 timeout 540 rtk cargo test -p nook-core`. From `panel/src-tauri/`: `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy -- -D warnings`, `timeout 540 rtk cargo test`. Commit: `docs: tick the review response tasks`
