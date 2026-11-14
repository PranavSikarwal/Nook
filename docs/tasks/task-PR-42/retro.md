# Retro

## What went wrong

1. The plan said two things about item 12 of the open gaps file (task 3: "item
   12 is closed" and "add the live v0.1.6 Mac check"). The first implementer
   stopped before task 3 and asked. Evidence: its report after tasks 1 and 2,
   gap G6.
2. The plan copied two fixes from review 3 of PR 37 without a look at the code.
   Task 6 asked for "the process group is gone", which a test cannot see, and
   task 7 asked for the supervisor's `env_vars`, which the tests cannot reach.
   Each cost one child run that ended with a question. Evidence: gaps G7 and G8.
3. The `gpt-6-luna` child ended three runs with a status message and no change
   (a 16 second run, then two runs that stopped before the commit). I sent three
   reminders that named the next step and the commit. I ran the final format,
   clippy and test checks of task 6, committed it, and corrected and committed
   task 10 myself, because the child's session closed while `cargo test` ran.
4. The plan held a wrong fact, "PR 32 merged after PR 31". Both landed in merge
   commit `8f378a5`. Round 1 found it as the only P2 (finding C1).
5. `scripts/gh` is in `.gitignore`, so a T3 worktree has no copy. Every command
   in the rules failed until I called the copy in the main checkout.
6. The boundary check compared `pr view` and `run list` output exactly. After my
   own push, the diff showed a SonarCloud bot comment and a new CI run, so the
   check reported changes that no child made.
7. The task 10 text that a child wrote in `docs/testing.md` named "tasks 6 to 9"
   of the plan. A central doc must name tests and files. I rewrote the text
   before the commit.

## Why

1. Stage 5 does not say that each plan task must be consistent with every other
   task and with the facts list.
2. Stage 5 does not say to read the code that a fix or a test names before the
   plan promises it.
3. The implementer rules let a child end a run without a commit or a blocker.
4. Stage 5 does not say how to check a fact about GitHub (a merge order, a
   number, a date) before it goes into the plan.
5. The rules name `scripts/gh` but not what to do where the file is missing.
6. The boundary check does not say which `pr view` and `run list` entries a
   push of the orchestrator changes.
7. The implementer rules do not say what a central doc may name.

## Proposals

### P1. Read the code before the plan promises a fix

- File: `.agents/rules/sdlc-work-standard.md`, stage 5.
- New text: "Before a task names a function, a variable or a mechanism from a
  review finding, open the code and make sure that the task can do it with the
  files it names. Write the answer into the task. Make sure that no two tasks
  or facts contradict each other."
- Cause: 1 and 2.

### P2. Check each fact about GitHub with a command

- File: `.agents/rules/sdlc-work-standard.md`, stage 5.
- New text: "A fact about another pull request, a release or a date comes from
  a `scripts/gh` command. Write the command next to the fact in the plan."
- Cause: 4.

### P3. A child ends a run only after a commit or a blocker

- File: `.agents/rules/delegation-standard.md`, "Implementer rules".
- New text: add item "End a run only after a commit, or with a blocker that
  names the file and the line. Do not end a run with a plan or a status
  message."
- Cause: 3.

### P4. Say how to call `scripts/gh` in a worktree

- File: `AGENTS.md`, "GitHub CLI and commit rules".
- New text: "`scripts/gh` is not in git. In a worktree where it is missing, call
  the copy in the main checkout of the repository."
- Cause: 5.

### P5. Compare only what a child can change in the boundary check

- File: `.agents/rules/delegation-standard.md`, "Boundary check".
- New text: save and compare `state`, `isDraft`, `reviews` and only the
  comments of the owner's login, and only the `workflow_dispatch` runs. Save the
  state again after each push of the orchestrator.
- Cause: 6.

### P6. A central doc names files and tests, never plan tasks

- File: `.agents/rules/delegation-standard.md`, "Implementer rules".
- New text: add to the brief rules: "A central doc and a doc comment name files,
  functions and tests. They never name a plan task or a pull request."
- Cause: 7.
