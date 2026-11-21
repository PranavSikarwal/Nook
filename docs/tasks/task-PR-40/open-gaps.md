# Open gaps

## G1. A fixed Nook folder or a folder set in config.toml

- Stage: 2 spec
- Question: Which Notes folder gets the notes that the tool creates?
- Options:
  1. A fixed folder named `Nook` in the default account. The user can move a note later.
  2. A folder name set in `config.toml` (for example `[tools.notes] folder`). The Daemon must check the name and pass it to the provider, and Cedar must check it.
  3. The default folder of Notes. The tool needs no folder logic, but the assistant then writes into the main list of the user.
  4. A folder name that the user types in a field of the Settings tab. The Daemon checks it, writes it to `config.toml` as `[tools.notes] folder`, and rebuilds the tools. It needs three new messages in `contracts/` and a small Panel field. This is option 2 with a Settings field, and the Panel only shows the value.
- Recommendation: Option 1. One fixed folder keeps the Cedar rule, the approval text and the script simple, and it keeps the assistant's notes apart from the notes of the user. A setting can come later if a user needs it. Under option 1 the Cedar condition `folder == "Nook"` can never fail, because the Daemon sets `folder` to a constant. It stays as a guard that denies if the value is ever wrong. Under option 2 it becomes a real check.
- Decision: Option 4. The user chooses the folder in a Settings field, and the default is `Nook`.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

The owner's words: "yes a field in settings tab is recomended" and "works keep following this".

## G2. Append to an existing note

- Stage: 2 spec
- Question: Does the tool also append text to an existing note?
- Options:
  1. No append. The tool only creates a new note.
  2. Append to a note that the tool created. The Daemon must store the note id, and the user can edit or delete the note in Notes, so the id can go stale.
  3. Append to any note. The tool must find notes, which means it reads private data. This is out of scope of the design.
  4. List, read, create, append, edit and delete notes, only inside the folder of G1. The model must list and read a note to find the one to change, so reading is in scope, but only in the chosen folder. Every call asks. Delete offers no chat grant.
- Recommendation: Option 1. Appending needs a way to find the note, and the first version has none. A later design can add it with its own review.
- Decision: Option 4. All six operations in the chosen folder, all in this pull request.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

The owner's words: "allow append edit delete also inside it's folder!", "yes but after adding append or maybe add edit write delete these things into the folder which user can select themselves", and, for the size of the pull request, "works". The question offered two options (all in this pull request, or a split), so the orchestrator took "works" to mean all in this pull request. The owner can correct this at gate 1.

## G3. Automation entitlement in the release signing setup

- Stage: 2 spec
- Question: Does the release setup get the Automation entitlement and the usage description?
- Options:
  1. Check the setup when the work starts, write the result into the spec, and add the usage description (`NSAppleEventsUsageDescription`) and an entitlements file with `com.apple.security.automation.apple-events`. Keep the ad hoc signature and leave the hardened runtime off.
  2. Add the usage description only. The entitlement waits until a hardened runtime build exists.
  3. Wait for a Developer ID signed and notarized release, and add both then.
- Recommendation: Option 1. The check is done and is in the "Signing check" section of the spec: the builds are ad hoc signed, have no entitlements file and no usage description. The usage description is required now, or macOS shows no prompt. The entitlements file costs nothing and a later hardened runtime build then works.
- Decision: Option 1.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

The owner's word: "works".

## G4. Where the Notes operation runs

- Stage: 3 spec review (found by round 1, as the "design decision for gate 1"), decided by the owner during gate 1
- Question: Which process runs `osascript` for the Notes tool, and how does the call get there?
- Options:
  1. The Panel runs it, and a long-lived desktop link carries the call. The Panel opens one connection at start and registers its operations. The Daemon offers the tool only while the link is up, and it sends `native_request` on the link. It needs four new messages, a link lifecycle in the Panel, a `DesktopLink` handle in `AppState`, and a registry swap on each link change. Spec round 1 to 4 reviewed this option.
  2. The Panel runs it, with no link. The Daemon sends `native_request` on the event stream of the chat that made the call, and the Panel answers on a new short connection. Two messages are new. The reviewer of round 2 recommended this option.
  3. The Daemon runs it. `NotesProvider` starts `/usr/bin/osascript` as a plain child process, outside the command sandbox, with one fixed script and one HTML argument. No message changes, the Panel code does not change, and there is no link. The tool runs outside the sandbox, because macOS refuses Apple events from a sandboxed process. The file tools also run in the Daemon with no OS sandbox, so the Notes tool is not the first tool outside it. The cost is a prompt whose app name macOS decides (Risk 1 of the spec), and a Mac test before merge.
- Recommendation: Option 3, after the owner raised the principle that the Daemon is the core and the Panel is only the view. It removes the most code and the most new surface. Options 1 and 2 stay as the fallback if the manual test shows that macOS cannot attribute the prompt to Nook.
- Decision: Option 3. The Daemon runs the tool outside the sandbox.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

The owner's words: "I want deamon to be core system controlling everything panel is just the view" and "let it run outside sandbox the notes thing due to apple event". The owner also said that the Daemon is always started by the Panel, so the case of a Daemon started by hand is out of scope, and that the owner will run the real Mac test.

## G5. Where the approval rules live

- Stage: 3 spec review (the owner asked in gate 1)
- Question: Are the approval rules of the tools kept in Rust code or in config, and can the user change them?
- Options:
  1. Keep the current setup for all tools. The tier and the approval actions are in `policy/tools.yaml`, the decisions are in `policy/cedar/`, and both ship in the app and load at start. The user can make a tool stricter in the Permissions tab, and never looser. Only the list of action ids (`APPROVAL_ACTIONS`) and the rule that an `ask` tool lists an action are in Rust.
  2. Add a layer that the user edits in `config.toml` for every tool, with a rule that it may only tighten an approval. This is a platform change for all tools and needs its own task.
- Recommendation: Option 1 for this task. It already keeps the approval of each Notes tool in config, and a user layer needs its own design and review.
- Decision: Option 1. The Notes tools follow the current setup. The owner may ask for option 2 as a later task.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

The owner's words: "isn't the recommended approval like delete is approval required is it maintained in config or in code directly i want them to be maintained in some config or so?", "not only for this for all the tools", and "works keep following this".

## G6. Which Notes tools offer "allow for this chat"

- Stage: 3 spec review (found by round 11, as the open part of S30)
- Question: Does each Notes tool offer the action `allow_for_chat`, which lets the model call the tool for the rest of the chat with no new approval?
- Options:
  1. Every tool except delete offers `allow_once`, `allow_for_chat` and `deny`. Delete offers `allow_once` and `deny` only. The model can then change and replace notes in the folder for the rest of a chat after one click, and a steered model could do harm with a chat grant for edit.
  2. List and read offer `allow_for_chat`. Create, append, edit and delete offer `allow_once` and `deny` only. Every change asks.
  3. Only list and read and create offer `allow_for_chat`. Append, edit and delete ask every time.
- Recommendation: Option 1, because it is the least friction and the approval card shows the folder and the note title. Option 3 is the safer middle choice if the owner worries about a chat grant for edit. The choice is a change in `tools.yaml` and Cedar only, with no Rust change (gap G5).
- Decision: Option 1.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

The owner's words, to the proposal "I recommend that delete asks every time (allow once or deny only), and the other operations also offer allow for this chat": "works". The owner can change this at gate 1.

## G7. Test files that build `ToolsConfig` by hand (found in stage 6, task 2)

- Stage: 6 implement
- Question: Task 2 adds a required field `notes` to `ToolsConfig`. A hand-built `ToolsConfig` in a test fails to compile (found: `daemon/crates/nook-core/tests/terminal_tool_tests.rs:43`). The plan names only `config.rs`. Does task 2 also change those initializers?
- Options:
  1. Task 2 also adds `notes: Default::default()` (or the value the test needs) to each hand-built `ToolsConfig` that stops compiling. No behavior changes.
  2. Give `ToolsConfig` a way to build without the field, such as `..Default::default()` in the library. This changes the public type more than the spec says.
- Recommendation: Option 1. The design already adds the field, and the fix is a mechanical edit of test initializers.
- Decision: Option 1, closed by recommendation under stage 6 rule 1 (the choice touches only test files, no contract, policy, public interface, authorization, sandbox or dependency). The owner reviews it at gate 2.
- Decided by: orchestrator
- Date: 2026-11-14
- Status: closed by recommendation

## G8. Two MCP tests fail in CI on the new runner image (found in review round 18, C14)

- Stage: 7 review
- Question: The check `Code Quality and Tests` fails on this branch since 2026-11-21, in two tests of `daemon/crates/nook-core/tests/mcp_tool_tests.rs` (`a_stopped_server_shows_as_stopped_until_a_call_starts_it`, `sources_list_each_server_as_ready_or_off_with_a_reason`). They fail with "one server runs" and "`Option::unwrap()` on a `None` value", which means the fake MCP server did not start inside the sandbox. This PR changes no MCP code and no MCP test. The same two tests passed on this branch on 2026-11-15 (runner image `20261111.196.1`), and all 24 MCP tests pass on the orchestrator's machine and on the reviewer's. They fail twice on this PR (runs 44884114949 and 44885309975, image `20261116.202.1`), and the branch `feat/google-docs-mcp` fails 13 MCP tests on the same image. The CI rule says that a failing check is always fixed, never deferred, and that a second failure is a finding. The cause is not visible in the CI log, because the test output does not print the start error. How is the check fixed?
- Options:
  1. A separate small task and pull request on `main` (it also unblocks `feat/google-docs-mcp`): print the start error in the two tests, find the cause on the new runner image, fix it, and then merge `main` into this branch. This pull request waits for it.
  2. Fix it inside this pull request: change `mcp_tool_tests.rs` (and `providers/mcp.rs` if the cause is there) in a new plan task. This widens the pull request beyond Notes.
  3. Wait and rerun the job later, in case the runner image changes back. Nothing is changed.
- Recommendation: Option 1. The fault is in the CI environment and in code that this task does not own, it blocks a second pull request, and a separate fix keeps the Notes diff clean. Option 3 does not fix a failing check, so it does not satisfy the review standard.
- Decision: Option 2, fix inside this pull request (owner answer to the question "How should the check be fixed?" on 2026-11-21: "Fix inside PR #40"). Finding of the orchestrator: the CI log shows the test binary at `target/debug/build/nook-core/<hash>/out/mcp_tool_tests-<hash>`. The helper `example_binary()` in the test reads `current_exe().parent().parent()/examples`, which is right for `target/debug/deps/` and wrong for that deeper folder, so the fake server is not found and `McpProvider::start` returns `None`. Most tests then return early without failing (`let Some(h) = ... else { return; }`, 24 tests in 0.19 s), and only the two tests that unwrap the result fail. The fix is in the test helper only (plan task R15).
- Decided by: owner
- Date: 2026-11-21
- Status: decided by owner
