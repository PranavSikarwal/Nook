# Retro

## What went wrong

1. **A test built its state by hand and hid a P1 (round 15, C1).** The chat grants of the Notes tools were stored under `grant_key(tool_name)`, which has no `nook:` prefix. `remove_tools` got the prefixed ids, so a folder change never dropped a chat grant. The first tests wrote the grant by hand under the same wrong key, so they passed. The reviewer found it. Fix: commit `bc6b953` (R1) with a real `allow_for_chat` flow.
2. **Tests passed without running (rounds 17 to 19, C14 and C15).** Cargo on the new runner image put the test binary in `target/debug/build/nook-core/<hash>/out/`. `example_binary()` then missed the fake MCP server, `McpProvider::start` returned `None`, and 22 of 24 MCP tests returned early and passed in 0.19 s. Two tests that unwrap the result failed. The same hole made `feat/google-docs-mcp` fail 13 tests. The local VM cannot run bubblewrap, so the tests also skipped here and for the reviewer. This cost two rounds (18 and 19, runs 44885309975 and 44889895863) and gap G8.
3. **Two Windows build breaks (round 16 C9, round 18 C13).** Commit `bc6b953` made `mod notes` compile on Windows, and the test module was not gated for unix. The R13 split then added an `impl` block without `#[cfg(unix)]`. Linux builds, clippy and tests cannot see either one. Each cost a full round. The first luna fix of R8 also gated the wrong item (`with_timeout`), and I made the real fix in `1268f64`.
4. **Children stopped without a commit, and a cancelled child kept writing.** Luna stalled or ran out of time on tasks 5 to 13, R8 to R11 and again in round 16. One child left R9 uncommitted and unformatted, another left a scratch edit in `tool_approval.rs`, and a late commit for task 9 raced a gemini child. A cancelled luna child kept editing `notes.rs` while a gemini child worked. I verified, formatted and committed the leftover work myself several times. Antigravity was also unavailable in round 16 (T3 said it predates the CLI).
5. **The disk filled up several times.** A luna build hit 0 bytes free. I deleted `~/.cache/sccache` and `daemon/target`, and added the cargo environment line to every brief.
6. **CI failed on a check that the plan did not list.** `ruff format --check` failed on `panel/e2e/test_fake_worker.py` (fixed in `1b53d66`), because the task checks in `docs/testing.md` did not include it.
7. **The wait for the reviewer died with the T3 server.** A restart killed the background poll, and the owner asked: "but you stopped how are we waiting on poll?". A long blocking loop also died with exit 137.
8. **A parallel task left dangling paths.** `docs/plans/` and `CHECKLIST.md` still name `desktop-app-tools`, and `design.md` and `plan.md` still name `0008-panel-runs`. Those files belong to the housekeeping task and to the task records.
9. **The review took 20 rounds** (14 on the spec, 6 on the code), and `gh pr edit` fails on a Projects-classic error, so each PR text change went through `gh api`.

## Why

1. The rules say to test through the public interface, but no rule says that a test creates its state through the flow that production uses. A hand-written key matches the code under test by construction.
2. No rule says what a skipped test means in CI. `NOOK_REQUIRE_SANDBOX` covered only the sandbox check, not a provider that starts nothing. A helper that finds a build output from the test executable depends on the Cargo layout, and nothing checked that.
3. The stage 6 checks are Linux checks. No step covers a change to `cfg` gates or to the OS reach of a module, and the Windows job runs only after a push.
4. The delegation rule says to demand a commit, but it does not say what to do when a child ends without one, it allows several tasks in one brief, and it does not say that a cancelled child can still write.
5. The delegation rule does not state the disk limits of the build machine.
6. Stage 6 says to run the checks from `docs/testing.md`, and that file does not list every command of `ci.yml`.
7. The delegation rule says to use a shell loop only for external state and as one command, but not how to resume it after a restart.
8. Parallel tasks share paths, and the standard has no rule for who updates a path that one task renames.
9. Many P3 findings per round came from a few causes (1 to 3), so one cause reopened a round several times.

## Proposals

### P1. Build test state through the production flow

File: `.agents/rules/delegation-standard.md`, section "Implementer rules". New item 9: "Create the state that a test needs through the flow that production uses (a real approval, a real grant, a real request). Do not write a key, an id or a stored record by hand, because a hand-written value copies the mistake of the code under test."

### P2. A skipped test must fail where the tool is required

File: `.agents/rules/delegation-standard.md`, section "Implementer rules". New item 10: "A test that skips when a tool is missing (sandbox, database, program) must fail at every skip site when the matching `NOOK_REQUIRE_*` variable is set, including a start that returns `None`. A helper that finds a build output must fail with the searched path when it finds nothing."

### P3. Check the other operating systems when a module's reach changes

File: `.agents/rules/sdlc-work-standard.md`, stage 6, after "Run the boundary check". New step: "When a task adds or changes a `cfg` attribute, or makes a module compile on another OS, switch each `cfg(unix)` of that module to `cfg(any())` in a scratch copy and run `cargo check -p <crate> --lib`. Expect 0 errors. Restore the copy, and treat the matching CI job (`Windows named-pipe IPC`) as the proof before the next round."

### P4. One task per child, and a cleanup step after a child ends

File: `.agents/rules/delegation-standard.md`, section "T3 Code", step 4 "Take over". New text: "Give a child one plan task, or two small ones, and a 40 minute time box. When a child ends without a commit, run `git status` and `git diff`, run the checks and `cargo fmt`, and commit or restore the files before you start another child. After `task_cancel`, read `git status` again after one minute, because a cancelled child may still write."

### P5. Put the disk limits in the brief

File: `.agents/rules/delegation-standard.md`, section "T3 Code", step 1 "Delegate". New text: "Every brief for a Rust task includes: `export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 TMPDIR=/tmp`, 'run cargo in the background with a timeout and a log', and 'run `rm -rf daemon/target` as the last step'."

### P6. Run every CI command that the changed component has

File: `.agents/rules/sdlc-work-standard.md`, stage 6, rule "A task passes when its checks pass with zero warnings". New text: "The checks of a component include each command of `.github/workflows/ci.yml` for that component (for example `ruff format --check .` for Python files). Add a missing command to `docs/testing.md` in the same task."

### P7. Make a wait resumable

File: `.agents/rules/delegation-standard.md`, section "T3 Code", step 2 "Wait". New text: "Write the poll command to a script file in the scratchpad before you run it. When the harness reports that a background task was cancelled by a restart, read the real state first (the review, the checks), then run the script again. Tell the owner in one line when you start and when you restart a wait."

### P8. Name the owner of a renamed path

File: `.agents/rules/task-records.md`, section "Open gaps". New text: "When a task renames or removes a path that files outside its task folder name, list those files in the spec. The orchestrator of the task that owns those files updates them, and the pull request text names them. The task folder files keep the old name, because they are records."
