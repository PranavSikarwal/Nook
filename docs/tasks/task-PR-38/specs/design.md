# Spec: SDLC Work Standard

```
Stage: 3 spec review
Head: see the pull request
Gate 1: pending
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

## The rule

The rule itself is the spec of this task:
`.agents/rules/sdlc-work-standard.md`. Review that file. It defines the roles,
the task folder, the nine stages with their exit checks, the review round, the
open gaps format, the implementer rules, the T3 Code rules, resume, and small
changes.

## Where the rule lives

The rule has one source, `.agents/rules/sdlc-work-standard.md`. Each harness
reaches it in its own way:

| Harness | What it loads | How it reaches the rule |
| --- | --- | --- |
| Antigravity | `AGENTS.md`, `GEMINI.md`, `.agents/rules/*.md` | Loads the file directly. The frontmatter sets `trigger: always_on`. |
| Claude Code | `CLAUDE.md` with `@` imports, `.claude/rules/` | `CLAUDE.md` imports `@.agents/rules/sdlc-work-standard.md`. |
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
| `CLAUDE.md` | `@AGENTS.md` | `@AGENTS.md` and `@.agents/rules/sdlc-work-standard.md`. |
| `CONTRIBUTING.md` | Keep completed plans in `docs/archive/plans/`. | Task folders stay in `docs/tasks/` after the merge. Completed plans from `docs/plans/` still move to `docs/archive/plans/`. |
| `docs/plans/README.md` | Lists active plans. | Adds: new work uses task folders under `docs/tasks/`. |

`CHECKLIST.md` and `docs/plan.md` link to old plans only. They stay unchanged.

## Implementation scope

1. Add `docs/tasks/_template/` with `specs/design.md`, `plans/plan.md`,
   `open-gaps.md` and `reviews.md`.
2. Make the changes in "Rule changes".
3. Check in a live Antigravity run that the rule loads (gap G2), and record the
   result.

Not in scope: moving the existing plans and designs (gap G4), a CI check for the
task folder (gap G6), and product code.

## This task is the first one

This task opened its pull request before the rule existed. Its first commit
already held the task folder, because the orchestrator read the next pull
request number from GitHub first. Later tasks follow stage 1 as written.

## Risks

1. The rule is always loaded, so it costs context on every turn. It is one file
   of about 300 lines. If it proves too long, the implementer and T3 sections
   move to their own files.
2. Rules in two places drift. The global `~/.claude/CLAUDE.md` keeps rules for
   all projects. Gap G5 decides what moves.
3. Two gates slow small fixes. The "Small change" section skips the spec and
   gate 1 when the owner asks.
