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
