---
trigger: model_decision
description: Delegation Standard. Read it before you delegate plan tasks to a child agent or write an implementer brief.
---

# Delegation Standard

The orchestrator delegates plan tasks to implementers and verifies each one.
`.agents/rules/sdlc-work-standard.md` stage 6 says when.

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

## Boundary check

Before you start an implementer, save the state:

```
git fetch origin
git rev-parse origin/<branch> > <scratch>/before-sha
git ls-remote --tags origin > <scratch>/before-tags
scripts/gh pr view <number> --json state,comments,reviews > <scratch>/before-pr
scripts/gh run list --branch <branch> --json databaseId > <scratch>/before-runs
```

Before each task, save the local commit: `git rev-parse HEAD > <scratch>/task-base`.
After each task, run `git fetch origin` and make sure that:

1. `git rev-parse origin/<branch>` still equals `before-sha`.
2. `git ls-remote --tags origin` still equals `before-tags`.
3. The `pr view` and `run list` output still equals `before-pr` and
   `before-runs`.
4. `git diff --name-only <task-base> HEAD` lists only the files the task names.

These checks catch a push, a tag, a merge, a comment, a review, a workflow run
and a change outside the task. The bans on `scripts/gh` and `.env` themselves
rest on the instruction alone.
