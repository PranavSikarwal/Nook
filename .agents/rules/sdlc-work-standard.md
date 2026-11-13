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

Reviews follow `.agents/rules/review-standard.md`. Read it before stage 3 and
before stage 7.

## Roles

| Role | Who | Does |
| --- | --- | --- |
| Owner | The human who owns the repository | Decides open gaps, approves gate 1 and gate 2, says "merge". |
| Orchestrator | The agent the owner talks to | Writes the spec, the plan, `open-gaps.md` and `reviews.md`. Delegates, verifies, pushes, starts workflows, asks the reviewer, merges after the owner says "merge", runs the release. |
| Implementer | A child agent that the orchestrator starts | Implements the plan tasks in its brief and commits locally. Follows "Implementer rules". |
| Reviewer | Chosen by `.agents/rules/review-standard.md` | Reviews the spec and the code, as "What the reviewer may do" in that file says. |

The orchestrator is the only agent that pushes, merges, tags, starts workflows,
or edits the task folder.

No commit, pull request title or body, comment or review holds AI attribution:
no `Co-Authored-By` line for an AI and no "Generated with" line.

## Task folder

```
docs/tasks/task-PR-<number>/
  specs/design.md   the spec: why, what changes, options, risks, rule changes
  plans/plan.md     one task per commit, each with a checkbox
  open-gaps.md      every gap and decision, from spec to merge
  reviews.md        one entry per review round
```

`docs/tasks/_template/` holds an empty copy of each file. The top of
`specs/design.md` holds the **state block**:

```
Stage: <1-9 and name>
Gate 1: <pending | approved by owner on YYYY-MM-DD: "<owner's words>">
Gate 2: <pending | approved by owner on YYYY-MM-DD: "<owner's words>">
```

Update the state block in every commit that moves the task to another stage.
The head commit is always `headRefOid` of the pull request.

A design that needs more than one pull request gets one task folder per pull
request. The later spec links to the first spec and covers only its own part.

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

Exit: a round exit check passes on the current head.

### 4. Gate 1: owner approves the spec

Send the owner the spec path and the list of open gaps. The owner decides each
gap and approves the spec. Write each decision into `open-gaps.md` (status
`decided by owner`, with the date), and write `Gate 1` with the owner's words
into the state block.

Exit: the owner said the spec is approved, and no gap is `open`.

### 5. Plan

Write `plans/plan.md`. Each task is one commit, has a checkbox, names its files,
and ends on a check from `docs/testing.md` (`git diff --check` for a task that
changes only Markdown). Add these tasks where they apply:

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

Delegate the plan tasks to implementers (see "T3 Code"). One implementer writes
to the branch at a time. After each task:

1. Check the boundary. `git rev-parse origin/<branch>` still equals your last
   push, `git ls-remote --tags origin` is unchanged, and
   `git diff --name-only <before> HEAD` lists only the files the task names.
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

Exit: `grep -c -- '- \[ \]' plans/plan.md` prints 0, every check passes, and
`git status --porcelain` prints nothing.

### 7. Review rounds

Mark the pull request ready (`scripts/gh pr ready <number>`). Run review rounds
on the code, as `.agents/rules/review-standard.md` says.

Exit: a round exit check passes on the current head. Write that head SHA into
`reviews.md` as the reviewed head.

### 8. Gate 2: owner approves the pull request

1. Send the owner the pull request, the last verdict, the check results, and
   every gap with status `closed by recommendation` or `deferred`.
2. When the owner says "merge", write `Gate 2` with the owner's words into the
   state block. Commit and push. This push touches only
   `docs/tasks/task-PR-<number>/`.
3. Start the native gate on the new head:
   `scripts/gh workflow run native-e2e-gate.yml --ref <branch> -f pr_number=<number>`.
4. Wait until `scripts/gh pr checks <number>` exits 0.
5. Make sure that `git diff --name-only <reviewed head> HEAD` lists only paths
   under `docs/tasks/task-PR-<number>/`. Any other path opens a new round.
6. Merge: `scripts/gh pr merge <number> --merge`.

Exit: `scripts/gh pr view <number> --json mergedAt,mergeCommit` shows the merge.

### 9. After the merge

For a release:

1. Start the package run on `main`: `scripts/gh workflow run package.yml --ref main`.
2. Make sure that the run's `headSha` (`scripts/gh run view <run-id> --json headSha`)
   equals the merge commit.
3. Wait until the jobs `Build (macOS)`, `Build (Ubuntu)` and `Build (Windows)`
   pass.
4. Download the artifacts (`scripts/gh run download <run-id>`), and create the
   tag and the release with the assets and the changelog section as notes:
   `scripts/gh release create v<version> --target <merge commit> --title "Nook v<version>" --notes-file <notes> <assets>`.

Exit: the merge commit is on `main`, and for a release
`scripts/gh release view v<version>` lists every asset.

## Open gaps

`open-gaps.md` holds one entry per gap:

```
## G<n>. <title>

- Stage: <stage that found it>
- Question:
- Options:
- Recommendation:
- Decision:
- Decided by: <owner | orchestrator by recommendation>
- Date: <YYYY-MM-DD>
- Status: <open | decided by owner | closed by recommendation | deferred>
```

Only the orchestrator writes this file. A requirement that the owner changes
during the task is a new gap. If it changes the spec, the task returns to gate 1.

## Implementer rules

Put this section, or the path to it, into every implementer brief.

1. Read the plan and the spec. Implement only the tasks in your brief, in order.
2. Write the failing test first, as `AGENTS.md` "Tests" says.
3. Make one commit per task. Stage only the files the task names
   (`git add <paths>`). Tick the task's box in the same commit.
4. Follow `CONTRIBUTING.md` "Branches and commits" and `AGENTS.md` "GitHub CLI
   and commit rules". A commit message holds no AI attribution.
5. Commit locally. The orchestrator pushes.
6. When the plan does not answer a question, stop and report the question, the
   options and your recommendation in your final message.
7. Run a command that can take more than 8 minutes in the background.
8. End with a report: the commits, the checks you ran and their results, and
   each gap you met.

An implementer keeps out of `scripts/gh`, `git push`, merges, tags, workflow
runs, review threads, the task folder files other than the plan checkbox, and
the contents of `.env`. The brief passes no token and no `.env` value.

## T3 Code

The orchestrator delegates and waits with the T3 Code tools.

1. **Delegate.** Call `orchestrator_capabilities` once per thread. Start each
   implementer with `delegate_task`: provider `codex`, model `gpt-6-luna`,
   `reasoningEffort: high`, `runtimeMode: full-access`, `isolation: shared`,
   unless the owner names another model. The brief names the plan path, the task
   numbers, a time box, and "Implementer rules" in this file.
2. **Wait.** Wait for a child with `t3_thread_wait` (`untilDelegatedWorkSettled:
   true`, timeout 15 to 30 minutes). Wait for a background command with
   `t3_background_wait`. Use a shell loop only for external state that T3 cannot
   see (a review on GitHub, CI), as one command of at most 30 minutes.
3. **Check progress.** When a wait times out, read the child's activity, `git
   log` and `git status`. If the child made progress, wait again. If not, send
   one reminder with `t3_thread_send` that names the next plan task. If it still
   made no progress at the next check, stop it with `task_cancel`, and finish the
   task yourself or delegate a smaller brief.
4. **Take over.** When a child's harness crashes or the child stops early, verify
   what it committed and continue from there. When its commits exist but no
   report arrives, verify the commits and continue.

## Push

Push with `git push`. When the push is rejected, run `git fetch`, merge
`origin/<branch>`, run the checks, and push again. Never force-push.

## Resume

To resume a task, find the pull request (`scripts/gh pr list --head <branch>`).
If none exists, start at stage 1. Otherwise read the state block, `git log`,
`git status`, the plan checkboxes, `reviews.md`, the reviews and
`scripts/gh pr checks <number>`. Continue at the first stage whose exit check
fails.

## Small change

A small change changes no behavior in `daemon/`, `worker/`, `panel/`,
`contracts/` or `policy/` (for example a typo, a CI version, a doc fix). When
the owner calls a task a small change, skip stages 2 to 4: the spec is one
paragraph at the top of `plans/plan.md`, and `open-gaps.md` records the owner's
words. Stages 1 and 5 to 9 still run.
