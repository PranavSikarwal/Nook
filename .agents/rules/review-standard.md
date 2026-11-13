---
trigger: model_decision
description: Review Standard. Read it before a spec review or a code review round on a pull request.
---

# Review Standard

A **round** is one review cycle on one head commit. The orchestrator pushes,
collects three **inputs** for that head (the reviewer's review, the SonarCloud
result and the CI checks), and answers all of them in one fix pass. The next
push starts the next round. `.agents/rules/sdlc-work-standard.md` says when a
task runs rounds: stage 3 for the spec and stage 7 for the code.

## Pick the reviewer

Pick the reviewer from the task (`specs/design.md`, or the spec paragraph in
`plans/plan.md` for a small change), and write the choice into the round in
`reviews.md`.

| Task | Reviewer |
| --- | --- |
| Spec review, new feature, design change, bug fix, or any change in `contracts/`, `policy/`, authorization, the sandbox, or the Daemon or Worker core | The T3 reviewer. It is required. |
| Small change (see "Small change" in the SDLC Work Standard) | The GitHub Actions reviewer. |

The GitHub Actions reviewer may also run in addition on any pull request. Its
findings join the same round. Use it only on a pull request from a branch of
this repository, never on a pull request from a fork: the job runs with the
reviewer secrets.

Fallback, when a reviewer fails (see "When an input fails to arrive"):

1. The GitHub Actions reviewer fails: use the T3 reviewer.
2. The T3 reviewer fails: start a dedicated reviewer agent (the `pr-reviewer`
   agent type) with the same request. A task that requires the T3 reviewer
   never falls back to the GitHub Actions reviewer alone.

## Verdict line

Every review that counts starts with this line:

```
Verdict: <APPROVE|COMMENT|REQUEST_CHANGES> (P0: <n>, P1: <n>, P2: <n>, P3: <n>)
```

The counts are open findings. Every reviewer posts as a `COMMENT` review,
because the owner cannot approve their own pull request, so the verdict line is
the only verdict. Put the line format into every review request.

## Ask the T3 reviewer

1. Find the thread "Review Pull Requests" with `t3_thread_search`. If the search
   finds no thread or several, ask the owner.
2. Send the request with `t3_thread_send`: the pull request URL, the head SHA,
   "spec review" or "code review", round 1 or "follow-up of round N", and the
   verdict line format.
3. Wait with `t3_thread_wait`. The reviewer's run ends before its subagent posts
   the review, so then read the reviews on the pull request (see "Read the
   inputs").

## Ask the GitHub Actions reviewer

`.github/workflows/reviewers.yml` runs `pr_reviewer.py` on a pull request
comment from the owner's account. Post the comment with `scripts/gh`:

1. Write the comment into a file. Round 1 starts with `/code-review-expert high`.
   Each later round starts with `/feedback-on-resolution high`, which checks
   whether the fixes resolved the earlier findings. The second line is the
   instruction "Start the review with the line:" and the verdict line format.
2. Post it: `scripts/gh pr comment <number> --body-file <file>`.
3. `/code-review` runs the lighter review. Use it only when the owner asks.

The job posts a "Review in progress" comment as `github-actions[bot]`. When
the model returns inline comments, the job posts a `COMMENT` review. When it
returns none, it edits the "Review in progress" comment or posts a plain
comment instead, with no `commit_id`. Read the result as "Read the inputs" 1
says. Push nothing while the job runs, so that its result belongs to the head.
Its run log is the "Manual PR reviewers" workflow run.

## Run a round

1. **Open.** Add `## Round N (spec|code)` to `reviews.md` with the reviewer you
   picked. Commit and push. The commit that adds the heading is the round's head.
2. **Start the inputs** on that head:
   1. Ask the reviewer.
   2. Start the native gate:
      `scripts/gh workflow run native-e2e-gate.yml --ref <branch> -f pr_number=<number>`.
      It runs only by hand. CI and SonarCloud start by themselves on a push.
3. **Wait** until all inputs exist for the head (see "Read the inputs"). Poll with
   one shell command of at most 30 minutes, for example
   `timeout 1800 bash -c 'until <test>; do sleep 120; done'`. Start no fix while
   an input is missing.
4. **Record.** Write the head SHA and every finding into the round: each reviewer
   finding, each SonarCloud issue, hotspot or gate condition, and each failed
   job. Give each one action: `fix`, `defer` (a gap with status `deferred`), or
   `reject` (with the reason). A failed check and a failed SonarCloud gate are
   always `fix`, because every CI check is a gate. List the gaps closed by recommendation since the
   last round, so the owner can read them early.
5. **Fix.** In a spec round, edit the spec, the rules and `open-gaps.md` in one
   commit for the pass. In a code round, append the `fix` items to
   `plans/plan.md` under `## Round N`, one task per commit, and implement them as
   stage 6 of the SDLC Work Standard says.
6. **Push.** The next round starts on the new head.

Run at most 3 rounds in a stage. If round 3 ends with an open P0, P1 or P2
finding, stop and ask the owner. A round opened because `main` moved counts
toward the limit.

## Read the inputs

1. **Reviews.** The T3 reviewer posts as the owner (`PranavSikarwal`). The
   GitHub Actions reviewer posts as `github-actions[bot]`. Read the reviews:
   `scripts/gh api --paginate repos/PranavSikarwal/Nook/pulls/<number>/reviews --jq '.[]|[.user.login,.commit_id,(.body|split("\n")[0])]|@tsv'`.
   For the T3 reviewer, the review that counts is the newest one from the owner
   whose `commit_id` is the head SHA and whose first line is a verdict line. For
   the GitHub Actions reviewer, also read the comments
   (`scripts/gh api --paginate repos/PranavSikarwal/Nook/issues/<number>/comments`)
   and take the newest review or comment from `github-actions[bot]` created after
   your trigger comment. If it has no verdict line, count its P0 to P3 findings
   yourself and write the counts into the round as "verdict derived". Ignore
   every other review and comment.
2. **Checks.** The **CI gate check** passes only when all four checks report
   `pass` on the head:
   `[ "$(scripts/gh pr checks <number> | cut -f1,2 | grep -c -E '^(Code Quality and Tests|Windows named-pipe IPC|SonarCloud Code Analysis|Native Tauri E2E Gate)\tpass$')" = 4 ]`.
   The exit code of `scripts/gh pr checks` alone is not enough: it is 0 also
   when a check is missing. Use the exit code only to see that a check is
   pending (8) or failed (1).
3. **SonarCloud:**
   `scripts/gh api repos/PranavSikarwal/Nook/commits/<sha>/check-runs --jq '.check_runs[]|select(.name=="SonarCloud Code Analysis")|.output.title,.output.summary'`.
   It gives the gate result and the counts of issues, hotspots, coverage and
   duplication on new code. The public SonarCloud API is not a source: it
   answers `total: 0` even for a project that does not exist.

## Round exit check

A round passes when all of these hold on the current head:

1. The review that counts says `Verdict: APPROVE`, or counts `P0: 0, P1: 0, P2: 0`.
2. Every P2 is fixed, or deferred or rejected with a reason in `reviews.md`.
3. Every deferred finding is a gap in `open-gaps.md`.
4. The CI gate check passes.

## When an input fails to arrive

1. **No review after 30 minutes.** Send the reviewer one reminder (or post the
   comment again for the GitHub Actions reviewer). If 15 more minutes pass with
   no review, use the fallback reviewer and tell the owner.
2. **A review on an older head.** Ignore it, and ask again for the current head.
3. **A failed job.** Rerun it once: `scripts/gh run rerun <run-id> --failed`.
   Write the rerun into the round as `flake` with the job name and run id. A
   second failure is a finding.
4. **A check pending or missing after 60 minutes.** Start the native gate again,
   or rerun the CI run. SonarCloud has no start command: push again only if it
   is still missing after the rerun, then tell the owner.
5. **A SonarCloud result that the check-run text does not explain** (an issue
   with no message, a hotspot, a coverage or duplication failure): write the
   check-run text into the round as a finding and ask the owner.
6. **`main` moved and the branch conflicts.** The orchestrator merges `main` into
   the branch, resolves the conflict, runs the checks, pushes, and opens a new
   round.

## What the reviewer may do

The reviewer reads the pull request, its diff, the repository and the check
results. It posts one review per round with the verdict line, inline comments,
and resolves its own threads that a fix closed. It changes no file, commit,
branch, label or check.
