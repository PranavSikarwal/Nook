# Plan: PR 42, housekeeping of the old plans

Spec (small change, owner on 2026-11-14, gap G1): move the finished plans of
`docs/plans/` to `docs/archive/plans/` and keep the plans with open work. Make
`docs/plans/README.md`, `CHECKLIST.md`, `docs/plan.md` and
`docs/plans/tool-platform-open-gaps.md` match the real state. Fix the P3
findings #17 to #21 of review 3 of PR 37 (finding #16 is deferred, gap G4) and
the retro observation of PR 38 (stage 8 step 5 says "review exit check", the
heading is "Round exit check"). No behavior change in `daemon/`, `worker/`,
`panel/`, `contracts/` or `policy/`. No user-visible change, so no changelog
line and no release.

Each task is one commit. Stage only the files the task names. Tick the task's
box in the same commit. Commit locally. Do not push.

Tasks 1 to 5 are Markdown only. Their check is `git diff --check origin/main
HEAD` after the commit (no output, exit 0), plus the check in the task.
Tasks 6 to 10 touch `daemon/`. Their checks are `cd daemon && cargo fmt --check`,
`cd daemon && cargo clippy --workspace --all-targets -- -D warnings` and
`cd daemon && cargo test` (from `docs/testing.md`), all with zero warnings.

## Facts for the state documents (as of 2026-11-14)

1. Released: v0.1.4 (2026-10-11), v0.1.5 (2026-11-10), v0.1.6 (2026-11-13).
2. Merged: PR 31 (TP1 to TP3), PR 32 (TP7 to TP13, merged after PR 31), PR 34
   (allowed folders view), PR 36 (Node 24), PR 37 (Daemon exit, v0.1.6), PR 38
   (SDLC work standard).
3. Open pull requests of other tasks: PR 39 (Google Docs MCP sample and test),
   PR 40 (Notes tool run by the Panel), PR 41 (notice for a call refused by the
   folder rule, item 11 of `open-items-fixes.md`). Do not edit
   `open-items-fixes.md` or `app-integrations.md`.
4. TP11: the two designs in `docs/design/` still say "Status: proposed". The
   owner has not approved them. TP11 stays unticked.

## Tasks

- [x] **1. Archive the finished plans.** `git mv` these six files from
  `docs/plans/` to `docs/archive/plans/`: `allowed-folders-hardening.md`,
  `pr32-sonar-gate-fixes.md`, `pr32-review-response.md`,
  `sonar-complexity-cleanup.md`, `node-24-upgrade.md`, `macos-runner-update.md`.
  Change nothing inside them. Keep every other file in `docs/plans/`
  (gap G3). Check: `ls docs/plans docs/archive/plans`, and
  `/usr/bin/grep -rn -E '(allowed-folders-hardening|pr32-sonar-gate-fixes|pr32-review-response|sonar-complexity-cleanup|node-24-upgrade|macos-runner-update)\.md' --include='*.md' . | /usr/bin/grep -v -e '^./docs/archive/' -e '^./docs/tasks/'`
  prints only lines of `docs/plans/README.md` (task 2 fixes them).
  Commit: `docs(plans): archive the six finished plans`.
- [x] **2. Fix `docs/plans/README.md`.** List under "Active plans" exactly the
  files of `docs/plans/` after task 1: `allowed-folders-view.md`,
  `daemon-exit-after-panel-quit.md`, `tool-platform-closeout-and-next-phase.md`,
  `tool-platform-open-gaps.md`, `tp7-tp13-tool-providers.md`,
  `app-integrations.md` and `open-items-fixes.md`, each with a one-line reason
  that it is still active (the open box, the open check, the TP11 approval, the
  open decision). Name the six archived plans under "Archived plans". Add one
  sentence: a plan moves to `../archive/plans/` when all its boxes are ticked
  and it holds no open decision or open check (`CONTRIBUTING.md`
  "Documentation"). Keep the paragraph about new work and task folders. Check:
  every `](x.md)` link in the file points at a file that exists
  (`for l in $(/usr/bin/grep -o -E '\]\([^)#]+' docs/plans/README.md | cut -c3-); do test -e "docs/plans/$l" || echo "missing $l"; done` prints nothing).
  Commit: `docs(plans): list the active plans in the plans README`.
- [x] **3. Fix `docs/plans/tool-platform-open-gaps.md`.** Update it to the
  facts above, with simple English and the existing numbering. Add the line
  "Updated on November 14, 2026." under the recorded date. Item 12 (stale
  Daemon) and item 14 (merge order of PR 31 and PR 32, both merged) are
  closed, so mark them like the other closed items. Item 0 (Notes and Google
  Docs): the owner chose option 2, a Panel-run Notes tool, and the work is in
  open PR 40 (Notes tool) and PR 39 (Google Docs sample). Item 1 (TP11): say
  that the designs wait for approval, and that PR 40 builds the Notes tool from
  `desktop-app-tools.md`. Item 6 (live checks): say that v0.1.5 and v0.1.6
  were released since, and add the open live check of v0.1.6 on macOS (quit
  the Panel, `pgrep nookd` shows nothing) that the owner will run (see
  `daemon-exit-after-panel-quit.md`, "Verification" 4, and gap G3). Add one
  item for the open Mac check of `allowed-folders-view.md`. Add one item for
  the notice of a refused call (item 11 of `open-items-fixes.md`, PR 41 open).
  Keep every existing number. Items 12 and 14 stay in place, marked closed. Each new open item gets the next free number after item 14 (gap G6). Do not drop an item that is still open. Check: re-read each item against the
  facts above and against `CHANGELOG.md`. Commit:
  `docs(plans): bring the tool platform gaps up to date`.
- [x] **4. Fix `CHECKLIST.md` and `docs/plan.md`.** `CHECKLIST.md`: keep TP11
  unticked and say exactly why: the designs are written and wait for approval,
  and PR 40 builds the Notes tool from the desktop design (no claim that the
  owner approved). Make the line under the title name the files that still
  hold the open work: `docs/plans/tool-platform-closeout-and-next-phase.md`,
  `docs/plans/tool-platform-open-gaps.md` and `docs/plans/open-items-fixes.md`.
  `docs/plan.md`: TP11 says "Add browser and desktop-app capability designs"
  (as in `CHECKLIST.md`), and TP13 no longer lists image search (the owner
  removed it on October 17, 2026, see `tool-platform-open-gaps.md` item 2).
  Change nothing else in those two files. Check: `/usr/bin/grep -n -E 'TP1[13]'
  CHECKLIST.md docs/plan.md` shows the new text, and
  `/usr/bin/grep -c -E '^- \[ \]' CHECKLIST.md` prints 1. Commit:
  `docs: align the checklist and the plan with the real state`.
- [x] **5. Fix the stage 8 wording.** In
  `.agents/rules/sdlc-work-standard.md`, stage 8 step 5, replace "the review
  exit check still holds" with "the round exit check still holds", the heading
  of `.agents/rules/review-standard.md` "Round exit check". Change nothing
  else. Check: `/usr/bin/grep -rn -i 'review exit check' .agents AGENTS.md CLAUDE.md
  CONTRIBUTING.md docs/testing.md` prints nothing. Commit:
  `docs(rules): name the round exit check in stage 8`.

- [x] **6. Finding #17: a test for the tool-abort and provider-drop steps.**
  In `daemon/crates/nook-core/tests/`, add one test that registers a running
  tool (or a fake MCP provider) in `AppState`, calls
  `nook_core::shutdown::stop_everything`, and asserts that the tool process
  group is gone and the provider is dropped. Test through the public interface
  with the real collaborators the suite already uses. Show that the test fails
  when the abort step is removed (comment it out, run the test, restore it) and
  write that result in the final report. Change no non-test code. Check: the
  new test passes, and the three daemon checks above pass. Commit:
  `test(daemon): check that stop_everything aborts running tools`.
- [ ] **7. Finding #18: pass the fake Worker flags through `env_vars`.** In
  `daemon/crates/nook-core/tests/supervisor_tests.rs`, stop using
  `std::env::set_var` for `FAKE_WORKER_IGNORE_EOF` and the PID file variable.
  Pass them through the `env_vars` of the supervisor config. Add `cfg(unix)` to
  the test that uses `kill -0`. Check: `cd daemon && cargo test -p nook-core
  --test supervisor_tests` passes ten times in a row, and the three daemon checks
  pass. Commit: `test(daemon): pass the fake Worker flags through env_vars`.
- [ ] **8. Finding #19: `TestDaemon::drop` kills the Worker.** In
  `daemon/crates/nookd/tests/signal_shutdown_tests.rs`, make `drop` read
  `worker.pid` and send SIGKILL to that PID after it kills the Daemon. Ignore a
  missing file and a dead PID. Check: `cd daemon && cargo test -p nookd --test
  signal_shutdown_tests` passes five times in a row and leaves no fake Worker
  (`pgrep -f fake_worker` prints nothing, use the process name that the test
  uses), and the three daemon checks pass. Commit:
  `test(daemon): kill the Worker when a test Daemon drops`.
- [ ] **9. Finding #21: a test for the symlink at the socket path.** In
  `daemon/crates/nook-core/tests/` (next to the other socket ownership tests,
  `socket_ownership_tests.rs`), add a test: bind a socket, replace the path with
  a symlink that points at it, and assert that `remove_socket_if_owned` returns
  false and the link stays. Show that it fails when `symlink_metadata` is
  replaced by `metadata` in `socket_identity` (restore it after) and write that
  result in the report. Change no non-test code. Check: the new test passes and
  the three daemon checks pass. Commit:
  `test(daemon): refuse to remove a symlink at the socket path`.
- [ ] **10. Finding #20: doc comments and docs.** Doc comments of one or two
  lines (AGENTS.md "Comments") for `stop_everything` (what it stops, in which
  order, and that every step waits), `abort_all_running_tools` (it waits for the
  tasks to finish), `socket_identity` (it returns `(dev, ino)`) and
  `remove_socket_if_owned` in `daemon/crates/nook-core/src/shutdown.rs` and
  `server.rs`. `docs/testing.md`: extend the paragraph about
  `signal_shutdown_tests.rs` (the Daemon leaves no Worker process, the tests of
  tasks 6 to 9). `docs/architecture.md`: say that on SIGTERM or SIGINT the
  Daemon waits up to 2 seconds for the Worker, the running tool tasks and the
  providers to stop before it exits, and exits after 5 seconds in any case. Read
  `daemon/crates/nookd/src/main.rs` and `nook-core/src/shutdown.rs` first, and
  state only what the code does. Check: the three daemon checks and
  `cd daemon && cargo doc --workspace --no-deps` pass with zero warnings, then
  `git diff --check origin/main HEAD`. Commit:
  `docs(daemon): document the shutdown steps and the socket identity`.
