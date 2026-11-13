---
trigger: always_on
description: SDLC Work Standard. The stages, gates, roles and records for every change to Nook.
---

# SDLC Work Standard

Every change to this repository is one **task**: one branch, one pull request,
one task folder. A task moves through nine stages. Each stage ends on an exit
check. Start a stage only when the exit check of the stage before it passes.

The task folder is the **record**. Write each decision, round and approval into
it in the same commit as the work, so that any agent can resume the task from
the folder and the pull request alone.

Three rule files hold the detail. Read each one before the stage that needs it:

1. `.agents/rules/task-records.md`: the task folder, the state block, the open
   gaps format and the retro. Read it before stage 1.
2. `.agents/rules/review-standard.md`: review rounds. Read it before stage 3
   and stage 7.
3. `.agents/rules/delegation-standard.md`: implementers and T3 Code. Read it
   before stage 6, and put its "Implementer rules" into every brief.

## Roles

| Role | Who | Does |
| --- | --- | --- |
| Owner | The human who owns the repository | Decides open gaps, approves gate 1 and gate 2, says "merge". |
| Orchestrator | The agent the owner talks to | Writes the spec, the plan, `open-gaps.md` and `reviews.md`. Delegates, verifies, pushes, starts workflows, asks the reviewer, merges after the owner says "merge", runs the release. |
| Implementer | A child agent that the orchestrator starts | Implements the plan tasks in its brief and commits locally. Follows "Implementer rules" in the delegation rule. |
| Reviewer | Chosen by `.agents/rules/review-standard.md` | Reviews the spec and the code, as "What the reviewer may do" in that file says. |

The orchestrator is the only agent that pushes, merges, tags, starts workflows,
or edits the task folder. An implementer only ticks its task's box in
`plans/plan.md`.

Every CI check is a **gate**: `Code Quality and Tests`, `Windows named-pipe
IPC`, `SonarCloud Code Analysis` and `Native Tauri E2E Gate`. The **CI gate
check** in `.agents/rules/review-standard.md` ("Read the inputs" 2) passes only
when all four report `pass` on the head. A failing check blocks the next stage
until a fix makes it pass. A failing check is always
fixed. It is never deferred or rejected.

Text posted under the owner's account holds no AI attribution: no
`Co-Authored-By` line for an AI and no "Generated with" line in a commit, a pull
request title or body, a comment or a review. The GitHub Actions reviewer posts
as `github-actions[bot]`, so its status notices are allowed.

## Stages

### 1. Start

1. Create a branch from `main` with a conventional prefix (`feat/`, `fix/`,
   `docs/`, `chore/`), named after the work.
2. Commit an empty commit: `git commit --allow-empty -m "chore(tasks): open task"`.
3. Push, and open a draft pull request: `scripts/gh pr create --draft`.
4. Copy `docs/tasks/_template/` to `docs/tasks/task-PR-<number>/`, commit, push.

Exit: the draft pull request exists, and its branch holds the task folder.

### 2. Spec

Write `specs/design.md`: why, scope, the design, the risks, and a "Rule
changes" table when the task changes a rule or a binding document. Name each
central doc the task changes (`docs/architecture.md`, `docs/contracts.md`,
`docs/tools-and-authorization.md`, `docs/mcp-servers.md`, `docs/release.md`,
`docs/testing.md`). For a release, name the new version. Write each question
that the owner must decide into `open-gaps.md`, with options and a
recommendation.

Exit: every template section has content, and every gap has options and a
recommendation.

### 3. Spec review

Run review rounds on the spec, as `.agents/rules/review-standard.md` says.

Exit: the round exit check passes on the current head.

### 4. Gate 1: owner approves the spec

Send the owner the spec path and the list of open gaps. The owner decides each
gap and approves the spec. Write each decision into `open-gaps.md` (status
`decided by owner`, with the date), and write `Gate 1` with the owner's words
into the state block.

Exit: the owner said the spec is approved, and no gap is `open`.

### 5. Plan

Write `plans/plan.md`. Each task is one commit, has a checkbox, names its files,
and ends on a check from `docs/testing.md`. Add these tasks where they apply:

1. **Docs task.** A task that changes behavior has a docs task right after it,
   which updates the central doc that describes that behavior. The central docs
   always match the code on `main`.
2. **Changelog task.** A user-visible change adds a line under "Unreleased" in
   `CHANGELOG.md`.
3. **Release task.** For a release, the last task runs
   `node scripts/sync-version.js <version>` with the version from the spec, and
   moves "Unreleased" in `CHANGELOG.md` under that version. A round fix after
   the release task changes no version file, and its changelog line goes under
   the new version.

Exit: every decision in the spec and in `open-gaps.md` maps to a plan task.

### 6. Implement

Delegate the plan tasks to implementers, as
`.agents/rules/delegation-standard.md` says. One implementer writes to the
branch at a time. After each task:

1. Run the boundary check in `.agents/rules/delegation-standard.md`.
2. Read the diff, and run the checks from `docs/testing.md` for each changed
   component. A task passes when its checks pass with zero warnings.

When an implementer reports a gap, decide it:

1. The design answers it, or the choice touches only the files the plan task
   names and changes no file in `contracts/` or `policy/`, no public interface,
   no authorization or sandbox behavior, and no dependency: pick the
   recommended option, write the gap as `closed by recommendation` in
   `open-gaps.md`, and continue. The owner reviews these at gate 2.
2. Any other gap: write it as `open`, stop the task, and ask the owner. A
   decision that changes the spec returns the task to gate 1.

When a child crashes with uncommitted files, read `git status` and the diff.
Commit the files that complete a task, as that task's commit. Restore the rest
with `git restore <paths>`.

When every task is done, run the **CI gate**: push, start the native gate
(`scripts/gh workflow run native-e2e-gate.yml --ref <branch> -f pr_number=<number>`),
and wait until the CI gate check passes. When a check fails, read
its log, fix the cause as a new plan task through this stage, push, and run the
CI gate again. A flaky job gets one rerun, as the review rule says.

Exit: `grep -c -E '^- \[ \]' docs/tasks/task-PR-<number>/plans/plan.md` prints 0, `git status --porcelain`
prints nothing, and the CI gate check passes on the pushed head.

### 7. Review rounds

Mark the pull request ready (`scripts/gh pr ready <number>`). Run review rounds
on the code, as `.agents/rules/review-standard.md` says.

Exit: the round exit check passes on the current head. Write that head SHA into
`reviews.md` as the reviewed head.

### 8. Gate 2: owner approves the pull request

1. Write the retro in `retro.md`, as `.agents/rules/task-records.md` says.
2. Send the owner the pull request, the last verdict, the check results, every
   gap with status `closed by recommendation` or `deferred`, and the retro.
3. When the owner says "merge", write `Gate 2` with the owner's words into the
   state block, and the owner's choice for each retro proposal into
   `retro.md`. Commit and push. This push touches only
   `docs/tasks/task-PR-<number>/`.
4. Start the native gate on the new head:
   `scripts/gh workflow run native-e2e-gate.yml --ref <branch> -f pr_number=<number>`.
5. Wait until the CI gate check passes, and the review exit check still holds
   for the reviewed head.
6. Make sure that `git diff --name-only <reviewed head> HEAD` lists only paths
   under `docs/tasks/task-PR-<number>/`. Any other path opens a new round.
7. Merge: `scripts/gh pr merge <number> --merge`.

Exit: `scripts/gh pr view <number> --json mergedAt,mergeCommit` shows the merge,
and the retro exit check in `.agents/rules/task-records.md` passes.

### 9. After the merge

For a release:

1. Start the package run on `main`: `scripts/gh workflow run package.yml --ref main`.
2. Find the run:
   `scripts/gh run list --workflow package.yml --branch main --limit 1 --json databaseId,headSha`.
   Make sure that its `headSha` equals the merge commit.
3. Wait with `scripts/gh run watch <run-id> --exit-status` (in the background,
   it takes longer than 8 minutes). The jobs `Build (macOS)`, `Build (Ubuntu)`
   and `Build (Windows)` must pass.
4. Download the artifacts (`scripts/gh run download <run-id>`), and create the
   tag and the release with the assets and the changelog section as notes:
   `scripts/gh release create v<version> --target <merge commit> --title "Nook v<version>" --notes-file <notes> <assets>`.

Exit: the merge commit is on `main`, and for a release
`scripts/gh release view v<version>` lists every asset.

## Push

Push with `git push`. When the push is rejected, run `git fetch`, merge
`origin/<branch>`, run the checks, and push again. Never force-push.

## Resume

To resume a task, find the pull request:
`scripts/gh pr list --head <branch> --state all --json number,state,mergedAt`.
If the branch exists but has no pull request, continue at stage 1 step 3. If
the pull request is merged, continue at stage 9. Otherwise read the state block, `git log`,
`git status`, the plan checkboxes, `reviews.md`, the reviews and
the CI gate check. Continue at the first stage whose exit check fails.

## Small change

A small change changes no behavior in `daemon/`, `worker/`, `panel/`,
`contracts/` or `policy/` (for example a typo, a CI version, a doc fix). When
the owner calls a task a small change, skip stages 2 to 4: the spec is one
paragraph at the top of `plans/plan.md`, and `open-gaps.md` records the owner's
words. Stages 1 and 5 to 9 still run.
