# Retro: PR 38

## What went wrong

1. The first design described the rule instead of being the rule. The owner
   said "spec is basically design only" after round 1, and the real rule files
   came in round 2. Evidence: `reviews.md` round 1, the owner's words in round 2.
2. The spec review took 7 rounds. Most findings were rule commands that did not
   work, not wrong ideas. Examples: `\t` in `grep -E` fails with GNU grep
   (round 4), `gh pr checks` exits 0 with a missing check (round 3), the
   reviews and thread reads stopped at 30 reviews and 100 threads (round 5),
   `git diff --check` prints nothing after the commit (code round 1), and a
   `grep` pattern counted a quoted example line (code round 1).
3. The orchestrator made avoidable edit errors: a script stopped on a missing
   heading and the round 4 fixes went out as two commits (`6b91f7a`,
   `d579503`), and a blank line at the end of a file came in twice (the plan and
   `reviews.md`). The new `git diff --check origin/main HEAD` caught both before
   the push.
4. The orchestrator fixed small defects itself in stage 6 and 7 (plan tasks 6, 7
   and 13) although all code goes to implementers by the owner's working model.
   The rule does not say who fixes a one-line defect.
5. The owner changed or added requirements during the task: the reviewer split
   (G10), no round limit (G12), every CI check is a gate, the retro, and the
   model order (G14). The "new requirement is a new gap" rule handled it, but
   two spec changes after gate 1 (G13, G15) are not noted in the state block.
6. The native gate failed once on a macOS runner flake (Postgres still
   "rejecting connections", run 43477721481). The rule says to rerun with
   `scripts/gh run rerun`. The orchestrator pushed a new head and dispatched a
   new run instead, and the record says so.
7. An Antigravity child started through T3 loaded no workspace rule (plan task 4,
   gap G13). The rule assumed it would.

## Why

1. The rule did not say what the spec is when the product of the task is itself
   a rule or a document.
2. No step makes the author run each command in a rule before it goes to
   review. The reviewer ran them, and that took a round for each defect.
3. The orchestrator edits files with scripts and commits without a check. The
   new Markdown check exists, but the rule does not make it a step before each
   commit.
4. The delegation rule defines who implements a plan task, not who fixes a
   defect found during stage 6 or 7 that sits outside any task.
5. The rule for a spec change after gate 1 says to return to gate 1, and the
   state block has no line to record that the owner approved it.
6. "Rerun" in the rule means only `gh run rerun`. The native gate starts with
   `workflow_dispatch`, so a new dispatch is its rerun.
7. The rule did not check what each harness loads under T3 before relying on it.

## Proposals

### P1. The spec of a rule task is the rule

- File: `.agents/rules/sdlc-work-standard.md`, stage 2.
- New text: "When the task changes rules or documents, those files are the spec.
  Write them in this stage, and keep `specs/design.md` to the why, the harness
  table and the rule changes."
- Cause: 1.
- Owner: pending

### P2. Run every command before review

- File: `.agents/rules/sdlc-work-standard.md`, stage 2 and stage 5 exit checks.
- New text: add "Run each command that the task adds to a rule or a doc, with
  `/usr/bin/grep` and on a real pull request, and write the result into
  `reviews.md`." to the exit of stage 2 (for a rule task) and stage 6.
- Cause: 2.
- Owner: pending

### P3. Check before every commit

- File: `.agents/rules/sdlc-work-standard.md`, section "Push".
- New text: "Before each commit, run `git diff --check origin/main HEAD`. When it
  prints anything, fix it first. Apply a scripted edit to every file before
  committing, and stop when one edit does not apply."
- Cause: 3.
- Owner: pending

### P4. Who fixes a defect found during stage 6 or 7

- File: `.agents/rules/delegation-standard.md`, new paragraph in "T3 Code" 4.
- New text: "An implementer fixes a defect that sits outside a plan task, as a new
  plan task. The orchestrator may fix a one-line Markdown defect itself, records
  it as a plan task marked `(orchestrator)`, and runs the same checks."
- Cause: 4.
- Owner: pending

### P5. Record spec changes after gate 1

- File: `.agents/rules/task-records.md`, state block.
- New text: add the line `Spec changes after gate 1: <gap ids approved by owner,
  or none>`.
- Cause: 5.
- Owner: pending

### P6. A new dispatch is the rerun of the native gate

- File: `.agents/rules/review-standard.md`, "When an input fails to arrive" 3.
- New text: "For `Native Tauri E2E Gate`, start `native-e2e-gate.yml` again on the
  same head. Write it into the round as `flake`."
- Cause: 6.
- Owner: pending

### P7. Check what a harness loads before relying on it

- File: `.agents/rules/delegation-standard.md`, "T3 Code" 1.
- New text: "A brief never assumes that a child loaded a workspace rule. It names
  the files to read, for every provider." This is already in the rule after
  G13. Record P7 as done.
- Cause: 7.
- Owner: pending
