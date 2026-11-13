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

## Roles

| Role | Who | Does |
| --- | --- | --- |
| Owner | The human who owns the repository | Decides open gaps, approves gate 1 and gate 2, says "merge". |
| Orchestrator | The agent the owner talks to | Writes the spec, the plan, `open-gaps.md` and `reviews.md`. Delegates, verifies, pushes, starts workflows, asks the reviewer, merges after the owner says "merge", runs the release. |
| Implementer | A child agent that the orchestrator starts | Implements the plan tasks it is given and commits locally. Follows "Implementer rules" below. |
| Reviewer | The T3 thread "Review Pull Requests" (`pr-reviewer` agent) | Reviews the spec and the code, and posts one review per round on the pull request. |

The orchestrator is the only agent that pushes, runs `scripts/gh`, edits the
task folder, or changes the pull request.

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
Head: <commit SHA of the last push>
Gate 1: <approved by owner on YYYY-MM-DD at SHA | pending>
Gate 2: <approved by owner on YYYY-MM-DD at SHA | pending>
```

Update the state block in every commit that moves the task to another stage.

## Stages

### 1. Start

1. Create a branch from `main` with a conventional prefix (`feat/`, `fix/`,
   `docs/`, `chore/`), named after the work.
2. Commit an empty commit: `git commit --allow-empty -m "chore: open task"`.
3. Push, and open a draft pull request.
4. Copy `docs/tasks/_template/` to `docs/tasks/task-PR-<number>/`, commit, push.

Exit: the draft pull request exists, and its branch holds the task folder.

### 2. Spec

Write `specs/design.md`: why, scope, the design, the risks, and a "Rule
changes" table when the task changes a rule or a binding document. Name each
central doc the task changes (`docs/architecture.md`, `docs/contracts.md`,
`docs/tools-and-authorization.md`, `docs/mcp-servers.md`, `docs/release.md`,
`docs/testing.md`). Write each question that the owner must decide into
`open-gaps.md`, with options and a recommendation.

Exit: every template section has content, and every gap has options and a
recommendation.

### 3. Spec review

Run one review round (see "Review round") on the spec.

Exit: the round exit check passes for the spec.

### 4. Gate 1: owner approves the spec

Send the owner the spec path and the list of open gaps. The owner decides each
gap and approves the spec. Write each decision into `open-gaps.md` (status
`decided by owner`, with the date), and write `Gate 1` into the state block.

Exit: the owner said the spec is approved, and no gap from stages 2 and 3 is
`open`.

### 5. Plan

Write `plans/plan.md`. Each task is one commit, has a checkbox, names its files,
and ends on a check from `docs/testing.md`. Add these tasks where they apply:

1. **Docs task.** A task that changes behavior has a docs task right after it,
   which updates the central doc that describes that behavior. The central docs
   always match the code on `main`.
2. **Release task.** A release task is the last task. It bumps the application
   version (`scripts/sync-version.js`) and moves "Unreleased" in `CHANGELOG.md`
   under the new version. A user-visible change adds a `CHANGELOG.md` line in
   its own task.

Exit: every decision in the spec and in `open-gaps.md` maps to a plan task.

### 6. Implement

Delegate the plan tasks to implementers (see "T3 Code" for the tools). One
implementer writes to the branch at a time. After each task, verify the commit
yourself: read the diff, and run the checks from `docs/testing.md` for each
changed component. A task passes when its checks pass with zero warnings.

When an implementer reports a gap, decide it:

1. The design answers it, or the choice stays inside the plan task and changes
   no file in `contracts/` or `policy/`, no public interface, no authorization or
   sandbox behavior, and no dependency: pick the recommended option, write the
   gap as `closed by recommendation` in `open-gaps.md`, and continue. The owner
   reviews these at gate 2.
2. Any other gap: write it as `open`, stop the task, and ask the owner. A
   decision that changes the spec returns the task to gate 1.

Exit: every plan box is ticked, every check passes, and `git status` is clean.

### 7. Review rounds

Mark the pull request ready for review, then run review rounds until a round
exit check passes. Run at most 3 rounds. If round 3 ends with an open P0, P1 or
P2 finding, stop and ask the owner.

Exit: the round exit check passes on the current head.

### 8. Gate 2: owner approves the pull request

Send the owner the pull request, the last review verdict, the check results,
and every gap with status `closed by recommendation` or `deferred`. The owner
reviews them. When the owner says "merge", write `Gate 2` into the state block,
push, wait for the checks on that head, and merge with a merge commit.

Exit: the owner said "merge", and the pull request is merged.

### 9. After the merge

1. For a release, run the Package workflow by hand on `main` at the merge
   commit. Create the tag and the GitHub release only when the run passes on
   macOS, Ubuntu and Windows.
2. Set the state block to `Stage: 9 merged` in the next pull request that
   touches the folder, or leave it at stage 8 when no later pull request touches
   it.

Exit: the merge commit is on `main`, and the release exists when the task is a
release.

## Review round

A round collects three **inputs** for one head commit, then answers all of them
in one fix pass. Never fix one input while another is still missing.

1. Open the round: add `## Round N` to `reviews.md`, commit, and push. The
   commit that adds the heading is the round's head. Read its SHA from
   `headRefOid` and write it into the round with the inputs in step 4.
2. Start the inputs on that head:
   1. Ask the reviewer (see "T3 Code").
   2. Start the native gate: `scripts/gh workflow run native-e2e-gate.yml --ref
      <branch> -f pr_number=<number>`.
3. Wait until all inputs exist for that head, for at most 60 minutes:
   1. The reviewer's review, whose `commit_id` is the head SHA.
   2. These four checks, completed: `Code Quality and Tests`, `Windows named-pipe
      IPC`, `SonarCloud Code Analysis`, `Native Tauri E2E Gate`. Read them with
      `scripts/gh pr view <number> --json headRefOid,statusCheckRollup`.
4. Write every finding into the round: each reviewer finding, each SonarCloud
   issue on new code, and each failed job. Give each one action: `fix`,
   `defer` (a gap with status `deferred`), or `reject` (with the reason).
5. Append the `fix` items to `plans/plan.md` under `## Round N`, one task per
   commit, and implement them through stage 6.
6. Push. The new head starts the next round.

A round exit check passes when all of these hold on the current head:

1. The newest review on this head starts with `Verdict: APPROVE`, or counts
   `P0: 0, P1: 0, P2: 0` open.
2. Every P2 is fixed, or deferred or rejected with a reason in `reviews.md`.
3. Every deferred finding is a gap in `open-gaps.md`.
4. All four checks are `success`.

The reviewer posts as a comment because the owner cannot approve their own pull
request. The first line of the review holds the verdict.

### When an input fails to arrive

1. **No review after 30 minutes.** Send the reviewer one reminder. If 15 more
   minutes pass with no review, tell the owner and wait.
2. **A review on an older head.** Ignore it, and ask again for the current head.
3. **A failed job.** Rerun it once. Write the rerun into the round as `flake` with
   the job name and run id. A second failure is a finding.
4. **A check pending or missing after 60 minutes.** Start it again once, then tell
   the owner.
5. **A SonarCloud issue with no message.** Read the SonarCloud page for the pull
   request. If the issue still has no text, write the rule key and the file into
   the round and ask the owner.
6. **`main` moved and the branch conflicts.** Merge `main` into the branch, push,
   and open a new round on the new head.

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
the contents of `.env`.

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
   see (the review on GitHub, CI), as one command with a limit of 60 minutes.
3. **Check progress.** When a wait times out, read the child's activity, `git
   log` and `git status`. If the child made progress, wait again. If not, send
   one reminder with `t3_thread_send` that names the next plan task. If it still
   made no progress at the next check, stop it with `task_cancel`, and finish the
   task yourself or delegate a smaller brief.
4. **Take over.** When a child's harness crashes or the child stops early, verify
   what it committed and continue from there. When its commits exist but no
   report arrives, verify the commits and continue.
5. **Ask the reviewer.** Find the thread "Review Pull Requests" with
   `t3_thread_search`. If the search finds no thread or several, ask the owner.
   Send the request with `t3_thread_send`: the pull request URL, the head SHA,
   and whether it is a spec review or a code review. The reviewer's run ends
   before its subagent posts the review, so after `t3_thread_wait`, read the
   reviews on the pull request.

## Resume

To resume a task, read the state block, `git log`, `git status`, the plan
checkboxes, `reviews.md`, and `scripts/gh pr view <number> --json
headRefOid,statusCheckRollup,reviews`. Continue at the first stage whose exit
check fails.

## Small change

A small change changes no behavior in `daemon/`, `worker/`, `panel/`,
`contracts/` or `policy/` (for example a typo, a CI version, a doc fix). When
the owner calls a task a small change, skip stages 2 to 4: the spec is one
paragraph at the top of `plans/plan.md`, and `open-gaps.md` records the owner's
words. Stages 1 and 5 to 9 still run.
