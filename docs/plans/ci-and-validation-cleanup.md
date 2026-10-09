# CI and validation cleanup plan

The native E2E work passes `actionlint`, `git diff --check`, the deterministic
native suite, Rust tests, Worker tests, Panel lint and build, Ruff, and contract
tests. This plan covers the remaining validation work without mixing unrelated
formatting changes into the native E2E gate commit.

## Current state

`actionlint` passes for `.github/workflows/native-e2e-gate.yml`. No workflow
syntax or expression fix is required.

`git diff --check` passes. No whitespace-error fix is required.

`cargo clippy --workspace --all-targets -- -D warnings` fails on two existing
`clippy::items_after_test_module` findings:

- `daemon/crates/nook-core/src/keychain.rs` has `mod tests` before
  `set_api_key`.
- `daemon/crates/nook-core/src/supervisor.rs` has `mod tests` before the
  `WorkerSupervisor` implementation.

`cargo fmt --all -- --check` reports formatting changes in existing files,
including `db.rs`, `tests/db_tests.rs`, and `nookctl/src/main.rs`. The native
E2E implementation did not modify those files.

The live native check makes one real model request for India news. The
repository already stores its endpoint, model, and API key in the
`native-e2e-live` environment. The manual workflow is the intended place to
run this check.

## Work items

### 1. Fix Clippy module placement

Move each `#[cfg(test)] mod tests` block to the end of its file, after all
production items.

1. Move the test module in `keychain.rs` below `set_api_key`.
2. Move the test module in `supervisor.rs` below the `impl WorkerSupervisor`
   block.
3. Do not change the tests or production behavior.
4. Run the focused unit tests, then run:

   ```sh
   rtk cargo clippy --workspace --all-targets -- -D warnings
   ```

Done condition: Clippy exits successfully with warnings treated as errors.

### 2. Make Rust formatting a deliberate cleanup change

Run the formatter in a dedicated branch or commit after the Clippy change.

1. Run `rtk cargo fmt --all` from `daemon/`.
2. Review every changed file. The expected initial set includes files outside
   the native E2E implementation.
3. Keep formatting-only changes separate from behavioral changes.
4. Run:

   ```sh
   rtk cargo fmt --all -- --check
   rtk cargo test --workspace
   rtk cargo clippy --workspace --all-targets -- -D warnings
   ```

Done condition: Rust formatting, tests, and Clippy pass. The commit contains no
behavioral change.

### 3. Run the live native check through GitHub Actions

Run the live check after the E2E branch has an open pull request. This keeps
endpoint secrets inside the protected `native-e2e-live` environment and records
the result against the PR head SHA.

1. Push the E2E branch and open a pull request to `main`.
2. Start the `Native Tauri E2E Gate` workflow with that pull request number.
3. Approve the `native-e2e-live` environment if its protection rule requests
   approval.
4. Confirm the deterministic job passes before the live job starts.
5. Confirm the live job completes one India-news request, approves the required
   web tool calls, and finds source links in the completed response.
6. Confirm `Native Tauri E2E Gate` reports success on the same PR head SHA.
7. If the PR head changes while the workflow runs, rerun the workflow for the
   new commit.

Done condition: The workflow posts a successful `Native Tauri E2E Gate` status
on the open PR's current head commit.

### 4. Require the gate only after a successful dry run

Add the status context to the `main` ruleset only after the workflow has passed
on a real pull request.

1. Update ruleset `24413973` to require `Native Tauri E2E Gate`.
2. Keep the existing `SonarCloud Code Analysis` requirement.
3. Verify GitHub blocks a merge while the native gate is missing, pending, or
   failing.
4. Verify a new commit changes the required SHA and requires another native-gate
   run.

Done condition: GitHub requires both status checks before a pull request can
merge into `main`.

## Commit boundaries

Keep the work in these commits:

1. `test(e2e): add native macOS merge gate` for the E2E runner, native tests,
   Tauri feature, workflow, generated lockfiles, and supporting fixes.
2. `chore(rust): satisfy Clippy module placement` for the two moved test
   modules.
3. `style(rust): format daemon workspace` for formatter-only changes.

The live workflow run and GitHub ruleset update are repository operations. They
do not require source commits unless the run exposes a defect.
