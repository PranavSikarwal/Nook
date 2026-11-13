---
trigger: model_decision
description: Task Records. Read it before you create or update a task folder, an open gap, the state block or a retro.
---

# Task Records

The task folder is the record of one task. `.agents/rules/sdlc-work-standard.md`
says when each file changes. Only the orchestrator writes these files, except
the plan checkbox, which an implementer ticks.

## Task folder

```
docs/tasks/task-PR-<number>/
  specs/design.md   the spec: why, what changes, options, risks, rule changes
  plans/plan.md     one task per commit, each with a checkbox
  open-gaps.md      every gap and decision, from spec to merge
  reviews.md        one entry per review round
  retro.md          what went wrong in the workflow, why, proposals
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

## Retro

The retro improves this framework, not the product. Write `retro.md` from the
record: `reviews.md`, `open-gaps.md`, the plan, the T3 thread activity, and the
owner's corrections during the task. It has three sections, with these exact headings:

1. `## What went wrong`: Each event where the workflow failed or cost time: a
   skipped or repeated step, a stalled child, a round that a rule could have
   prevented, a command in a rule that did not work, an owner correction. Give
   the evidence (commit, round, run id, owner's words).
2. `## Why`: The cause in the rules or the tools for each event: a missing
   step, a vague exit check, a wrong command, a missing failure case.
3. `## Proposals`: One change to `.agents/rules/` per cause, with the file, the
   section and the new text. At gate 2, end each proposal with a line
   `Owner: accepted` or `Owner: rejected`.

Write each accepted proposal as a gap with status `deferred` in
`open-gaps.md`. The next task that the owner starts after the merge is a small
change that applies them. When a task went without a problem, write "No
problem" under each heading.

The retro exit check:

```
f=docs/tasks/task-PR-<number>/retro.md
[ "$(grep -c -E '^## (What went wrong|Why|Proposals)$' "$f")" = 3 ] &&
[ "$(grep -c -E '^Owner: (accepted|rejected)$' "$f")" = "$(grep -c -E '^### ' "$f")" ]
```

Each proposal is a `### ` heading under `## Proposals`.

