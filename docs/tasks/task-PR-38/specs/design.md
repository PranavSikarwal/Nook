# Spec: SDLC Work Standard

```
Stage: 6 implement (retro proposals, round 2)
Gate 1: approved by owner on 2026-11-13: "approve"
Gate 2: pending
```

Branch: `chore/agent-workflow-rules`. Pull request: #38.

## Why

The workflow for a change lives in the owner's chat messages, in the global
`~/.claude/CLAUDE.md` and in Claude's memory. Codex, Antigravity and a new
session see only part of it. Steps get skipped: a plan starts before the design
is approved, review rounds answer one source at a time, and decisions made
during the work are not written down.

This task puts the workflow into the repository as one rule, the SDLC Work
Standard, that every agent harness loads. Each pull request then runs the same
stages, and the owner reviews the same three things each time: the spec with
its open gaps, the gaps closed during the work, and the final pull request.

## The rules

The four rule files are the spec of this task. Review them.

1. `.agents/rules/sdlc-work-standard.md`: the roles, the CI gates, the nine
   stages with their exit checks, push, resume and small changes. It is always
   loaded.
2. `.agents/rules/task-records.md`: the task folder, the state block, the open
   gaps format and the retro, with its exit check.
3. `.agents/rules/review-standard.md`: which reviewer to use (the T3 reviewer or
   the GitHub Actions reviewer in `reviewers.yml`), the verdict line, how to ask
   each reviewer, how to read the inputs, when rounds stop to ask the owner, the
   round exit check, and what to do when an input fails.
4. `.agents/rules/delegation-standard.md`: the implementer rules, the T3 Code
   rules and the boundary check after each task.

The SDLC file points to the other three before the stage that needs each one.
Each file stays under 12,000 characters, the limit that Antigravity sets for one
rule file.

## Where the rule lives

Each rule has one source in `.agents/rules/`. Each harness reaches the rules
in its own way:

| Harness | What it loads | How it reaches the rule |
| --- | --- | --- |
| Antigravity | In its own app: `AGENTS.md`, `GEMINI.md`, `.agents/rules/*.md`. Through T3: no workspace rule (gap G13). | In its own app, it loads the files directly. Through T3, the delegate brief tells it to read the rules first. The SDLC file sets `trigger: always_on`. The other three files set `trigger: model_decision`, so the agent loads each one when its description fits. |
| Claude Code | `CLAUDE.md` with `@` imports, `.claude/rules/` | `CLAUDE.md` imports `@.agents/rules/sdlc-work-standard.md`. The other three files are read through the pointers in the SDLC file. |
| Codex | `AGENTS.md` only, no imports | `AGENTS.md` starts with a pointer that tells the agent to read the rule before any work. Every implementer brief also names the rule. |

Cursor reads `AGENTS.md` and `.cursor/rules/`. Nook does not use Cursor, so it
gets no extra pointer.

## Rule changes

The implementation changes these files so that no two documents describe the
workflow differently. The rule changes land in the same commit as the
pointers.

| File | Old text | New text |
| --- | --- | --- |
| `AGENTS.md` "Where things are" | `docs/plans/` holds active implementation plans. `docs/design/` holds proposed designs. | Each task has `docs/tasks/task-PR-<number>/`. `docs/plans/` and `docs/design/` keep only the plans and designs that started before PR 38. |
| `AGENTS.md` "Planning rules" | Always write plans inside `docs/plans/`. | Follow `.agents/rules/sdlc-work-standard.md`. The spec and the plan live in the task folder. |
| `AGENTS.md` top | none | A pointer: read `.agents/rules/sdlc-work-standard.md` before any work. |
| `AGENTS.md` "Done" 3 | If the plan is wrong or a change outside it is needed, stop and report. | Unchanged, plus: the rule's stage 6 says when the orchestrator may close a gap by recommendation. |
| `AGENTS.md` "Where things are" | `docs/plan.md` holds the active work, and `CHECKLIST.md` tracks it. | `docs/plan.md` and `CHECKLIST.md` track the work that started before PR 38. A new task is tracked by its task folder and its pull request. |
| `AGENTS.md` "Issue tracker" | Issues and specs live as local markdown under `.scratch/`. | Issues live under `.scratch/`. The spec of a task lives in its task folder. |
| `docs/agents/issue-tracker.md` | The spec is `.scratch/<feature-slug>/spec.md`. | Adds: a task that becomes a pull request moves its spec into `docs/tasks/task-PR-<number>/specs/design.md`. |
| `CLAUDE.md` | `@AGENTS.md` | `@AGENTS.md` and `@.agents/rules/sdlc-work-standard.md`. |
| `CONTRIBUTING.md` "Branches and commits" | Do not add author-attribution trailers or generator notices to commits. | No AI attribution in a commit, a pull request title or body, a comment or a review. |
| `CONTRIBUTING.md` "Pull requests" | Update `docs/plan.md` and `CHECKLIST.md` when the change completes or adds an active task. | Follow `.agents/rules/sdlc-work-standard.md`. Update `docs/plan.md` and `CHECKLIST.md` only for work that started before PR 38. |
| `CONTRIBUTING.md` | Keep completed plans in `docs/archive/plans/`. | Task folders stay in `docs/tasks/` after the merge. Completed plans from `docs/plans/` still move to `docs/archive/plans/`. |
| `docs/testing.md` | No check for a change that touches only Markdown. | Adds `git diff --check origin/main HEAD`, run after the commit, as the check for a Markdown-only change. |
| `docs/plans/README.md` | Lists active plans. | Adds: new work uses task folders under `docs/tasks/`. |

`CHECKLIST.md` and `docs/plan.md` keep their content. This pull request merges
only after every change in this table is in it, so no agent reads two opposite
rules on `main`.

## Implementation scope

1. Add `docs/tasks/_template/` with `specs/design.md`, `plans/plan.md`,
   `open-gaps.md`, `reviews.md` and `retro.md`.
2. Make the changes in "Rule changes".
3. Check in a live Antigravity run that the rule loads (gap G2), and record the
   result.

This pull request applies every row of the "Rule changes" table.

Not in scope: moving the existing plans and designs (gap G4), a CI check for the
task folder (gap G6), the `reviewers.yml` fixes (gap G11), and product code.

Applied through gap G16: retro proposals P1, P2 and P4 (plan tasks 15 to 17),
with the code round 3 refinements (plan tasks 18 to 21). P3, P5 and P6 are
rejected.

## This task is the first one

This task opened its pull request before the rule existed. Its first commit
already held the task folder, because the orchestrator read the next pull
request number from GitHub first. Later tasks follow stage 1 as written.

## Risks

1. The SDLC rule is always loaded, so it costs context on every turn. The other
   three files load only when their stage needs them.
2. Rules in two places drift. The global `~/.claude/CLAUDE.md` keeps rules for
   all projects. Gap G5 decides what moves.
3. Two gates slow small fixes. The "Small change" section skips the spec and
   gate 1 when the owner asks.
