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

- [ ] **Step 1: Write the failing test.** Add it after `chat_grants_inside_a_removed_folder_are_dropped`.

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

- [ ] **Step 2: Run it.** From `daemon/`: `rtk cargo test -p nook-core --test folders_tests chat_grants_inside_a_folder_deleted`. Expected: FAIL on the last assertion.
- [ ] **Step 3: Fix `remove`.** When `canonicalize` fails, use the stored path as it is: `Path::new(path).canonicalize().unwrap_or_else(|_| PathBuf::from(path))`. The stored path is already resolved, because `check_new_root` resolved it at the time of the add. Say that in a one-line comment.
- [ ] **Step 4: Run the whole file.** `rtk cargo test -p nook-core --test folders_tests`. Expected: 11 pass.
- [ ] **Step 5: Run fmt and clippy as in Task 1 step 6.**
- [ ] **Step 6: Commit.** `git commit -m "fix(daemon): drop chat grants of a removed folder that no longer exists on disk"` with the two files and this plan.

### Task 3: Deny a stale approval with its real reason

**Files:**
- Modify: `daemon/crates/nook-core/src/tool_approval.rs` (`deny_stale_approvals` only)
- Test: `daemon/crates/nook-core/tests/folders_tests.rs`

**Interfaces:**
- Consumes: `ProviderError::Rejected(&'static str)` from `providers/mod.rs`, and `providers::roots::OUTSIDE_ROOTS`.

- [ ] **Step 1: Write two tests.** Add them after `a_waiting_approval_in_another_folder_stays`.

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

- [ ] **Step 2: Run them.** `rtk cargo test -p nook-core --test folders_tests waiting_write`. Expected: the first FAILS with left `"Path is outside the allowed folders"`. The second passes. It pins the behavior that step 3 must keep.
- [ ] **Step 3: Carry the reason in `deny_stale_approvals`.** Replace the `filter` with a `filter_map` that keeps the call id and a `&'static str` reason. The reason is the text inside `ProviderError::Rejected`, and `OUTSIDE_ROOTS` for every other error. Deny each stale approval with its own reason. Put the mapping in one small private function, `stale_reason(error: &ProviderError) -> &'static str`.
- [ ] **Step 4: Run the whole file.** Expected: 13 pass.
- [ ] **Step 5: Run the other suites that use approvals.** `rtk cargo test -p nook-core --test filesystem_tool_tests --test provider_execution_tests --test tool_authorization_tests`. Expected: all pass.
- [ ] **Step 6: Run fmt and clippy as in Task 1 step 6.**
- [ ] **Step 7: Commit.** `git commit -m "fix(daemon): deny a stale approval with the reason the tool gives"`.

### Task 4: One busy path in the Folders tab, and two more native tests

**Files:**
- Modify: `panel/src/components/FoldersTab.tsx`
- Test: `panel/e2e/specs/folders.spec.mjs`

- [ ] **Step 1: Add two tests to `folders.spec.mjs`.** Put them between "lets the user write after the access changes to read and write" and "refuses a path again after the folder is removed". Add `renameSync` to the `node:fs` import.

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

- [ ] **Step 2: Run the native suite.** From the repository root: `set -a && . ./.env && set +a && cd panel && env -u NOOK_API_KEY xvfb-run -a rtk npm run test:e2e:deterministic`. The run builds the app and takes several minutes. Expected: both new tests pass, because they pin behavior that exists. If one fails, stop and report the output. Do not change the test to make it pass.
- [ ] **Step 3: Make `handleAdd` use `change`.** Change `change` to accept `() => Promise<RootInfo[] | null>` and to set the list only when the result is not `null`. `handleAdd` becomes one call to `change` that returns `null` when `pickFolder()` returns no path. A cancelled pick then leaves the list and the banner as they were (`setError(null)` runs only after a result that is not `null`). The file keeps one `try`/`catch`/`finally`.
- [ ] **Step 4: Run the Panel checks from `panel/`.** `rtk npm run lint`, `rtk npm run test:unit`, `rtk npm run build`. Expected: zero warnings, all pass.
- [ ] **Step 5: Run the native suite again as in step 2.** Expected: every spec passes, with the same count as step 2.
- [ ] **Step 6: Commit.** `git commit -m "refactor(panel): use one busy path in the Folders tab and test the error and the missing-folder note"`.

### Task 5: Documents

**Files:**
- Modify: `docs/plans/allowed-folders-view.md`, `docs/testing.md`, `docs/plans/README.md`, `CHANGELOG.md`

- [ ] **Step 1: Tick the acceptance boxes in `docs/plans/allowed-folders-view.md`.** Tick a box only when a test that passed in this work proves it, and name that test after the line. Reword "Panel unit tests for the Folders tab" to "Native tests for the Folders tab (`panel/e2e/specs/folders.spec.mjs`)", because the Panel has no component test runner and this plan adds no dependency. Leave "Manual: the menu shows both items" open, with the note "needs a Mac".
- [ ] **Step 2: Extend the `folders_tests.rs` paragraph in `docs/testing.md`** with the three new Rust tests and the two new native tests, in the same plain style.
- [ ] **Step 3: List `allowed-folders-view.md` and this file under "Active plans" in `docs/plans/README.md`.**
- [ ] **Step 4: Add under `## Unreleased` in `CHANGELOG.md`:** a `### Added` heading with the line "Manage the folders that the file and command tools can use in Settings, Folders. A change applies at once."
- [ ] **Step 5: Commit.** `git commit -m "docs: record the allowed folders checks and the changelog entry"`.

### Task 6: Make the socket tests independent of the umask

Do this task after Task 3 and before Task 4.

**Files:**
- Modify: `daemon/crates/nook-core/tests/socket_ownership_tests.rs` (the helper `socket_in` only)

Cause: `Server::bind` in `server.rs` calls `libc::umask(0o177)` around `UnixListener::bind`. A umask is shared by every thread, so a test that runs `create_dir_all` in that moment makes a folder with mode `0600`. Do not change `server.rs`. Say this in a one-line `Warning:` comment in the helper.

- [ ] **Step 1: Reproduce.** From `daemon/`, run `for i in 1 2 3 4 5 6; do rtk cargo test -p nook-core --test socket_ownership_tests; done`. Expected: at least one run FAILS with `PermissionDenied`. If all six pass, run it twelve times. If it still never fails, stop and report.
- [ ] **Step 2: Fix `socket_in`.** After `create_dir_all`, set the folder mode to `0o700` with `std::fs::set_permissions`. Keep the signature.
- [ ] **Step 3: Run the loop of step 1 again with twelve runs.** Expected: all twelve pass.
- [ ] **Step 4: Run fmt and clippy as in Task 1 step 6.**
- [ ] **Step 5: Commit.** `git commit -m "test(daemon): set the folder mode in the socket tests so a parallel umask cannot break them"`.

## Final report

End with a report that lists, for each task: the commit hash, the exact test commands that you ran, and their pass and fail counts. List every place where you stopped or where the result differed from "Expected".
