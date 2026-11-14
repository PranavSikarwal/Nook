# Plan: Notes tools run by the Daemon

Spec: `docs/tasks/task-PR-40/specs/design.md`.

Each task is one commit with one checkbox. An implementer runs the tasks of its
brief in order, writes the failing test first, and ticks the box in the same
commit. "Daemon checks" means, from `daemon/`: `rtk cargo test` and `rtk cargo
clippy --workspace --all-targets -- -D warnings`. "Contract checks" means, from
the repository root: `rtk uv run --with pytest --with jsonschema pytest
tests/contracts panel/e2e/test_fake_worker.py`. "Panel checks" means, from
`panel/`: `rtk npm run lint`, `rtk npm run test:unit` and `rtk npm run build`.
Every command must pass with zero warnings.

A task that needs a decision that the spec does not make stops and reports the
question, the options and a recommendation.

## Tasks

### Daemon: building blocks

- [ ] **1. Share the process helpers of the sandbox.** `daemon/crates/nook-core/src/sandbox.rs`. Make `GroupKill` (and its field), `read_capped` and `join_reader` `pub(crate)`, all `#[cfg(unix)]`, as spec "Run in the Daemon" rule 6 says. No behavior change. Check: Daemon checks.
- [ ] **2. Add the Notes folder setting to the config.** `daemon/crates/nook-core/src/config.rs`. Add `[tools.notes] folder` to `ToolsConfig`, default `Nook`. Add one function that checks a folder name (trim spaces, 1 to 100 characters, no control character, no `:`, not `Recently Deleted` in any letter case), with unit tests for each rule and for a config file that holds a bad name. A bad name does not stop the load. The function that reads it reports the fixed reason `The Notes folder name is not valid`. Check: Daemon checks.
- [ ] **3. Keep the refused value out of the schema message.** `daemon/crates/nook-core/src/providers/mod.rs`. `ProviderRegistry::facts` builds `Invalid arguments: {error}` from the `jsonschema` error. Change it to name only the keyword and the property path, never the value and never the error `kind` (spec "Data handling" and the "Registry, schema message" test). Test it for a file tool and for the `Fixed` provider of the unit tests. Check: Daemon checks.
- [ ] **4. Add the argument checks and the HTML builders.** New `daemon/crates/nook-core/src/providers/notes.rs`, and `#[cfg(unix)] mod notes;` in `providers/mod.rs` (same gate as `mcp`). Write the check functions of spec "The tools" (title trim and refusal, `body`, `text`, `note_id` with the `x-coredata://` prefix and its character set, NUL refusal) and the HTML builders (`&`, `<` and `>` escaped, each line in a `div`, the title as the first `div` for create and edit, only the lines for append). Test each rule: `Title\n`, `Title\r\n`, `\n`, `Title\n\n`, `Ti\ntle`, NUL, `-e` as a value, a `note_id` without the prefix, and the escaping. Check: Daemon checks.
- [ ] **5. Add the `osascript` runner.** `daemon/crates/nook-core/src/providers/notes.rs`. One runner type holds the program path and the `Arc<tokio::sync::Mutex<()>>`. It runs `<program> -e <script> <arg>...` with the `:` prefix on every data argument, the environment `LANG=en_US.UTF-8` only, the working folder `/`, no standard input, its own process group, `GroupKill`, `read_capped` and `join_reader`, the 60 second limit, an output limit that the caller sets, and one run at a time. It maps the kept standard error to the results of spec "Run in the Daemon" rule 8 (`(-1743)`, `(-1728)` or `(9001)`, `(9002)`, other) and logs the exit status and the error number, never the output text. Tests use a fake `osascript` shell script that writes its arguments, `pwd -P` and its exit code to a file: three arguments for a one-value call, the `:` prefix, a value `-e`, the timeout kill with a gone process group, a cancel that kills the group, an output of 1 MiB that is drained, the four result kinds, parallel calls that never overlap, and (Linux only, `#[cfg(target_os = "linux")]`) the exact environment through `/proc/$$/environ`. Check: Daemon checks.

### Daemon: the six tools

- [ ] **6. Add the create, list and read tools.** `daemon/crates/nook-core/src/providers/notes.rs`. The three scripts of the spec as constants, the descriptors (schemas with `required` and `additionalProperties: false`, no `pattern`, the descriptions that name the folder, say that note text is data and, for create, hold the constant `The title is one line.`), `facts` with the summaries of the approval table, `call` with the check functions then the runner, and the fixed result texts. List and read keep `MAX_TOOL_OUTPUT_BYTES + 1` bytes (from `tool_approval.rs`), and the list script asks for 101 notes and the Daemon keeps 100 and adds `More notes exist.`. The provider reports the source `notes` ("Notes tools", `ready`, six tools). Tests call `Provider::call` and `facts` directly with the fake program: arguments, results, the 101-line list, the cut read, the boundary at `MAX_TOOL_OUTPUT_BYTES` (for a list, sized minus the header), `[output truncated]`, a four-byte character at the cut, and invalid arguments that start no process. Check: Daemon checks.
- [ ] **7. Add the append, edit and delete tools.** `daemon/crates/nook-core/src/providers/notes.rs`. The three scripts of the spec, their descriptors (`note_id`, `note_title` and the other arguments), `facts` summaries (note title, new title, first 200 characters with the length), `call` and the fixed result texts (`Added the text to the note ...`, `Changed the note ...`, `Deleted the note ...`). Tests as for task 6, plus the title and not-found messages. Check: Daemon checks.

### Daemon: policy and wiring

- [ ] **8. Add the policy entries.** `policy/tools.yaml`, `policy/cedar/schema.cedarschema`, `policy/cedar/policies.cedar`, and a policy test in `daemon/crates/nook-core/tests/` (the real policy files). Six entries and six Cedar actions: list, read, create, append and edit offer `allow_once`, `allow_for_chat` and `deny` and Cedar permits grant scope `once` or `chat`. Delete offers `allow_once` and `deny` and Cedar permits only `once`. No `always_allow`. Tests: each scope rule, no permanent grant, and `nookd --check-policy` prints `policy ok`. Check: Daemon checks, and `cargo run -p nookd -- --check-policy` from `daemon/`.
- [ ] **9. Wire the provider into the registry.** `daemon/crates/nook-core/src/providers/mod.rs`, `daemon/crates/nookd/src/main.rs`, `daemon/crates/nook-core/tests/common/mod.rs`. `build_registry(tools, notes_program: Option<PathBuf>)`, a `Carry` field for the program and one for the lock, and `assemble` adds `NotesProvider` for `Some(path)` when the file exists and the folder name is valid, else the source `off` with the fixed reason (`Notes was not found on this Mac` or `The Notes folder name is not valid`) and one log line. `nookd` passes `Some("/usr/bin/osascript")` on macOS and the path of `NOOK_E2E_OSASCRIPT` only under `#[cfg(feature = "e2e")]` on every platform, and `None` elsewhere. The harness takes the new parameter. Tests: `None` gives no Notes tool, a fake path gives the six tools, a missing file and a bad name give `off` with the fixed reason, and `rebuilt` after a folder change keeps the tools and the same lock. Check: Daemon checks.
- [ ] **10. Test the Notes tools through the socket.** New `daemon/crates/nook-core/tests/notes_tool_tests.rs` (starts with `#![cfg(unix)]`). With the real socket, the fake Worker and the fake program: the program runs only after an allow, a deny leaves the file empty, a cancel before the approval does not run it, a call with a registry built with `None` is denied with `Tool runs in the Worker`, the delete approval has no chat button, and two parallel calls never overlap, also across a `rebuilt`. Check: Daemon checks.

### Daemon: the folder change

- [ ] **11. Drop chat grants by tool.** `daemon/crates/nook-core/src/policy.rs`. Add `GrantStore::remove_tools` that removes the chat grants of the given tool ids and no other grant. Unit test. Check: Daemon checks.
- [ ] **12. Let the call carry its registry.** `daemon/crates/nook-core/src/tool_approval.rs`, `daemon/crates/nook-core/src/providers/mod.rs`. `Execution` gets `registry: Arc<ProviderRegistry>` and `start_execution` calls it. `ProviderRegistry` gets a manual `Debug` that prints the tool ids. Read the registry once for `contains`, `normalize_with` and the `Execution`. No other behavior change, so every existing test must still pass. Check: Daemon checks.
- [ ] **13. Make a Notes call atomic against a folder change.** `daemon/crates/nook-core/src/tool_approval.rs`, `daemon/crates/nook-core/src/server.rs`, `daemon/crates/nook-core/tests/common/mod.rs`. Follow the four rules of spec "The folder setting": both paths take the read lock of `state.config` at the start and release it as soon as the `Execution` holds the registry and the grants are checked, no code under it reads `state.config`, `PendingApprovalEntry` stores the folder of its card, a decision compares it under the read lock before it records a grant and denies with `The Notes folder changed` on a mismatch, and a new function denies every pending Notes approval and skips `decision_in_flight`. Tests: the test holds `state.config.write()`, sends the decision, changes the state in memory, releases the guard and expects the denial and no grant; the other order; the chat-grant path; and an approval and a grant of another tool that stay. Check: Daemon checks.
- [ ] **14. Add the Notes folder messages.** `contracts/panel-daemon.schema.json`, `contracts/examples/panel-daemon.ndjson`, `tests/contracts/test_contracts.py`, `daemon/crates/nook-core/src/protocol.rs`, `server.rs`, new `daemon/crates/nook-core/src/notes_folder.rs`, `panel/src-tauri/src/lib.rs` (two thin commands that call `ask_daemon`), `panel/src/lib/daemon.ts` (the two functions and the browser mock), `panel/src/lib/types.ts`. The messages `get_notes_settings`, `set_notes_folder` and `notes_settings` (`id`, `folder`, `supported`, optional `reason`). The change runs inside the write lock of `state.config` as spec "The folder setting" says: check the name, write `config.toml` keeping the comments (a failed write gives the fixed error `The folder could not be saved` and changes nothing), deny the pending Notes approvals with the function of task 13, drop the chat grants with task 11, rebuild the tools and send a new catalog. The contract test gets the negative cases (`set_notes_folder` with an empty or a missing `folder`, `notes_settings` without `supported`). Socket tests: `get_notes_settings`, a good change, a bad name, a bad name already in `config.toml` that a good change repairs, `supported` false with the path `None`, the failed write, and another tool's approval and grant that stay. Check: Daemon checks, Contract checks, Panel checks, and `rtk cargo check` from `panel/src-tauri/`.

### Panel and bundle

- [ ] **15. Add the Notes folder field to the Settings tab.** New `panel/src/lib/notesFolder.ts` (pure functions: the field is hidden when `supported` is false, it shows the stored folder and the `reason`, and Save is on only when the value changed and is not empty after trimming), new `panel/e2e/unit/notes-folder.test.mjs` (run by `node --test`), and `panel/src/components/ToolsTab.tsx`. Show a line that says that macOS asks for permission on the first call and does not ask again after a refusal. Check: Panel checks.
- [ ] **16. Add the native test flow.** `panel/e2e/run.mjs` (set `NOOK_E2E_OSASCRIPT` and write the fake script; the build already has `--features e2e`), `panel/e2e/fake_worker.py` (a command that calls the create and delete tools), `panel/e2e/test_fake_worker.py` (one test for it), a new spec in `panel/e2e/specs/`, and `panel/e2e/specs/tools.spec.mjs` (four sources become five). The spec checks that the program ran only after Allow, that a deny leaves the file empty, that a cancel before the approval does not run it, that the delete approval has no chat button, and that a new folder from the Settings field reaches the next call. Check: Contract checks, and `rtk npm run test:e2e:deterministic` from `panel/`.
- [ ] **17. Add the macOS permission settings.** New `panel/src-tauri/Info.plist` (`NSAppleEventsUsageDescription` with the text "Nook needs to control Notes so that the assistant can work with the notes that you approve."), new `panel/src-tauri/entitlements.plist` (`com.apple.security.automation.apple-events`), `panel/src-tauri/tauri.conf.json` (`bundle.macOS.entitlements`), and the same usage description in the `Info.plist` that `scripts/bundle.sh` writes (that script gets no `--entitlements` flag). No change to the signing identity or the hardened runtime. Check: `python3 -c "import json,plistlib,sys; json.load(open('panel/src-tauri/tauri.conf.json')); [plistlib.load(open(f,'rb')) for f in ('panel/src-tauri/Info.plist','panel/src-tauri/entitlements.plist')]"` from the repository root, `bash -n scripts/bundle.sh`, and `rtk cargo check` from `panel/src-tauri/`.

### Docs

- [ ] **18. Rewrite the ADRs.** `git mv docs/adr/0008-panel-runs-desktop-app-tools.md docs/adr/0008-daemon-runs-desktop-app-tools-outside-the-sandbox.md` and rewrite it for this route (status Accepted, no link to the deleted draft, and the old reason against the Daemon kept as a recorded consequence), and add one short paragraph to `docs/adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md` that names the Notes tools as the exception. Check: `git grep "0008-panel-runs"` finds nothing, and `git diff --check` is clean.
- [ ] **19. Document the tools.** `docs/tools-and-authorization.md` (a "Notes tools" section, the folder setting, the approval table, the Cedar actions, the failure messages, the denial `Tool runs in the Worker` where the tools are not offered, and the corrected table "Where each tool runs" in the section "Tools that run in the Daemon") and `docs/architecture.md` ("Tool boundary"). Check: `git diff --check`, and read each statement against the code.
- [ ] **20. Document the contracts, the tests and the release.** `docs/contracts.md` (the three messages, and `notes` in the sentence that lists the source ids), `docs/testing.md` (the new tests, the fake `osascript` and the `NOOK_E2E_OSASCRIPT` setting) and `docs/release.md` (the usage description and entitlement, and that ad hoc builds may ask again after an update). Check: `git diff --check`, and Contract checks.
- [ ] **21. Add the changelog line.** `CHANGELOG.md`, one line under "Unreleased". Check: `git diff --check`.

## Manual test on a Mac (owner, before gate 2)

These steps are not tasks. The orchestrator writes the result into `reviews.md`.
The owner can start from the package artifact (run `package.yml` on the branch)
or from a local build.

1. Run `codesign -d --entitlements :- Nook.app` and `plutil -p Nook.app/Contents/Info.plist`, and check that the usage description and the entitlement are in the bundle.
2. Ask the assistant to create a note and check the macOS prompt: it must name Nook. If it names `nookd` or shows no prompt, tell the orchestrator, because that is a new gap (spec Risk 1).
3. Run each of the six tools on real notes: list, read, create, append, edit and delete. Check the title in the Notes list, that note ids start with `x-coredata://`, and that a delete leaves the note in Recently Deleted.
4. Check the hard cases: a title that starts with `-`, a title with non-ASCII characters, a title typed with Shift and Return, a folder name with non-ASCII characters or a leading `-`, a body of 20,000 line breaks, a note over 20,000 characters, an append to a note that has a list or an image, a wrong title, a note in another folder, and a locked note.
5. Change the folder in Settings, and check that the next call uses the new folder and that a waiting approval of the old folder is denied.
6. Refuse the macOS prompt once, and check the message and the line in the Settings field.

## Where each decision goes

| Spec part or gap | Tasks |
| --- | --- |
| G1 Settings field and `config.toml` value | 2, 14, 15 |
| G2 six operations in the folder | 6, 7 |
| G3 usage description and entitlements | 17, 20 |
| G4 the Daemon runs it outside the sandbox | 1, 5, 9, 18 |
| G5 approval rules in `tools.yaml` and Cedar | 8 |
| G6 `allow_for_chat` except for delete | 8, 10 |
| Where each tool runs, the call step by step | 19 |
| The folder setting (messages, sequence, locks) | 11, 12, 13, 14 |
| Offering the tools (registry, source, platforms) | 9 |
| The tools (checks, results) | 4, 6, 7 |
| Run in the Daemon (scripts, rules 1 to 8) | 5, 6, 7 |
| Approval and policy | 8, 10 |
| Permission from macOS and the signing check | 17, Mac test |
| Data handling (log message without the value) | 3 |
| The test build | 9, 16 |
| Failure behavior and tests | 5, 6, 7, 9, 10, 13, 14, 16 |
| Central docs, ADRs and the changelog | 18, 19, 20, 21 |
| Risks 1, 2, 5 and 10 (a Mac is needed) | Mac test |
| Risk 7 (dangling paths) | The orchestrator tells the owner at gate 2 |
