# Open gaps: PR #38

Each entry records one question and its decision. The format is in
`.agents/rules/sdlc-work-standard.md`, "Open gaps".

## G1. Name and place of the task folder

- Stage: 2 spec
- Question: where do task folders live, and what is their name?
- Options: (a) `docs/tasks/task-PR-<number>/`. (b) `docs/tasks/<branch-slug>/`.
  (c) `docs/plans/task-PR-<number>/`.
- Recommendation: (a), as the owner asked. The number links the folder to its
  pull request. Stage 1 opens the pull request with an empty commit first, so
  the number exists before the folder.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G2. Rule loading for Antigravity

- Stage: 2 spec
- Question: does Antigravity load `.agents/rules/sdlc-work-standard.md`?
- Options: (a) rely on `.agents/rules/` with `trigger: always_on`, the current
  path in the Antigravity docs. (b) also add a copy in the legacy
  `.agent/rules/`.
- Recommendation: (a). Check it in one live Antigravity run during the
  implementation, and record the result here.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G3. Small changes

- Stage: 2 spec
- Question: may a small change skip the spec and gate 1?
- Options: (a) every task runs every stage. (b) the rule's "Small change"
  section: no behavior change in `daemon/`, `worker/`, `panel/`, `contracts/`
  or `policy/`, the owner calls it small, stages 1 and 5 to 9 still run.
- Recommendation: (b).
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G4. Existing plans and designs

- Stage: 2 spec
- Question: what happens to `docs/plans/` and `docs/design/`?
- Options: (a) leave them in place and finish them there. New work uses task
  folders. (b) move all of them into task folders now.
- Recommendation: (a). `CHECKLIST.md` and `AGENTS.md` link to them, and a move
  breaks the links and the history of ticked boxes.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G5. Global rules versus repository rules

- Stage: 2 spec
- Question: what stays in `~/.claude/CLAUDE.md`?
- Options: (a) the global file keeps the rules for all projects (writing style,
  no AI attribution, comments, RTK, T3 waiting). The repository rule repeats
  what Codex and Antigravity need, and points to `CONTRIBUTING.md` and
  `AGENTS.md` for the commit rules. (b) the T3 section leaves the global file.
- Recommendation: (a). Other projects still need the T3 rules. Add the ban on
  AI attribution in pull request text and reviews to `CONTRIBUTING.md`, because
  today it covers commits only.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G6. Enforce the task folder in CI

- Stage: 2 spec
- Question: does CI check that a pull request has its task folder?
- Options: (a) a CI step that fails without the folder. (b) no CI check. The
  orchestrator and the reviewer check the folder.
- Recommendation: (b), changed after review round 1. `Code Quality and Tests` is
  not a required check, so a failure does not block a merge. Look at it again
  after two tasks use the folder.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G7. The reviewer thread id

- Stage: 2 spec
- Question: does the rule name the T3 thread id of "Review Pull Requests"?
- Options: (a) name the thread only, and find it with `t3_thread_search`. (b)
  write the id into the rule.
- Recommendation: (a). The id belongs to one machine. The rule asks the owner
  when the search finds no thread or several.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G8. Release in the same pull request as a fix

- Stage: 3 spec review (round 1, finding 7)
- Question: may a release share a pull request with a fix or a feature?
- Options: (a) yes, with the release task as the last plan task. PR 37 did
  this. PR 35 was a release-only pull request. (b) a release is always its own
  pull request.
- Recommendation: (a) when the owner asks for the release together with the
  work, else (b). A round fix after the release task changes no version file,
  and its changelog line goes under the new version (rule stage 5).
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G9. Who may close a gap during implementation

- Stage: 3 spec review (round 1, finding 2)
- Question: may an implementer close a gap by recommendation and continue?
- Options: (a) the implementer stops and reports. The orchestrator closes the
  gap by recommendation only when it touches only the files the plan task
  names and changes no contract, policy, public interface, authorization or
  sandbox behavior, or dependency. Any other gap goes to the owner. (b) the implementer closes any
  gap and continues.
- Recommendation: (a). The owner still reviews every closed gap at gate 2, as
  asked, and a gap that touches safety waits for the owner. Each round in
  `reviews.md` also lists the gaps closed since the last round, so the owner
  can read them early.
- Decision: the recommendation.
- Decided by: owner ("approve", at gate 1, for every open gap with its recommendation)
- Date: 2026-11-13
- Status: decided by owner

## G10. Which reviewer reviews what

- Stage: 3 spec review (owner input in round 2)
- Question: which reviewer runs a round: the T3 reviewer or the GitHub Actions
  reviewer in `reviewers.yml`?
- Options: (a) the GitHub Actions reviewer for small changes, and the T3
  reviewer required for spec reviews, new features, design changes, bug fixes
  and core changes, with fallbacks. (b) the GitHub Actions reviewer first for
  everything, and the T3 reviewer only as a fallback.
- Recommendation: (a).
- Decision: (a). Core fixes, new features, design changes and bug fixes require
  the T3 reviewer or another dedicated reviewer agent at least. The review
  process lives in its own rule file, `.agents/rules/review-standard.md`.
- Decided by: owner ("yes i agree, but make it clear the core fixes new feature
  and design changes or the bug fixes requires the t3 reviewer or special
  reviewer atleast")
- Date: 2026-11-13
- Status: decided by owner

## G11. Harden the GitHub Actions reviewer

- Stage: 3 spec review (round 3, findings 29, 32, 33, 34)
- Question: when does `reviewers.yml` and `pr_reviewer.py` get fixed?
- Options: (a) a follow-up task after this pull request: `persist-credentials:
  false`, run the script from a base-ref checkout, fail the trust step when the
  base file is missing, exit non-zero without secrets, a `concurrency` block,
  pin the review to `git rev-parse HEAD`, always post a review with the verdict
  line first, pass `base_ref` through `env:`. (b) fix it in this pull request.
- Recommendation: (a). It is workflow code, outside this spec. Until then the
  review rule forbids the Actions reviewer on fork pull requests and reads its
  plain comments.
- Decision: (a). It joins the other follow-up fixes.
  It is the first task after PR 38 merges. The retro proposals of PR 38 come
  after it. Until it merges, the review rule sends small changes to the T3
  reviewer too. The G11 task removes that clause from
  `.agents/rules/review-standard.md`.
- Decided by: owner ("works let's add it to other follow up fixes")
- Date: 2026-11-13
- Status: deferred

## G12. Round 3 ended with open findings

- Stage: 3 spec review (round 3)
- Question: the rule allows 3 spec rounds. Round 3 ended with 6 open P2
  findings, now fixed but not reviewed. What next?
- Options: (a) a round 4 to confirm the fixes, then gate 1. (b) gate 1 now, and
  the code rounds in stage 7 check the fixes.
- Recommendation: (a). Four P2 fixes change commands that every later task
  runs (the CI gate check, resume, the reviewer reads), so a reviewer must see
  them once.
- Decision: (a), and the rule changes: rounds have no limit. Each round fixes
  every finding, and all gates run again, until every finding is fixed and the
  reviewer approves.
- Decided by: owner ("yes add it to rule that it asks for all review fixes until
  all of the are fixed! and the PR is approved we keep review fixes and follow
  up with all gates rerun again and again in same step")
- Date: 2026-11-13
- Status: decided by owner
