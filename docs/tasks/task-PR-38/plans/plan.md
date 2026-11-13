# Plan: PR 38, SDLC Work Standard

Spec: `docs/tasks/task-PR-38/specs/design.md`, approved at gate 1. The rule
files in `.agents/rules/` are done. This plan applies the "Rule changes" table
and adds the task folder template.

Each task is one commit. Every task changes only Markdown, so its check is
`git diff --check origin/main HEAD` after the commit (no output, exit 0). Stage only the files the task names.
Tick the task's box in the same commit. Commit locally. Do not push.

## Tasks

- [x] **1. Task folder template.** Create `docs/tasks/_template/` with five
  files. Take each file's content from `.agents/rules/task-records.md`.
  - `specs/design.md`: the state block (`Stage: 1 start`, `Gate 1: pending`,
    `Gate 2: pending`) and the headings `## Why`, `## Scope`, `## Design`,
    `## Central docs that change`, `## Rule changes`, `## Risks`, each with
    one line that says what goes there.
  - `plans/plan.md`: a heading, one line that names the spec, and `## Tasks`
    with one example task line `- [ ] **1. <title>.** <files>. Check: <command>.`
  - `open-gaps.md`: a heading and the gap entry format from "Open gaps".
  - `reviews.md`: a heading and one example round with the fields `Head`,
    `Reviewer`, `SonarCloud`, `Checks`, `Gaps closed by recommendation`, and
    the finding table `| # | Finding | Action | Where |`.
  - `retro.md`: the headings `## What went wrong`, `## Why`, `## Proposals`,
    each with one line that says what goes there.
  - Commit: `docs(tasks): add the task folder template`.

- [x] **2. Pointers for each harness.** Edit `AGENTS.md` and `CLAUDE.md`.
  - `AGENTS.md`: add a section `## Work standard` right after the first
    paragraph: "Every change follows `.agents/rules/sdlc-work-standard.md`.
    Read it before any work. It points to `task-records.md`,
    `review-standard.md` and `delegation-standard.md` in the same folder."
  - `AGENTS.md` "Where things are": replace the bullets for `docs/plan.md`,
    `docs/plans/` and `docs/design/` with the new text from the spec's
    "Rule changes" table. Add a bullet: "`docs/tasks/task-PR-<number>/` holds
    the spec, plan, open gaps, review rounds and retro of each pull request.
    `docs/tasks/_template/` is the empty copy."
  - `AGENTS.md` "Issue tracker": the new text from the table.
  - `AGENTS.md` "Planning rules": replace the paragraph with the new text from
    the table.
  - `AGENTS.md` "Done" 3: append "Stage 6 of
    `.agents/rules/sdlc-work-standard.md` says when the orchestrator may close a
    gap by recommendation."
  - `CLAUDE.md`: add the line `@.agents/rules/sdlc-work-standard.md` after
    `@AGENTS.md`.
  - Commit: `docs(rules): point every harness to the SDLC Work Standard`.

- [x] **3. Contributing, issue tracker, testing and plans index.** Edit
  `CONTRIBUTING.md`, `docs/agents/issue-tracker.md`, `docs/testing.md` and
  `docs/plans/README.md`.
  - `CONTRIBUTING.md` "Branches and commits": replace "Do not add
    author-attribution trailers or generator notices to commits." with the new
    text from the table.
  - `CONTRIBUTING.md` "Pull requests": replace the `docs/plan.md` sentence with
    the new text from the table.
  - `CONTRIBUTING.md`, the line "Keep completed plans in
    `docs/archive/plans/`.": replace with the new text from the table.
  - `docs/agents/issue-tracker.md`: add the sentence from the table under
    "Conventions".
  - `docs/testing.md`: add a section `## Documentation checks` at the end:
    "A change that touches only Markdown runs `git diff --check`. It must print
    nothing."
  - `docs/plans/README.md`: add under the first paragraph: "New work uses a
    task folder under `docs/tasks/`, as `.agents/rules/sdlc-work-standard.md`
    says. The plans below started before PR 38."
  - Commit: `docs: apply the SDLC Work Standard to the contributor docs`.

- [x] **4. Live load check (orchestrator).** Start one Antigravity child and
  one Codex child. Ask each, without naming a file, which stage of the work
  standard comes after the spec review. Record each answer and whether the
  child read the rule in gap G2 and in `reviews.md`.
  - Commit: `docs(tasks): record the live rule load check`.

- [x] **5. CI gate (orchestrator).** Push, start the native gate, and run the
  CI gate check from `.agents/rules/review-standard.md`. Fix any failure as a
  new task below.

- [x] **6. Blank line in `docs/testing.md` (orchestrator).** Task 3 put the
  new heading right after the last paragraph. Add the blank line.

- [x] **7. Brief line for every provider (orchestrator, gap G13).** Edit
  `.agents/rules/delegation-standard.md` "T3 Code" 1 and the Antigravity row in
  the spec.

## Round 1

Code round 1 findings C1 to C4 and gap G14. Each task is one commit by the
implementer. Check after each commit: `git diff --check origin/main HEAD`
prints nothing once task 10 is done (before it, it reports only the C3 line).

- [x] **8. C1: stage 6 exit check.** `.agents/rules/sdlc-work-standard.md`,
  stage 6 "Exit": replace ``grep -c -- '- \[ \]' plans/plan.md`` with
  ``grep -c -E '^- \[ \]' docs/tasks/task-PR-<number>/plans/plan.md``.
  Commit: `docs(rules): count only real open tasks in the stage 6 exit check`.
- [x] **9. C2: Markdown check.** `docs/testing.md`, "Documentation checks":
  the sentence becomes "A change that touches only Markdown runs
  `git diff --check origin/main HEAD` after the commit. It must print nothing."
  Commit: `docs(testing): check Markdown changes against main after the commit`.
- [x] **10. C3: blank line at the end.** `.agents/rules/task-records.md`: the
  file ends with one line break, not two.
  Commit: `docs(rules): remove the blank line at the end of task-records`.
- [x] **11. C4: round heading in the template.** `docs/tasks/_template/reviews.md`:
  `## Round 1` becomes `## Round 1 (spec|code)`.
  Commit: `docs(tasks): give the template round heading its type`.
- [x] **12. G14: implementer model order.** `.agents/rules/delegation-standard.md`,
  "T3 Code" 1: replace "provider `codex`, model `gpt-6-luna`,
  `reasoningEffort: high`, `runtimeMode: full-access`, `isolation: shared`,
  unless the owner names another model." with: "`runtimeMode: full-access`,
  `isolation: shared`, and the first model in this order that is available,
  unless the owner names another model:" followed by a numbered list: 1.
  Provider `codex`, model `gpt-6-luna`, `reasoningEffort: high`. 2. Provider
  `antigravity`, model `gemini-3.8-flash`, `thinking_level: high`, when
  `gpt-6-luna` is unavailable or its child was cancelled as stalled. Keep the
  sentences after it.
  Commit: `docs(rules): add gemini-3.8-flash as the second implementer model`.
