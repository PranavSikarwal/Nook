# Design: one task workflow for every pull request

Status: proposed. Waits for the owner's review and approval (gate 1).

Branch: `chore/agent-workflow-rules`. Pull request: #38.

## Why

Today the workflow lives in three places: the owner's chat messages, the global
`~/.claude/CLAUDE.md`, and Claude's memory files. A new agent, another provider
(Codex, Antigravity) or a new session does not see all of it. Steps get skipped:
a plan starts before the design is approved, review rounds run one source at a
time, and decisions made during the work are not written down.

This change puts the workflow in the repository, as rules that every agent
provider loads. Every pull request then follows the same stages, and the owner
reviews the same three things each time: the design, the open gaps file, and
the final pull request.

## Scope

In scope:

1. A rule set in `.agents/rules/` that defines the stages, the gates and the
   roles (orchestrator, implementer, reviewer).
2. A task folder for each pull request, `docs/tasks/task-PR-<number>/`, with a
   template.
3. Pointers so that Claude Code, Codex and Antigravity load the rules.
4. The T3 Code rules for delegating and waiting.
5. A rule that the central docs (`docs/architecture.md`, `docs/contracts.md`,
   `docs/tools-and-authorization.md`, `docs/mcp-servers.md`, `docs/release.md`,
   `docs/testing.md`) change in the same pull request as the code.
6. Changes to `AGENTS.md` ("Planning rules", "Where things are") and
   `docs/plans/README.md`, so that no two documents describe the workflow.

Not in scope: moving the existing plans in `docs/plans/` and the designs in
`docs/design/` into task folders (see gap G4), and any product code.

## The task folder

Each pull request gets one folder. The orchestrator (the agent that runs the
pull request) creates it right after it opens the pull request, because the
folder name needs the number.

```
docs/tasks/task-PR-<number>/
  design.md      what changes and why, options, risks; approved by the owner
  open-gaps.md   every gap and decision, from design to merge
  plans/         implementation plans, one task per commit, with checkboxes
  specs/         exact specs a delegate implements (contracts, rule text, tests)
  reviews.md     one entry per review round: inputs, actions, follow-up result
```

`docs/tasks/_template/` holds an empty copy of each file with its headings.

## The stages

Each stage ends on a completion criterion that the next stage checks.

1. **Start.** Create a branch named for the work from `main`. Open a draft pull
   request. Create `docs/tasks/task-PR-<number>/` from the template. Done when
   the pull request exists and its first commit holds the folder.
2. **Design.** The orchestrator writes `design.md` and fills `open-gaps.md` with
   each question that the owner must decide, each with options and a
   recommendation. Done when every section of the template has content.
3. **Design review.** The orchestrator pushes and asks the reviewer to review the
   design. It fixes the findings in one round and records the round in
   `reviews.md`. Done when the reviewer has no open P0 to P2 finding on the
   design.
4. **Gate 1: owner approval of the design.** The owner reads `design.md` and
   decides each open gap. The orchestrator writes each decision into
   `open-gaps.md` with the date. Nothing in stage 5 starts before the owner says
   the design is approved.
5. **Plan and specs.** The orchestrator writes the plan in `plans/` and any spec
   in `specs/`. Each plan task is one commit with a checkbox. Each plan that
   changes behavior has a task that updates the central docs, next to the task
   that changes the code. Done when every design decision maps to a plan task.
6. **Implementation.** The orchestrator delegates the plan tasks to implementer
   agents (see "T3 Code rules"). Implementers follow the design. When an
   implementer meets a gap the design does not answer, it follows the design's
   intent, picks the recommended option, writes the gap and its decision into
   `open-gaps.md` as "closed by recommendation", and continues. The orchestrator
   verifies each commit: it runs the commands in `docs/testing.md` for each
   changed component. Done when every plan box is ticked and the checks pass.
7. **Review rounds.** The orchestrator marks the pull request ready, pushes, and
   asks the reviewer for a review. Then it waits until all three inputs exist for
   the same head commit: the reviewer's review, the SonarCloud result, and the
   result of every CI job, including the Native Tauri E2E Gate. It does not start
   fixes while an input is missing. It writes one round in `reviews.md` with every
   finding from the three sources and one action for each (fix, defer to a gap,
   or reject with a reason). It delegates all fixes at once, verifies, pushes,
   and asks the reviewer for a follow-up review. Done when the reviewer has no
   open P0 to P2 finding, the SonarCloud gate passes, and every required check is
   green.
8. **Gate 2: owner approval of the pull request.** The orchestrator reports the
   pull request, the review result and the list of gaps "closed by
   recommendation". The owner reviews them. The orchestrator merges only after
   the owner says to merge. It uses a merge commit.
9. **After the merge.** If the pull request is a release, follow the release rule
   in `AGENTS.md` (package run on `main`, then the tag). Mark the task folder
   "merged" at the top of `design.md`.

## Open gaps file

`open-gaps.md` is the single record of every question and decision for the pull
request. Each entry has an ID (`G1`, `G2`), the stage that found it, the
question, the options, the recommendation, the decision, who decided (owner or
agent), the date, and the status. A status is one of:

1. `open`: waits for the owner.
2. `decided by owner`.
3. `closed by recommendation`: an agent decided it during the work. The owner
   reviews it at gate 2.
4. `deferred`: moved to a later pull request, with the reason.

## Central docs stay in step with the code

The central docs describe current behavior. A pull request that changes
behavior also changes the doc that describes it, in the same pull request. The
plan lists this as its own task next to the code task, and the reviewer checks
it. The task folder records the work. The central docs record the result.

## Where the rules live

One source of truth, with pointers for each provider:

| Provider | What it loads | How it reaches the rules |
| --- | --- | --- |
| Claude Code | `CLAUDE.md` and its `@` imports | `CLAUDE.md` imports `@.agents/rules/workflow.md` and the other rule files, so they are always in context. |
| Codex | `AGENTS.md` (no imports) | `AGENTS.md` opens with a short pointer that tells the agent to read `.agents/rules/` before any work, plus the one rule that matters most for an implementer. |
| Antigravity | `.agent/rules/` and `AGENTS.md` | A file in `.agent/rules/` that points to `.agents/rules/` (see gap G2). |

The rule files split by role, because each role takes a different path:

1. `.agents/rules/workflow.md`: the stages and the gates. Read by the
   orchestrator.
2. `.agents/rules/implementer.md`: what a delegated agent does (follow the plan,
   one commit per task, record gaps, update the central docs). Short, because
   it goes into every delegate brief.
3. `.agents/rules/review-round.md`: how to collect the three inputs, write a round
   in `reviews.md`, and finish the round.
4. `.agents/rules/t3-orchestration.md`: the T3 Code rules.

The rule text is written with the `writing-for-agents` skill: steps with a clear
completion criterion, positive instructions, and one source for each meaning.

## T3 Code rules

These move from the global `~/.claude/CLAUDE.md` and from memory into
`.agents/rules/t3-orchestration.md`:

1. Call `orchestrator_capabilities` once per thread before the first
   `delegate_task`.
2. Delegate a plan task with `delegate_task`: provider `codex`, model
   `gpt-6-luna`, `reasoningEffort: high`, `runtimeMode: full-access`,
   `isolation: shared`, unless the owner names another model. Give the brief the
   plan path, the task numbers, the time box, and `.agents/rules/implementer.md`.
3. Wait with `t3_thread_wait` on the child (`untilDelegatedWorkSettled: true`,
   timeout 15 to 30 minutes), and with `t3_background_wait` for a background
   command. Do not use `sleep` timers.
4. When a wait times out, check real progress: the child's activity, `git log`
   and `git status`. Remind a child that made no progress. Cancel it with
   `task_cancel` at the next check and finish the work yourself or re-delegate a
   smaller brief.
5. When a child's commits exist but no report arrives, verify the commits and
   continue.
6. Ask the reviewer with `t3_thread_send` to the "Review Pull Requests" thread.
   Its run ends before its subagent posts the review, so after the wait, read the
   pull request reviews for the head commit. Waiting on the review, SonarCloud
   and CI is external state, so one bounded shell poll is allowed for it.
7. Run any command that can take more than 8 minutes in the background.

## Risks

1. A long rule set costs context on every turn. The role split keeps each
   agent's load small, and the implementer file stays short.
2. Rules in two places drift. The global `~/.claude/CLAUDE.md` keeps only rules
   for all projects, and points to the repository rules here (see gap G5).
3. The two gates slow small fixes. Gap G3 asks whether a small change can skip
   the design stage.
