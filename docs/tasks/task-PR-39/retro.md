# Retro: PR 39

## What went wrong

1. The `gpt-6-luna` implementer ended its run without a commit nine times over
   plan tasks 1 to 5. Nothing failed. The Codex transcripts show a final
   answer after 40 to 140 seconds, often while a cold `cargo test` still ran.
   The orchestrator sent many reminders before it read the transcripts and
   switched to `gemini-3.8-flash`. The owner asked "why did it stop?" and then
   "go through their jsonl find out why they stopped because this is very
   reucrring".
2. The shared disk filled to 100 percent twice. Linking and clippy failed. An
   implementer deleted `daemon/target` during a build and lost a verification
   run.
3. `Code Quality and Tests` hung for about an hour. A test helper
   (`BlockingSecretStore`) blocked forever in `spawn_blocking`. The test skips
   on the local machine, so only CI could show it. The orchestrator did not read
   CI after each push. The owner asked "hey why did you wait for so long?".
4. SonarCloud failed the quality gate on `efd389a`. Blocking `std::fs` calls ran
   in the async `main` of `nookctl`. Plan task 13 (`494b770`) fixed it. The
   same file shows one more SonarCloud issue on the head of round 3 that the
   gate accepts (`nookctl/src/main.rs:78`).
5. The first commit of plan task 3 held one test instead of five. The
   orchestrator found it by reading the diff and sent a follow-up.
6. The docs of task 10 held a false sentence ("The orchestrator checked these
   steps") and a macOS claim with no mark. The orchestrator found both in the
   diff and sent exact wording.
7. `open-gaps.md` ended with a blank line twice. `git diff --check` caught both
   before review, in round 5 of the spec and when G10 and G11 were added.
8. A test of the Daemon (`the_daemon_exits_on_sigterm_with_no_client`) failed
   once on an old race and passed on rerun.
9. The owner added three decisions after the spec review began (G7, G8, G9),
   and the spec had to change before gate 1.
10. `scripts/gh` is not in a worktree, because it is ignored by git. Every
    child and the orchestrator hit this at the start.
11. Luna had a usage limit for 5 hours. The owner told the orchestrator to use
    `claude-haiku-5-5` until it returned. The two fix tasks of round 2 ran on
    Haiku and passed on the first run.

## Why

1. The brief did not tell the child that any final message ends its task, and
   did not tell it how to wait for a slow command. The rules had no limit on
   the number of stops before a switch.
2. No rule said which build settings an implementer must use, or that nobody
   deletes a `target` folder. Three worktrees share one disk.
3. Stage 6 says to wait for the CI gate at the end. It does not say to read CI
   after every push, and it does not give a time limit for one job.
4. A SonarCloud finding is visible only after a push. No step asks the
   orchestrator to read the issue list after the first code push.
5. The boundary check proves that a child stayed in its task, not that it did
   the task. The diff read in stage 6 caught this, so the rule worked. No change
   is proposed.
6. The same reason as 5.
7. The check `git diff --check origin/main HEAD` exists and worked. No change is
   proposed.
8. The race is older than this task. The rule already allows one rerun of a
   flaky job. No change is proposed.
9. A mid-task decision from the owner is a normal input. The rule that a new
   requirement is a new gap handled it. No change is proposed.
10. The Start stage does not copy `scripts/gh` into the worktree.
11. A provider limit is outside the rules. The existing memory of the owner
    covered it. No change is proposed.

## Proposals

### P1. Put the stop rule and the wait method in every brief

- File: `.agents/rules/delegation-standard.md`, "Implementer rules".
- New text: "Put these lines in every brief: 'Any message you send as your final
  answer ends the task. Send progress as commentary and keep calling tools.
  Run any cargo command with its output in a file, and wait for it with
  `timeout 900 sh -c "while pgrep -x cargo >/dev/null; do sleep 5; done"`. End
  only after the commits exist, or with a blocker that names the file and
  line.' After two runs that end with no commit, change the implementer or do
  the task yourself."
- Cause: 1.

### P2. Name the build settings and protect `target`

- File: `.agents/rules/delegation-standard.md`, "Implementer rules".
- New text: "Build with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
  CARGO_PROFILE_TEST_DEBUG=0 TMPDIR=/tmp`. Never delete a `target` folder and
  never run `cargo clean`. Before stage 6, make sure that `df` shows at least
  20 GB free."
- Cause: 2.

### P3. Read CI after every push

- File: `.agents/rules/sdlc-work-standard.md`, stage 6.
- New text: "After every push, read the checks of the new head within 15
  minutes. Cancel a `Code Quality and Tests` job that runs longer than 15
  minutes, and find its cause in the log before you push again. A test that
  skips on the local machine is proved only by CI, so read its CI result."
- Cause: 3.

### P4. Read SonarCloud after the first code push

- File: `.agents/rules/sdlc-work-standard.md`, stage 6.
- New text: "After the first push that changes code, read the SonarCloud
  issues of the pull request, and fix each one before the next plan task."
- Cause: 4.

### P5. Copy `scripts/gh` into the worktree at the start

- File: `.agents/rules/sdlc-work-standard.md`, stage 1.
- New text: "In a worktree, copy `scripts/gh` from the main checkout
  (`cp -p <main checkout>/scripts/gh scripts/gh`). The file is ignored by git."
- Cause: 10.
