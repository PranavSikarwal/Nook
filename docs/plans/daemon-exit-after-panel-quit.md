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

- [x] Keep a clone of the `Arc` state in `main` before `Server::bind`. After `server.run()` returns, call `supervisor.shutdown()` and the cleanup that stops running tool process groups (read `sandbox.rs` `GroupKill` and `providers/` to find an explicit call; if only `Drop` does it, drop the owner explicitly). Wrap the whole cleanup in `tokio::time::timeout(Duration::from_secs(2), ...)`. Then exit.
- [x] If no explicit cleanup API exists for the tool process groups, stop and report. Do not invent a new public API.
- [x] Add a `Warning:` comment on `shutdown::exit`: it skips destructors.
- [x] Add the test `the_daemon_stops_its_worker_on_sigterm`: start `nookd` with the fake Worker, send SIGTERM, assert that the Worker process is gone within 5 seconds.
- [x] Commit: `fix(daemon): stop the Worker and tool processes before exiting`

### Task 6: One ownership-checked socket removal and honest exit codes (finding 3), and the two forced-exit tests (finding 12)

**Files:** `daemon/crates/nook-core/src/server.rs`, `daemon/crates/nookd/src/shutdown.rs`, `daemon/crates/nookd/src/main.rs`, `daemon/crates/nookd/tests/signal_shutdown_tests.rs`.

- [x] Move the "remove the socket only if the inode is still ours" logic of `Server::run` into one public function. Use it in `Server::run`, in the deadline thread and in the second-signal path. The deadline thread and the second-signal path exit with code `1`. The graceful path exits with `0`. `Server` must expose what the function needs (the inode) without a second source of truth.
- [x] Add `a_second_signal_exits_the_daemon_at_once`: make shutdown hang (a client connection that never closes does not hang the accept loop; instead use an environment variable such as `NOOK_TEST_HANG_SHUTDOWN` only if the code already has a test hook; else send SIGTERM twice within 100 ms and assert the exit within 1 second and the exit code `0` or `1`).
- [x] Add `a_stuck_shutdown_is_forced_after_the_deadline`: skipped because no hook exists in test code and it cannot be tested through the public interface without instrumenting production code.
- [x] Commit: `fix(daemon): remove the socket only when it is ours on every exit path`

### Task 7: The Panel asks the Daemon to stop before it kills it (finding 2)

**Files:** `panel/src-tauri/src/lib.rs`.

- [x] Write the test first. `quitting_the_panel_stops_a_daemon_that_handles_sigterm`: a child `sh -c 'trap "exit 0" TERM; sleep 60 & wait'` exits by itself within the grace time, and the test records that it exited with status 0 (not killed by a signal 9). `quitting_the_panel_kills_a_daemon_that_ignores_sigterm`: a child `sh -c 'trap "" TERM; sleep 60 & wait'` is killed after the grace time.
- [x] On Unix, `stop_spawned_daemon` sends SIGTERM with `libc::kill` (the Panel already depends on `libc`; check `panel/src-tauri/Cargo.toml` and add nothing new), polls `try_wait()` every 50 ms for at most 2 seconds, then calls `start_kill()`. Pass the grace time as a parameter so a test can use a short one. On Windows keep `start_kill()`.
- [x] Commit: `fix(panel): ask the Daemon to stop before killing it`

### Task 8: Small fixes (findings 4, 8, 10)

**Files:** `daemon/crates/nookd/tests/signal_shutdown_tests.rs`, `daemon/crates/nookd/src/shutdown.rs`, `CHANGELOG.md`.

- [x] Finding 4: the tests use the same database URL fallback as `daemon/crates/nook-core/tests/db_tests.rs`, in one helper function. No developer user name stays in the file.
- [x] Finding 8: `handle_signal` logs with a direct `info!` call. Remove `spawn_blocking`. Make sure "Received ..." is logged before the shutdown message is sent.
- [x] Finding 10: the changelog line starts with "On macOS and Linux,".
- [x] Commit: `fix(daemon): use the shared database URL in the signal tests and log signals directly`

### Task 9: Tick and verify

- [x] Tick the boxes. Run from `daemon/`: `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy --workspace --all-targets -- -D warnings`, `timeout 540 rtk cargo test -p nookd`, `NOOK_REQUIRE_SANDBOX=1 timeout 540 rtk cargo test -p nook-core`. From `panel/src-tauri/`: `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy -- -D warnings`, `timeout 540 rtk cargo test`. Commit: `docs: tick the review response tasks`

## Review response, round 2 (PR 37)

The follow-up review of `f931767` found one P2 (finding 1) and four P3. This round fixes findings 1, 12, 13, 14 and 15. Findings 5, 6, 7, 9 and 11 stay in a follow-up pull request.

Same constraints as above: branch `fix/daemon-exit-on-sigterm`, one task per commit, foreground commands with `timeout 540`, prefix `rtk`, test first, no push, no background command. Load the environment before a Daemon test: `set -a && . ./.env && set +a` from the repository root. The sandbox tests cannot start bubblewrap on this machine. They skip with a message unless `NOOK_REQUIRE_SANDBOX` is set, so do not set it locally.

### Task 10: Make the cleanup wait (finding 1)

**Files:** `daemon/crates/nook-core/src/supervisor.rs`, `daemon/crates/nook-core/src/providers/mod.rs`, `daemon/crates/nook-core/src/tool_approval.rs`, create `daemon/crates/nook-core/src/shutdown.rs` (and register it in `lib.rs`), `daemon/crates/nookd/src/main.rs`, `daemon/crates/nookd/tests/signal_shutdown_tests.rs`, `daemon/crates/nookd/tests/fake_worker*` or `daemon/crates/nook-core/tests/fake_worker.py` (whichever the signal tests use).

Decision: one function, `nook_core::shutdown::stop_everything(state: &AppState, timeout: Duration)`, does the whole cleanup and waits. `nookd/src/main.rs` calls it and nothing else. The private loop in `main.rs` that aborts tools goes away. This is the one place for this rule (AGENTS.md Design 4).

- [x] Test first. Add a fake Worker mode that keeps running after its stdin closes (for example an environment variable such as `FAKE_WORKER_IGNORE_EOF=1` that makes the script sleep forever when it reads EOF; read the existing fake Worker first and extend it). Add `the_daemon_kills_a_worker_that_ignores_stdin_close`: start `nookd` with that Worker, send SIGTERM, wait for the Daemon to exit, then assert that the Worker process is gone within 1 second after the Daemon exit. Run it on the current code and see it fail (the Worker stays). Remove or fix the old `the_daemon_stops_its_worker_on_sigterm` only if the new test replaces it.
- [x] `WorkerSupervisor::shutdown` returns only after the supervisor task has killed the child and reaped it. Use a `tokio::sync::oneshot` or `Notify` that the supervisor loop signals after `child.kill().await`. If no Worker runs, it returns at once. The other caller (restart on a settings change) must still work: run the supervisor tests.
- [x] Reuse `abort_running_tools` from `tool_approval.rs` (make it `pub` if needed) and extend it, or add a sibling, so that the caller can wait: after `abort()`, poll `AbortHandle::is_finished()` every 10 ms until all are finished.
- [x] Providers: before `ProviderHandle::replace(ProviderRegistry::empty())`, take `let old = handle.current();` then replace, then wait until `Arc::strong_count(&old) == 1` (polling every 10 ms), then `drop(old)`. The `GroupKill` drop then runs on the calling thread before the function returns.
- [x] `stop_everything` runs these three steps in this order inside `tokio::time::timeout(timeout, ...)` and returns whether it finished in time. `main.rs` calls it with 2 seconds and logs a `warn!` when it returns false. Keep `shutdown::exit` and the 5 second thread unchanged.
- [x] Run `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy --workspace --all-targets -- -D warnings`, `timeout 540 rtk cargo test -p nookd`, `timeout 540 rtk cargo test -p nook-core`. All pass.
- [x] Commit: `fix(daemon): wait for the Worker and tool processes to stop before exiting`

### Task 11: Tests that reach the new code (finding 12) and a stricter ownership check (finding 13)

**Files:** `daemon/crates/nook-core/src/server.rs`, `daemon/crates/nookd/tests/signal_shutdown_tests.rs`, `daemon/crates/nook-core/tests/sandbox_tests.rs`.

- [x] Test first. In `server.rs` add unit tests for `remove_socket_if_owned`: a socket with the expected identity is removed; a different file at the same path is kept; a path that was re-bound (new socket, new identity) is kept; a missing path does not panic. Run them; the re-bound case must fail only if you can build it with the current inode-only check, otherwise state in the commit body why it cannot.
- [x] Store `(dev, ino)` from `tokio::fs::symlink_metadata` at bind time and compare both in `remove_socket_if_owned` (use `std::os::unix::fs::MetadataExt::{dev, ino}`). `Server` keeps one identity value, not two fields. Update every caller (`Server::run`, `nookd/src/shutdown.rs`, `nookd/src/main.rs`).
- [x] Delete the test `a_second_signal_exits_the_daemon_at_once`. It does not reach the second-signal path. Write one sentence in the commit body.
- [x] In `sandbox_tests.rs` add a test that starts a sandboxed command that spawns a long child, calls the same cleanup path that `stop_everything` uses (drop of the owner), and asserts that the process group is gone. It skips with a message when the sandbox is missing, like the other sandbox tests.
- [x] Run fmt, clippy and `timeout 540 rtk cargo test -p nookd` and `-p nook-core`. All pass.
- [x] Commit: `test(daemon): cover the socket ownership check and the Worker kill; compare device and inode`

### Task 12: Panel test and code cleanups (findings 14 and 15)

**Files:** `panel/src-tauri/src/lib.rs`.

- [x] Finding 14: in the two `quitting_the_panel_*` signal tests, start the shell so that it is the only process: `exec sleep 60` after `trap '' TERM` for the ignore test, and for the handler test use `trap 'exit 0' TERM; sleep 60 & wait` only if the script also kills `$!` in the trap (`trap 'kill $!; exit 0' TERM`). After a test run, `pgrep -f "sleep 60"` shows nothing.
- [x] Finding 15: add a one-line comment above the `unsafe` block: the PID is safe to signal because `child.id()` is `None` once the child is reaped. Replace the two copied wait loops with one private helper `wait_for_exit(&mut Child, Duration) -> Option<ExitStatus>`.
- [x] Run, from `panel/src-tauri/`: `rtk cargo fmt --check`, `timeout 540 rtk cargo clippy -- -D warnings`, `timeout 540 rtk cargo test`. All pass.
- [x] Commit: `fix(panel): keep the quit tests from leaking processes`

### Task 13: Tick and verify

- [x] Tick the boxes of this section. Run the Daemon and Panel check commands once more. Commit: `docs: tick the round 2 review response`

### Task 14: Kill the Worker when the supervise future is cancelled (found while checking round 2)

**Files:** `daemon/crates/nook-core/src/supervisor.rs`, `daemon/crates/nookd/tests/signal_shutdown_tests.rs`.

**Cause:** `supervisor_loop` selects on `spawn_and_supervise(...)` and on its own `shutdown_rx.recv()`. `spawn_and_supervise` has a second receiver for the same message. When the outer branch wins, the select drops the inner future, and the `Child` inside it drops without a kill, because the command has no `kill_on_drop`. The Worker stays alive. The test `the_daemon_kills_a_worker_that_ignores_stdin_close` failed in 3 of 6 runs and leaked fake Workers (parent PID 919).

- [ ] Test first: make the existing test strict. Run it 10 times in a row with a shell loop of `timeout 120 rtk cargo test -p nookd --test signal_shutdown_tests kills_a_worker`. Write down how many fail. After each run, `pkill -f tests/fake_worker.py` so that leaked Workers do not pile up.
- [ ] Fix: call `cmd.kill_on_drop(true)` where the supervisor builds the Worker command (next to `cmd.stdin(Stdio::piped())`). Do not change the select structure.
- [ ] Add a supervisor test in `crates/nook-core/tests/supervisor_tests.rs` that starts a supervisor with the fake Worker in ignore-EOF mode, calls `shutdown()`, and asserts that the Worker process is gone within 1 second. Run it 10 times too.
- [ ] Run the 10-run loop of the Daemon test again: 10 out of 10 pass. Then fmt, clippy `-D warnings`, `timeout 540 rtk cargo test -p nookd`, `timeout 540 rtk cargo test -p nook-core`.
- [ ] Commit: `fix(daemon): kill the Worker when the supervisor future is dropped`. Put the failure counts before and after into the commit body.
