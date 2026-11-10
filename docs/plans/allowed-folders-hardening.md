# Allowed folders hardening: implementation plan

**Goal:** Close the gaps that a review found in the allowed folders view, and prove each acceptance check of `docs/plans/allowed-folders-view.md` on Linux.

**Architecture:** No new module, message, or dependency. Three small Daemon fixes in `folders.rs` and `tool_approval.rs`, one cleanup in `FoldersTab.tsx`, two more native tests, and the documents.

**Tech stack:** Rust (tokio, `nook-core`), React 19 with TypeScript, WebdriverIO native tests.

**Spec:** `docs/plans/allowed-folders-view.md`. Read it first. Read the "Engineering rules" in `AGENTS.md` second.

## Global constraints

- Work on the branch `feat/allowed-folders-view`. Commit locally. Do not push.
- Do the tasks in order. One task is one commit. Tick the boxes of a task in this file in the same commit.
- Change only the files that a task lists. If a step cannot be done as written, stop and report. Do not work around it.
- Add no dependency. Change no file in `contracts/` or `policy/`.
- Commit messages use `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Prefix shell commands with `rtk`, as `docs/testing.md` does.
- Load the environment before a Daemon test or a native test: `set -a && . ./.env && set +a` from the repository root.

## What the review found

| # | Finding | Where | Task |
| --- | --- | --- | --- |
| 1 | `add`, `set_access`, and `remove` read the list under a read lock, release it, and write under a second lock. Two requests can lose one change. | `folders.rs` | 1 |
| 2 | Each function logs "Added", "Changed", or "Removed" before the save. A failed save leaves a false log line. | `folders.rs` | 1 |
| 3 | `remove` drops chat grants only when the folder still exists on disk, because it resolves the path first. A folder that the user deleted keeps its grants. | `folders.rs` | 2 |
| 4 | A waiting approval that a folder change makes invalid is always denied with "Path is outside the allowed folders". After a change to read only, the true reason is "The folder is read-only". | `tool_approval.rs` | 3 |
| 5 | `handleAdd` repeats the busy and error handling of `change`. | `FoldersTab.tsx` | 4 |
| 6 | No test shows the error banner or the note for a folder that left the disk. | `folders.spec.mjs` | 4 |
| 7 | The acceptance boxes in the spec are not ticked, and the changelog has no entry. | documents | 5 |
| 9 | `terminal-tool.spec.mjs` fails on Linux with "A command wrote outside the allowed folder". The Linux sandbox mounts a private `tmpfs` on `/tmp` (`sandbox.rs`), so `touch /tmp/...` exits 0 inside the sandbox and leaves the host untouched. The host stays safe. The spec's exit-code check holds only on macOS, where Seatbelt denies the write. Found by the native run. | `terminal-tool.spec.mjs` | 7 |
| 10 | SonarCloud lists 5 new issues on the PR (check-run annotations): four `Replace this closure with a reference to the into_inner method` (`providers/mod.rs:289`, `providers/mod.rs:297`, `supervisor.rs:216`, `supervisor.rs:225`) and one `Use <details>, <fieldset>, <optgroup>, or <address> instead of the "group" role` (`FoldersTab.tsx:99`). | four closures, one element | 8 |
| 11 | GitHub review (`github-actions`, REQUEST_CHANGES, 6 findings). Checked against the code: finding 1 (`config.toml` truncated in place) is wrong, because `write_private` already writes `config.toml.tmp` with mode `0600`, syncs, and renames it. Finding 2 is half right: the plan allows folders outside home, so only the error text `HOME_OR_ABOVE` is wrong. Findings 3, 5, and 6 are right. Finding 4 is real but has no effect: `prepare` in `filesystem.rs` checks the path against the registry that the call took, and the file tools have no persistent grants, so the race can leave only a chat grant for a path outside every folder. No change for finding 4. | `folders.rs`, `policy.rs` callers, `config.rs` test | 9 to 12 |
| 8 | `socket_ownership_tests` fail on Linux at random: `Server::bind` sets the process-wide umask to `0o177` for a moment, and a parallel test that creates its folder then gets a folder with mode `0600` and no search bit. Found by the baseline run. | `socket_ownership_tests.rs` | 6 |

## Review focus

The verifier checks these cases after the work. Each one has a test in the task named.

1. A folder is deleted from disk and then removed from the list. Its chat grants must be gone. Task 2.
2. A write waits for approval, and the folder changes to read only. The denial must say "The folder is read-only". Task 3.
3. A write waits for approval, and the last folder is removed, so the tool itself is gone. The denial must say "Path is outside the allowed folders". Task 3.
4. The user cancels the folder picker. The list and the error banner must stay as they were. Task 4.
5. The user adds a folder that is already listed. The banner must show the Daemon's reason, and the list must not change. Task 4.

---

### Task 1: Change the folder list under one lock

**Files:**
- Modify: `daemon/crates/nook-core/src/folders.rs`

**Interfaces:**
- Produces: the private function below. `add`, `set_access`, and `remove` keep their public signatures.

```rust
/// Runs `edit` on a copy of the list while the config write lock is held, then
/// saves the copy, rebuilds the tools, and tells the Worker. `edit` returns the
/// folder that it removed, if any.
async fn apply(
    state: &Arc<AppState>,
    edit: impl FnOnce(&mut Vec<RootConfig>) -> Result<Option<PathBuf>, &'static str>,
) -> Result<Vec<RootInfo>, String>
```

A test that fails first is not possible here. The lost update needs two threads to interleave inside a window of a few instructions, and the fair `tokio::sync::RwLock` hides it on one thread. The ten tests in `folders_tests.rs` and the unit tests in `folders.rs` are the safety net for this refactor.

- [x] **Step 1: Record the green baseline.** Run from `daemon/`: `rtk cargo test -p nook-core --test folders_tests` and `rtk cargo test -p nook-core --lib folders`. Expected: all pass.
- [x] **Step 2: Change `apply` to the signature above.** Inside it, take `state.config.write().await` first. Clone `config.tools.filesystem.roots` into `next`. Call `edit(&mut next)` and return its error as a `String`. The rest of the body stays as it is now.
- [x] **Step 3: Rewrite `add`, `set_access`, and `remove` as calls to `apply`.** Each passes a closure that holds its current check and its current change of the list. `add` builds `Keepout::current()` before the call, and its closure calls `check_new_root`. `remove` returns `Some(directory)` from its closure. No function reads `state.config` outside `apply`, except `list`.
- [x] **Step 4: Move each `info!` line after the `apply` call, and emit it only when the result is `Ok`.** Keep the three messages and their fields. `add` logs the stored path, which the closure hands back through a local variable.
- [x] **Step 5: Run the tests of step 1 again.** Expected: all pass, the same count as in step 1.
- [x] **Step 6: Run `rtk cargo fmt --check` and `rtk cargo clippy --workspace --all-targets -- -D warnings` from `daemon/`.** Expected: no output from fmt, no warning from clippy.
- [x] **Step 7: Commit.** `git add daemon/crates/nook-core/src/folders.rs docs/plans/allowed-folders-hardening.md` and `git commit -m "fix(daemon): change the allowed folders under one lock and log after the save"`.

### Task 2: Drop the grants of a folder that left the disk

**Files:**
- Modify: `daemon/crates/nook-core/src/folders.rs` (the closure in `remove`)
- Test: `daemon/crates/nook-core/tests/folders_tests.rs`

- [x] **Step 1: Write the failing test.** Add it after `chat_grants_inside_a_removed_folder_are_dropped`.

```rust
#[tokio::test]
async fn chat_grants_inside_a_folder_deleted_from_disk_are_dropped() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let inside = area.base.join("one/file.txt");
    let result = h
        .approve_and_run(chat, "g1", WRITE, &write_args(&inside, "x"), "allow_for_chat_path")
        .await;
    assert_eq!(result["status"], "ok", "{result}");
    std::fs::remove_dir_all(area.base.join("one")).unwrap();

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    let store = h.state.policy.authorizer.grant_store();
    assert!(store
        .lock()
        .await
        .matching_scopes(chat, "write_local_file", None, inside.to_str(), None)
        .is_empty());
}
```

- [x] **Step 2: Run it.** From `daemon/`: `rtk cargo test -p nook-core --test folders_tests chat_grants_inside_a_folder_deleted`. Expected: FAIL on the last assertion.
- [x] **Step 3: Fix `remove`.** When `canonicalize` fails, use the stored path as it is: `Path::new(path).canonicalize().unwrap_or_else(|_| PathBuf::from(path))`. The stored path is already resolved, because `check_new_root` resolved it at the time of the add. Say that in a one-line comment.
- [x] **Step 4: Run the whole file.** `rtk cargo test -p nook-core --test folders_tests`. Expected: 11 pass.
- [x] **Step 5: Run fmt and clippy as in Task 1 step 6.**
- [x] **Step 6: Commit.** `git commit -m "fix(daemon): drop chat grants of a removed folder that no longer exists on disk"` with the two files and this plan.

### Task 3: Deny a stale approval with its real reason

**Files:**
- Modify: `daemon/crates/nook-core/src/tool_approval.rs` (`deny_stale_approvals` only)
- Test: `daemon/crates/nook-core/tests/folders_tests.rs`

**Interfaces:**
- Consumes: `ProviderError::Rejected(&'static str)` from `providers/mod.rs`, and `providers::roots::OUTSIDE_ROOTS`.

- [x] **Step 1: Write two tests.** Add them after `a_waiting_approval_in_another_folder_stays`.

```rust
#[tokio::test]
async fn a_read_only_change_denies_a_waiting_write_with_its_reason() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let target = area.base.join("two/waiting.txt");
    let mut call = h
        .execute(Uuid::new_v4(), "w1", WRITE, &write_args(&target, "x"))
        .await;
    assert!(matches!(h.panel().await, DaemonMessage::ApprovalRequested { .. }));

    folders::set_access(&h.state, &area.path("two"), RootAccess::Read)
        .await
        .unwrap();

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied", "{result}");
    assert_eq!(result["output"], "The folder is read-only");
    assert!(!target.exists());
    assert!(h.state.pending_approvals.lock().await.is_empty());
}

#[tokio::test]
async fn removing_the_last_folder_denies_a_waiting_write_as_outside() {
    let area = Area::new();
    let mut h = area.harness(&["one"]).await;
    let target = area.base.join("one/waiting.txt");
    let mut call = h
        .execute(Uuid::new_v4(), "w1", WRITE, &write_args(&target, "x"))
        .await;
    assert!(matches!(h.panel().await, DaemonMessage::ApprovalRequested { .. }));

    folders::remove(&h.state, &area.path("one")).await.unwrap();

    let result = tool_result(&mut call).await;
    assert_eq!(result["status"], "denied", "{result}");
    assert_eq!(result["output"], "Path is outside the allowed folders");
    assert!(h.state.pending_approvals.lock().await.is_empty());
}
```

- [x] **Step 2: Run them.** `rtk cargo test -p nook-core --test folders_tests waiting_write`. Expected: the first FAILS with left `"Path is outside the allowed folders"`. The second passes. It pins the behavior that step 3 must keep.
- [x] **Step 3: Carry the reason in `deny_stale_approvals`.** Replace the `filter` with a `filter_map` that keeps the call id and a `&'static str` reason. The reason is the text inside `ProviderError::Rejected`, and `OUTSIDE_ROOTS` for every other error. Deny each stale approval with its own reason. Put the mapping in one small private function, `stale_reason(error: &ProviderError) -> &'static str`.
- [x] **Step 4: Run the whole file.** Expected: 13 pass.
- [x] **Step 5: Run the other suites that use approvals.** `rtk cargo test -p nook-core --test filesystem_tool_tests --test provider_execution_tests --test tool_authorization_tests`. Expected: all pass.
- [x] **Step 6: Run fmt and clippy as in Task 1 step 6.**
- [x] **Step 7: Commit.** `git commit -m "fix(daemon): deny a stale approval with the reason the tool gives"`.

### Task 4: One busy path in the Folders tab, and two more native tests

**Files:**
- Modify: `panel/src/components/FoldersTab.tsx`
- Test: `panel/e2e/specs/folders.spec.mjs`

- [x] **Step 1: Add two tests to `folders.spec.mjs`.** Put them between "lets the user write after the access changes to read and write" and "refuses a path again after the folder is removed". Add `renameSync` to the `node:fs` import.

```js
  it('shows the reason and keeps the list when the same folder is added again', async () => {
    await openFoldersFromMenu()
    const before = (await $$('[data-testid="folder-row"]')).length
    await $('[data-testid="folders-add"]').click()
    const banner = await $('[data-testid="folders-error"]')
    await banner.waitForDisplayed({ timeout: 10_000 })
    if (!(await banner.getText()).includes('overlaps a folder that is already in the list')) {
      throw new Error(`Unexpected error text: ${await banner.getText()}`)
    }
    if ((await $$('[data-testid="folder-row"]')).length !== before) {
      throw new Error('A refused folder changed the list.')
    }
    await closeSettings()
  })

  it('marks a folder that left the disk and still lists it', async () => {
    const moved = `${listedExtra}-moved`
    renameSync(listedExtra, moved)
    try {
      await openFoldersFromMenu()
      const reason = await row(listedExtra).$('[data-testid="folder-reason"]')
      await reason.waitForDisplayed({ timeout: 10_000 })
      if (!(await reason.getText()).includes('The folder does not exist')) {
        throw new Error(`Unexpected reason: ${await reason.getText()}`)
      }
      await closeSettings()
    } finally {
      renameSync(moved, listedExtra)
    }
  })
```

- [x] **Step 2: Run the native suite.** From the repository root: `set -a && . ./.env && set +a && cd panel && env -u NOOK_API_KEY xvfb-run -a rtk npm run test:e2e:deterministic`. The run builds the app and takes several minutes. Expected: both new tests pass, because they pin behavior that exists. If one fails, stop and report the output. Do not change the test to make it pass.
- [x] **Step 3: Make `handleAdd` use `change`.** Change `change` to accept `() => Promise<RootInfo[] | null>` and to set the list only when the result is not `null`. `handleAdd` becomes one call to `change` that returns `null` when `pickFolder()` returns no path. A cancelled pick then leaves the list and the banner as they were (`setError(null)` runs only after a result that is not `null`). The file keeps one `try`/`catch`/`finally`.
- [x] **Step 4: Run the Panel checks from `panel/`.** `rtk npm run lint`, `rtk npm run test:unit`, `rtk npm run build`. Expected: zero warnings, all pass.
- [x] **Step 5: Run the native suite again as in step 2.** Expected: every spec passes, with the same count as step 2.
- [x] **Step 6: Commit.** `git commit -m "refactor(panel): use one busy path in the Folders tab and test the error and the missing-folder note"`.

A cancelled picker leaves the list unchanged by code (change() ignores null). No native test, because the test build cannot cancel the picker.

### Task 5: Documents

**Files:**
- Modify: `docs/plans/allowed-folders-view.md`, `docs/testing.md`, `docs/plans/README.md`, `CHANGELOG.md`

- [x] **Step 1: Tick the acceptance boxes in `docs/plans/allowed-folders-view.md`.** Tick a box only when a test that passed in this work proves it, and name that test after the line. Reword "Panel unit tests for the Folders tab" to "Native tests for the Folders tab (`panel/e2e/specs/folders.spec.mjs`)", because the Panel has no component test runner and this plan adds no dependency. Leave "Manual: the menu shows both items" open, with the note "needs a Mac".
- [x] **Step 2: Extend the `folders_tests.rs` paragraph in `docs/testing.md`** with the three new Rust tests and the two new native tests, in the same plain style.
- [x] **Step 3: List `allowed-folders-view.md` and this file under "Active plans" in `docs/plans/README.md`.**
- [x] **Step 4: Add under `## Unreleased` in `CHANGELOG.md`:** a `### Added` heading with the line "Manage the folders that the file and command tools can use in Settings, Folders. A change applies at once."
- [x] **Step 5: Commit.** `git commit -m "docs: record the allowed folders checks and the changelog entry"`.

### Task 6: Make the socket tests independent of the umask

Do this task after Task 3 and before Task 4.

**Files:**
- Modify: `daemon/crates/nook-core/tests/socket_ownership_tests.rs` (the helper `socket_in` only)

Cause: `Server::bind` in `server.rs` calls `libc::umask(0o177)` around `UnixListener::bind`. A umask is shared by every thread, so a test that runs `create_dir_all` in that moment makes a folder with mode `0600`. Do not change `server.rs`. Say this in a one-line `Warning:` comment in the helper.

- [x] **Step 1: Reproduce.** From `daemon/`, run `for i in 1 2 3 4 5 6; do rtk cargo test -p nook-core --test socket_ownership_tests; done`. Expected: at least one run FAILS with `PermissionDenied`. If all six pass, run it twelve times. If it still never fails, stop and report.
- [x] **Step 2: Fix `socket_in`.** After `create_dir_all`, set the folder mode to `0o700` with `std::fs::set_permissions`. Keep the signature.
- [x] **Step 3: Run the loop of step 1 again with twelve runs.** Expected: all twelve pass.
- [x] **Step 4: Run fmt and clippy as in Task 1 step 6.**
- [x] **Step 5: Commit.** `git commit -m "test(daemon): set the folder mode in the socket tests so a parallel umask cannot break them"`.

### Task 7: Make the outside-write check of the terminal spec true on Linux

Do this task before step 2 of Task 4. Task 4 step 2 needs the whole native suite to run.

**Files:**
- Modify: `panel/e2e/specs/terminal-tool.spec.mjs` (the test `runs an approved command inside the allowed folder only` only)

The boundary is the host file, and the spec already checks it with `existsSync(outside)`. Keep that check on every platform. Run the exit-code check only on macOS: wrap it in `if (process.platform === 'darwin')` and put a one-line comment above it that gives the reason (Linux gives the command a private `/tmp`, so the command exits 0 and the host stays untouched).

- [x] **Step 1: Confirm the cause.** From `daemon/`, run `rtk cargo test -p nook-core --test sandbox_tests cannot_write_outside_the_roots`. Expected: PASS. This shows the sandbox boundary holds on Linux.
- [x] **Step 2: Edit the spec as described above.** Change nothing else in the file.
- [x] **Step 3: Run only this spec.** From `panel/`, with the environment loaded: `env -u NOOK_API_KEY xvfb-run -a rtk npm run test:e2e:deterministic -- --spec e2e/specs/terminal-tool.spec.mjs`. If the runner rejects `--spec`, run the whole suite instead. Expected: the four tests of the spec pass.
- [x] **Step 4: Commit.** `git add panel/e2e/specs/terminal-tool.spec.mjs docs/plans/allowed-folders-hardening.md` and `git commit -m "test(e2e): check the host file, not the exit code, for a write outside the folder on Linux"`. Do not add `folders.spec.mjs` to this commit.

### Task 8: Close the five SonarCloud issues

**Files:**
- Modify: `daemon/crates/nook-core/src/providers/mod.rs`, `daemon/crates/nook-core/src/supervisor.rs`, `panel/src/components/FoldersTab.tsx`

No new test is possible for a lint rule. The existing tests are the safety net. Change nothing else in these files.

- [x] **Step 1: Replace the four closures.** In each of the four places `.unwrap_or_else(|error| error.into_inner())` becomes `.unwrap_or_else(PoisonError::into_inner)`. Add `use std::sync::PoisonError;` to each file, next to the existing `use std::sync::Arc;` (write it as `use std::sync::{Arc, PoisonError};`).
- [x] **Step 2: Replace the `role="group"` span in `FoldersTab.tsx` (line 99).** Use `<fieldset className="flex min-w-0 rounded-md border border-white/10 overflow-hidden">` with a first child `<legend className="sr-only">Access for {root.path}</legend>`. Remove the `role` and the `aria-label` attributes. Keep the buttons inside it as they are.
- [x] **Step 3: Run the Daemon checks from `daemon/`.** `rtk cargo fmt --check`, `rtk cargo clippy --workspace --all-targets -- -D warnings`, `rtk cargo test -p nook-core --test folders_tests --test supervisor_tests`. Expected: no output from fmt, no warning from clippy, all tests pass.
- [x] **Step 4: Run the Panel checks from `panel/`.** `rtk npm run lint`, `rtk npm run test:unit`, `rtk npm run build`. Expected: all pass. Do NOT run the native suite. The orchestrator runs it.
- [x] **Step 5: Commit.** `git add` the three files and this plan, then `git commit -m "fix: close the SonarCloud issues in the folders change"`.

### Task 9: Resolve `HOME` without failing open, and make the home error text true

**Files:**
- Modify: `daemon/crates/nook-core/src/folders.rs` (the constant `HOME_OR_ABOVE`, `Keepout::current`, the test module)

**Interfaces:**
- Produces: `fn resolve_home(home: PathBuf) -> PathBuf` (private, in `folders.rs`). It returns the resolved path, or the path as it is when it cannot be resolved.

- [ ] **Step 1: Write the failing tests** in the `tests` module of `folders.rs`.

```rust
#[test]
fn a_home_folder_that_cannot_be_resolved_is_kept_as_it_is() {
    let missing = PathBuf::from("/nowhere/home-that-does-not-exist");
    assert_eq!(resolve_home(missing.clone()), missing);
}

#[cfg(unix)]
#[test]
fn a_home_folder_behind_a_link_is_resolved() {
    let l = layout();
    let link = l.home.parent().unwrap().join("home-link");
    std::os::unix::fs::symlink(&l.home, &link).unwrap();
    assert_eq!(resolve_home(link), l.home);
}
```

- [ ] **Step 2: Run them.** From `daemon/`: `rtk cargo test -p nook-core --lib folders::tests::a_home_folder`. Expected: FAIL to compile with "cannot find function `resolve_home`".
- [ ] **Step 3: Add `resolve_home`** above `Keepout` and use it in `Keepout::current`: replace `.and_then(|home| home.canonicalize().ok())` with `.map(resolve_home)`.
- [ ] **Step 4: Change the text of `HOME_OR_ABOVE`** to `"That is your home folder or a folder above it. Choose a smaller folder"`. The tests compare the constant, so no test text changes. Run `rtk rg "Choose a folder inside your home" /home/azureuser/nook --glob '!target' --glob '!node_modules'` and update any other place that holds the old text. Expected after the change: no match.
- [ ] **Step 5: Run the folders tests.** `rtk cargo test -p nook-core --lib folders` and `rtk cargo test -p nook-core --test folders_tests`. Expected: all pass.
- [ ] **Step 6: Run fmt and clippy as in Task 1 step 6.**
- [ ] **Step 7: Commit.** `git commit -m "fix(daemon): keep an unresolved home folder in the keepout and fix the home error text"`.

### Task 10: Drop the chat grants of a folder that goes from read and write to read only

**Files:**
- Modify: `daemon/crates/nook-core/src/folders.rs` (the closure in `set_access` only)
- Test: `daemon/crates/nook-core/tests/folders_tests.rs`

- [ ] **Step 1: Write the failing test** after `chat_grants_inside_a_folder_deleted_from_disk_are_dropped`. Copy the structure of `chat_grants_inside_a_removed_folder_are_dropped`.

```rust
#[tokio::test]
async fn a_read_only_change_drops_the_chat_grants_inside_the_folder() {
    let area = Area::new();
    let mut h = area.harness(&["one", "two"]).await;
    let chat = Uuid::new_v4();
    let inside = area.base.join("two/file.txt");
    let other = area.base.join("one/file.txt");
    for (call, path) in [("g1", &inside), ("g2", &other)] {
        let result = h
            .approve_and_run(chat, call, WRITE, &write_args(path, "x"), "allow_for_chat_path")
            .await;
        assert_eq!(result["status"], "ok", "{result}");
    }

    folders::set_access(&h.state, &area.path("two"), RootAccess::Read)
        .await
        .unwrap();

    let store = h.state.policy.authorizer.grant_store();
    let guard = store.lock().await;
    assert!(guard
        .matching_scopes(chat, "write_local_file", None, inside.to_str(), None)
        .is_empty());
    assert!(!guard
        .matching_scopes(chat, "write_local_file", None, other.to_str(), None)
        .is_empty());
}
```

- [ ] **Step 2: Run it.** `rtk cargo test -p nook-core --test folders_tests a_read_only_change_drops`. Expected: FAIL on the first `assert!` (the grant is still there).
- [ ] **Step 3: Fix the closure in `set_access`.** Read the old access before the change. Return `Ok(Some(PathBuf::from(path)))` when the old access is `RootAccess::ReadWrite` and the new access is `RootAccess::Read`. Return `Ok(None)` in every other case. `apply` already drops the grants inside the returned folder. Add a one-line comment that gives the reason: an old write approval must not revive when the folder becomes writable again.
- [ ] **Step 4: Run the whole file.** `rtk cargo test -p nook-core --test folders_tests`. Expected: 14 pass.
- [ ] **Step 5: Run fmt and clippy as in Task 1 step 6.**
- [ ] **Step 6: Commit.** `git commit -m "fix(daemon): drop chat grants inside a folder that becomes read only"`.

### Task 11: Split the steps after the save out of `apply`

**Files:**
- Modify: `daemon/crates/nook-core/src/folders.rs` (`apply` only)

No test changes. The 14 tests in `folders_tests.rs` and the unit tests in `folders.rs` are the safety net.

**Interfaces:**
- Produces: `async fn settle(state: &Arc<AppState>, removed: Option<PathBuf>)` (private). It holds what `apply` does after the lock block: drop the chat grants inside `removed`, then call `deny_stale_approvals`. The `info!` line about the rebuilt tools stays in `apply`.

- [ ] **Step 1: Run the tests as a baseline.** `rtk cargo test -p nook-core --test folders_tests` and `rtk cargo test -p nook-core --lib folders`. Expected: all pass.
- [ ] **Step 2: Extract `settle`** as described, and call it from `apply` as the last step. Keep the order: grants first, then stale approvals.
- [ ] **Step 3: Run the tests of step 1 again, then fmt and clippy as in Task 1 step 6.** Expected: the same counts, no warning.
- [ ] **Step 4: Commit.** `git commit -m "refactor(daemon): move the steps after a folder change into settle"`.

### Task 12: Pin that `config.toml` is replaced through a rename

**Files:**
- Test: `daemon/crates/nook-core/src/config.rs` (the `tests` module)
- Modify: `docs/plans/allowed-folders-hardening.md` (tick Tasks 9 to 12 and add the note under "Review response" below)

The test pins existing behavior and passes at once. It answers review finding 1.

- [ ] **Step 1: Add the test** after `updating_the_roots_keeps_the_file_private`.

```rust
#[cfg(unix)]
#[test]
fn updating_the_roots_replaces_the_file_and_leaves_no_temporary_file() {
    use std::os::unix::fs::MetadataExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, HAND_EDITED).unwrap();
    let before = std::fs::metadata(&path).unwrap().ino();

    Config::update_filesystem_roots(&path, &[]).unwrap();

    assert_ne!(std::fs::metadata(&path).unwrap().ino(), before);
    assert!(!dir.path().join("config.toml.tmp").exists());
}
```

- [ ] **Step 2: Run it.** `rtk cargo test -p nook-core --lib updating_the_roots_replaces`. Expected: PASS.
- [ ] **Step 3: Run fmt and clippy as in Task 1 step 6.**
- [ ] **Step 4: Add this section at the end of the plan file, before "Final report":**

```
## Review response (GitHub review on PR 34)

- Finding 1, in-place write of `config.toml`: not a fault. `write_private` writes `config.toml.tmp` with mode `0600`, syncs it, and renames it over the file. A test pins this (Task 12).
- Finding 2, folders outside home: the plan allows them. Only the error text was wrong (Task 9). Whether to limit folders to the home folder, or to block system folders such as `/etc`, is an open decision for the owner.
- Finding 3, stale write grants after a downgrade: fixed (Task 10).
- Finding 4, race between an approval and a removal: not changed. A call checks its path against the registry that it took, and the file tools have no persistent grants. The race can leave only a chat grant for a path that is outside every folder.
- Finding 5, unresolved home folder: fixed (Task 9).
- Finding 6, `apply` does too many jobs: fixed (Task 11).
```

- [ ] **Step 5: Add one sentence to the `folders_tests.rs` paragraph of `docs/testing.md`:** the file has 14 tests and one covers "chat grants inside a folder that becomes read only are dropped".
- [ ] **Step 6: Commit.** `git add` the two files and `git commit -m "test(daemon): pin the atomic config write and record the review response"`.

## Final report

End with a report that lists, for each task: the commit hash, the exact test commands that you ran, and their pass and fail counts. List every place where you stopped or where the result differed from "Expected".
