# Nook

Nook is a cross-platform desktop overlay app. It has a Tauri Panel, a Rust Daemon, and a Python Deep Agents Worker. The Worker talks to a self-hosted OpenAI-compatible model.

## Work standard

Every change follows `.agents/rules/sdlc-work-standard.md`. Read it before any work. It points to `task-records.md`, `review-standard.md` and `delegation-standard.md` in the same folder.

## Where things are

- `docs/architecture.md`, `docs/contracts.md`, `docs/tools-and-authorization.md`, `docs/mcp-servers.md`, `docs/release.md`, and `docs/testing.md` describe current behavior.
- `docs/plan.md` and `CHECKLIST.md` track the work that started before PR 38. A new task is tracked by its task folder and its pull request.
- Each task has `docs/tasks/task-PR-<number>/`. `docs/plans/` and `docs/design/` keep only the plans and designs that started before PR 38.
- `docs/tasks/task-PR-<number>/` holds the spec, plan, open gaps, review rounds and retro of each pull request. `docs/tasks/_template/` is the empty copy.
- `docs/adr/` holds decisions that are hard to reverse. `docs/archive/` holds retired specifications and historical records.
- `model-check/` holds the script that tests the model endpoint with Deep Agents.

## Agent skills

### Issue tracker

Issues live under `.scratch/`. The spec of a task lives in its task folder. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. See `docs/agents/domain.md`.

## Planning rules

Follow `.agents/rules/sdlc-work-standard.md`. The spec and the plan live in the task folder.

## Engineering rules

These rules apply to every code change, by a person or by any agent.

### Design

1. Give each module and each function one responsibility. Split a function when its name needs "and".
2. Keep modules deep: a small public interface that hides the detail. Call another module only through that interface.
3. Keep the dependency direction: Panel calls Daemon, Daemon calls Worker. The three processes share only the messages in `contracts/`. Change the schema, its example in `contracts/examples/`, and the contract test in the same commit as the code on both sides.
4. Keep one source of truth for each rule, constant, and user-visible message. Search for an existing helper first, and reuse it.
5. Fail closed. An authorization, path, or sandbox check that cannot decide answers "deny". Write durable state (a file, the database) first, and change memory only after the write works.
6. Read shared state and write it back under one lock, so two requests cannot lose an update.
7. Return errors to the caller. Rust code outside tests returns `Result` and uses a fixed message for the user. The Panel shows the error or logs it.
8. Build only what the current plan step needs.

### Patterns in this code

Extend the pattern that already owns the job. Add a new pattern only when two real cases need it.

| Job | Pattern | Where |
| --- | --- | --- |
| Add or change a tool | `Provider` trait behind `ProviderRegistry` (strategy and registry) | `daemon/crates/nook-core/src/providers/` |
| Swap tools while calls run | `ProviderHandle` holds an immutable snapshot and replaces it whole | `providers/mod.rs` |
| Talk to the Worker | `WorkerSupervisor` owns the process and its channels | `supervisor.rs` |
| Panel to Daemon request | One variant of `ClientMessage`, one answer variant of `DaemonMessage` (command messages) | `protocol.rs` |
| Webview to Rust | One Tauri command that calls `ask_daemon` and returns a typed value | `panel/src-tauri/src/lib.rs` |
| React to Tauri | `panel/src/lib/daemon.ts` is the only file that calls `invoke` or `listen` (facade). It also holds the browser mock | `panel/src/lib/daemon.ts` |
| Authorize a tool call | Cedar decides, the Daemon stores grants and approvals | `policy.rs`, `tool_approval.rs` |

### Comments

Write the minimum. Prefer a clear name, a type, or a smaller function.

1. Write a comment for the why: a hidden reason, an invariant, or a trap. Start a trap with `Warning:`.
2. Write a comment for the what only when the name and the types cannot say it.
3. Give each public Rust item and each exported TypeScript function a doc comment of one or two lines when its name does not state the full contract.
4. Keep each comment to one or two lines, and keep it true when the code changes.

### Tests

1. Write the failing test first. Run it and see it fail for the expected reason. Then write the code.
2. Test through the public interface with the real collaborators that the suite already uses: the real policy files, the real socket, and the fake Worker.
3. Test one behavior in each test. Name the test as a sentence, such as `a_removed_folder_is_refused_for_new_calls`.

### Done

1. Every command in `docs/testing.md` for each changed component passes with zero warnings.
2. Each plan step is one commit, with its checklist box ticked in the same commit.
3. A change stays inside the plan. If the plan is wrong or a change outside it is needed, stop and report. Stage 6 of `.agents/rules/sdlc-work-standard.md` says when the orchestrator may close a gap by recommendation. A small fix is the exception: `.agents/rules/delegation-standard.md` "T3 Code" 5 says when.
4. A pull request that completes a release bumps the application version and moves "Unreleased" in `CHANGELOG.md` under the new version before it merges. After the merge, run the Package and Release workflow by hand on `main` at the merge commit. Create the tag only when the package run passes on macOS, Ubuntu and Windows.

## GitHub CLI and commit rules

This repository uses the personal GitHub profile `PranavSikarwal`.

1. Never run `gh auth switch`. It modifies global state in `~/.config/gh/hosts.yml` and disrupts other workspaces.
2. Run all GitHub CLI commands through `scripts/gh`:
   ```sh
   scripts/gh <subcommand>
   ```
   Do not run `gh` directly and do not run `gh auth token`. The wrapper manages authentication automatically.
3. Commits in this repository must use local repository identity rather than the global work identity.
4. Git pushes authenticate using the repository-local credential helper that reads `gh auth token --user PranavSikarwal`.
