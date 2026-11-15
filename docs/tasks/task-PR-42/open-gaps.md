# Open gaps

## G1. Treat the task as a small change

- Stage: 1 start
- Question: does the task skip stages 2 to 4 (spec, spec review, gate 1)?
- Options: (a) small change. (b) all nine stages.
- Recommendation: (a). The task changes only Markdown, doc comments and tests.
- Decision: (a).
- Decided by: owner ("Yes, small change (Recommended)")
- Date: 2026-11-14
- Status: decided by owner

## G2. Which P3 findings of review 3 of PR 37 this task fixes

- Stage: 1 start
- Question: #16 changes behavior (exit code 1 when the cleanup times out), #17,
  #18, #19 and #21 change tests in `daemon/`, and #20 is docs and doc comments.
  The task said "docs only".
- Options: (a) all except #16. (b) only #20. (c) all six, and drop the small
  change.
- Recommendation: (a).
- Decision: (a). #17 to #21 are fixed here. #16 is deferred (G4).
- Decided by: owner ("All except #16 (Recommended)")
- Date: 2026-11-14
- Status: decided by owner

## G3. Two plans with a manual Mac check

- Stage: 1 start
- Question: `allowed-folders-view.md` has one open box (the menu shows both
  items) and `daemon-exit-after-panel-quit.md` ends with a live check of the
  installed v0.1.6 on macOS. Archive them?
- Options: (a) keep both active until the owner runs the checks. (b) archive the
  Daemon exit plan and carry its check into `tool-platform-open-gaps.md`. (c)
  the owner already ran both.
- Recommendation: (b).
- Decision: (a). Both plans stay in `docs/plans/`.
- Decided by: owner ("I will do the check keep them active")
- Date: 2026-11-14
- Status: decided by owner

## G4. Exit code 1 when the Daemon cleanup times out (finding #16)

- Stage: 1 start
- Question: when does the Daemon exit with a non-zero code after
  `stop_everything` returns false?
- Options: (a) a follow-up code task: exit 1, or kill the Worker PID before the
  exit. (b) never.
- Recommendation: (a). It changes Daemon behavior, so it needs a spec and gate 1.
  The same task can take the other deferred items of review 3 and review 2 of PR
  37 (#5 signal handlers installed late, #7 log lines lost at exit).
- Decision: (a), deferred.
- Decided by: owner ("All except #16 (Recommended)")
- Date: 2026-11-14
- Status: deferred

## G5. Deferred gaps of PR 38

- Stage: 1 start
- Question: which gaps of `docs/tasks/task-PR-38/open-gaps.md` have the status
  `deferred`?
- Options: none.
- Recommendation: none.
- Decision: only G11 (the `reviewers.yml` hardening) has the status `deferred`.
  The owner put G11 outside this task. The retro proposals P1, P2 and P4 were
  applied in PR 38, P7 through G13, and P3, P5 and P6 were rejected, so no
  proposal is left. The only retro follow-up that this task adds is the
  observation that stage 8 step 5 says "review exit check" while the heading is
  "Round exit check".
- Decided by: owner (the task text: "G11 ... is NOT part of this task")
- Date: 2026-11-14
- Status: decided by owner

## G6. Numbering of the new items in the open gaps file

- Stage: 6 implement (plan task 3)
- Question: the plan says item 12 (stale Daemon) is closed and also says to add
  the live v0.1.6 Mac check. Does the check take the number 12?
- Options: (a) item 12 stays closed, and each new open item gets the next free
  number at the end of its section. (b) renumber the file.
- Recommendation: (a). It keeps the numbers that other documents quote.
- Decision: (a). The choice touches only the file that task 3 names and changes
  no contract, policy, interface or dependency.
- Decided by: orchestrator by recommendation
- Date: 2026-11-14
- Status: closed by recommendation

## G7. How task 6 observes the abort and the provider drop

- Stage: 6 implement (plan task 6)
- Question: the plan says to assert that the tool process group is gone. The
  test cannot see a process group without a non-test change.
- Options: (a) assert through drop flags: a slow call whose future sets a flag
  when the abort drops it, and a provider whose `Drop` sets a flag. (b) change
  non-test code to expose the process group.
- Recommendation: (a). It proves the same two steps (abort and provider drop)
  through public parts, and it follows the pattern of `provider_execution_tests.rs`.
- Decision: (a). Commit `1bd481f`. The choice touches only the test file that
  task 6 names and changes no contract, policy, interface or dependency.
- Decided by: orchestrator by recommendation
- Date: 2026-11-14
- Status: closed by recommendation

## G8. How task 7 passes the fake Worker flags

- Stage: 6 implement (plan task 7)
- Question: finding #18 says to pass the flags through the supervisor's
  `env_vars`. `WorkerSupervisor::with_tools` builds `env_vars` inside, and the
  tests cannot add to it without a non-test change.
- Options: (a) pass the flags in the Worker command: `env FAKE_WORKER_IGNORE_EOF=1
  FAKE_WORKER_PID_FILE=<file> python3 <script>`. `env` replaces itself with
  `python3`, so the PID stays the same and no test changes the process
  environment. (b) add an `env_vars` parameter to the supervisor.
- Recommendation: (a). It removes the `set_var` race, which is the point of the
  finding, and changes no non-test code. (b) changes a public interface.
- Decision: (a). The choice touches only the test file that task 7 names and
  changes no contract, policy, interface or dependency.
- Decided by: orchestrator by recommendation
- Date: 2026-11-14
- Status: closed by recommendation

## G9. Retro proposal P1: read the code before the plan promises a fix

- Stage: 8 gate 2
- Question: apply retro proposal P1 of PR 42 (`retro.md`)? The rule file and
  the new text are in the proposal. File: `.agents/rules/sdlc-work-standard.md`, stage 5.
- Options: (a) apply it in a later small-change task. (b) reject it.
- Recommendation: (a).
- Decision: (a). Apply it after the G11 task of PR 38, in the order of this file.
- Decided by: owner ("works merge" at gate 2, then "Accept all six (Recommended)")
- Date: 2026-11-14
- Status: deferred

## G10. Retro proposal P2: check each fact about GitHub with a command

- Stage: 8 gate 2
- Question: apply retro proposal P2 of PR 42 (`retro.md`)? The rule file and
  the new text are in the proposal. File: `.agents/rules/sdlc-work-standard.md`, stage 5.
- Options: (a) apply it in a later small-change task. (b) reject it.
- Recommendation: (a).
- Decision: (a). Apply it after the G11 task of PR 38, in the order of this file.
- Decided by: owner ("works merge" at gate 2, then "Accept all six (Recommended)")
- Date: 2026-11-14
- Status: deferred

## G11. Retro proposal P3: a child ends a run only after a commit or a blocker

- Stage: 8 gate 2
- Question: apply retro proposal P3 of PR 42 (`retro.md`)? The rule file and
  the new text are in the proposal. File: `.agents/rules/delegation-standard.md`, "Implementer rules".
- Options: (a) apply it in a later small-change task. (b) reject it.
- Recommendation: (a).
- Decision: (a). Apply it after the G11 task of PR 38, in the order of this file.
- Decided by: owner ("works merge" at gate 2, then "Accept all six (Recommended)")
- Date: 2026-11-14
- Status: deferred

## G12. Retro proposal P4: say how to call `scripts/gh` in a worktree

- Stage: 8 gate 2
- Question: apply retro proposal P4 of PR 42 (`retro.md`)? The rule file and
  the new text are in the proposal. File: `AGENTS.md`, "GitHub CLI and commit rules".
- Options: (a) apply it in a later small-change task. (b) reject it.
- Recommendation: (a).
- Decision: (a). Apply it after the G11 task of PR 38, in the order of this file.
- Decided by: owner ("works merge" at gate 2, then "Accept all six (Recommended)")
- Date: 2026-11-14
- Status: deferred

## G13. Retro proposal P5: compare only what a child can change in the boundary check

- Stage: 8 gate 2
- Question: apply retro proposal P5 of PR 42 (`retro.md`)? The rule file and
  the new text are in the proposal. File: `.agents/rules/delegation-standard.md`, "Boundary check".
- Options: (a) apply it in a later small-change task. (b) reject it.
- Recommendation: (a).
- Decision: (a). Apply it after the G11 task of PR 38, in the order of this file.
- Decided by: owner ("works merge" at gate 2, then "Accept all six (Recommended)")
- Date: 2026-11-14
- Status: deferred

## G14. Retro proposal P6: a central doc names files and tests, never plan tasks

- Stage: 8 gate 2
- Question: apply retro proposal P6 of PR 42 (`retro.md`)? The rule file and
  the new text are in the proposal. File: `.agents/rules/delegation-standard.md`, "Implementer rules".
- Options: (a) apply it in a later small-change task. (b) reject it.
- Recommendation: (a).
- Decision: (a). Apply it after the G11 task of PR 38, in the order of this file.
- Decided by: owner ("works merge" at gate 2, then "Accept all six (Recommended)")
- Date: 2026-11-14
- Status: deferred
