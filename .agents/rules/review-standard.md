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

Pick the reviewer from the task in `specs/design.md`, and write the choice into
the round in `reviews.md`.

| Task | Reviewer |
| --- | --- |
| Spec review, new feature, design change, bug fix, or any change in `contracts/`, `policy/`, authorization, the sandbox, or the Daemon or Worker core | The T3 reviewer. It is required. |
| Small change (see "Small change" in the SDLC Work Standard) | The GitHub Actions reviewer. |

The GitHub Actions reviewer may also run in addition on any pull request. Its
findings join the same round.

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

1. Round 1: `scripts/gh pr comment <number> --body "/code-review-expert high"`,
   then a new line with "Start the review with the line: Verdict: ..." and the
   format above.
2. Each later round: `/feedback-on-resolution high`, with the same verdict line
   instruction. It checks whether the fixes resolved the earlier findings.
3. `/code-review` runs the lighter review. Use it only when the owner asks.

The job posts a "Review in progress" comment, then a `COMMENT` review pinned to
the head. Its run log is the "Manual PR reviewers" workflow run.

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
   `reject` (with the reason). List the gaps closed by recommendation since the
   last round, so the owner can read them early.
5. **Fix.** Append the `fix` items to `plans/plan.md` under `## Round N`, one task
   per commit, and implement them as stage 6 of the SDLC Work Standard says.
6. **Push.** The next round starts on the new head.

Run at most 3 rounds in a stage. If round 3 ends with an open P0, P1 or P2
finding, stop and ask the owner. A round opened because `main` moved counts
toward the limit.

## Read the inputs

1. **Reviews:**
   `scripts/gh api repos/PranavSikarwal/Nook/pulls/<number>/reviews --jq '.[]|[.user.login,.commit_id,(.body|split("\n")[0])]|@tsv'`.
   The review that counts is the newest one whose `commit_id` is the head SHA
   and whose first line is a verdict line. Ignore every other review.
2. **Checks:** `scripts/gh pr checks <number>`. Exit code 0 means every check
   passed, 8 means a check is pending, 1 means a check failed. The four checks
   are `Code Quality and Tests`, `Windows named-pipe IPC`,
   `SonarCloud Code Analysis` and `Native Tauri E2E Gate`.
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
4. `scripts/gh pr checks <number>` exits 0.

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
