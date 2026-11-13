# Open gaps: PR #38

Each entry records one question and its decision. Status is one of `open`,
`decided by owner`, `closed by recommendation` or `deferred`.

## G1. Name and place of the task folder

- Stage: design.
- Question: where do the task folders live?
- Options: (a) `docs/tasks/task-PR-<number>/`. (b) `.scratch/task-PR-<number>/`,
  next to the local issue tracker. (c) `docs/plans/task-PR-<number>/`.
- Recommendation: (a). The folder is a permanent record of why the code
  changed, and `docs/` is where the reviewer and SonarCloud already look.
- Decision: none.
- Status: open.

## G2. Rule loading for Antigravity

- Stage: design.
- Question: how does Antigravity load the rules?
- Options: (a) a file in `.agent/rules/` that points to `.agents/rules/`. (b) a
  symbolic link `.agent/rules -> ../.agents/rules`. (c) rely on `AGENTS.md` only.
- Recommendation: (a). A symbolic link breaks on a Windows checkout, and the
  pointer in `AGENTS.md` alone is weaker. The implementer checks in a live
  Antigravity run that the rule loads, and records the result here.
- Decision: none.
- Status: open.

## G3. Small changes

- Stage: design.
- Question: does a small change (a typo, a version bump, a one-line CI fix) need
  the design stage and gate 1?
- Options: (a) every pull request runs every stage. (b) a pull request may skip
  stages 2 to 4 when the owner says "small change" in the request. The task
  folder then holds only `open-gaps.md` and `reviews.md`.
- Recommendation: (b). The owner keeps the choice, and the review rounds and
  gate 2 still run.
- Decision: none.
- Status: open.

## G4. Existing plans and designs

- Stage: design.
- Question: what happens to `docs/plans/` and `docs/design/`?
- Options: (a) leave them. New work uses task folders. A plan in `docs/plans/`
  moves into a task folder when its work starts. (b) move all of them now.
- Recommendation: (a). Moving them now mixes a large rename into this pull
  request. `docs/plans/README.md` gets a note about the new place.
- Decision: none.
- Status: open.

## G5. Global rules versus repository rules

- Stage: design.
- Question: what stays in `~/.claude/CLAUDE.md`?
- Options: (a) the global file keeps the rules for all projects (writing style,
  no AI attribution, comments, RTK) and its T3 waiting section. The repository
  rules repeat the T3 rules for other providers. (b) the T3 section leaves the
  global file, and the global file points to the repository rules.
- Recommendation: (a) for now, with the global T3 section cut to a pointer once
  the repository rules prove themselves in one pull request. Other projects
  still need the T3 rules.
- Decision: none.
- Status: open.

## G6. Enforce the task folder in CI

- Stage: design.
- Question: does CI check that a pull request has its task folder?
- Options: (a) add a small CI step that fails when a pull request to `main` has
  no `docs/tasks/task-PR-<number>/design.md` (or only `open-gaps.md` and
  `reviews.md` for a small change). (b) no CI check. The reviewer checks it.
- Recommendation: (a). A rule that a machine checks does not drift. The step is
  a short shell script in `scripts/` that `ci.yml` runs.
- Decision: none.
- Status: open.

## G7. The reviewer thread id

- Stage: design.
- Question: does the rule name the T3 thread id of "Review Pull Requests"?
- Options: (a) name the thread only, and the orchestrator finds it with
  `t3_thread_search`. (b) write the id
  `3f89e173-6c3e-4c10-8d4d-c35b33b0b487` into the rule.
- Recommendation: (a). The id belongs to one machine. The name works on every
  machine.
- Decision: none.
- Status: open.
