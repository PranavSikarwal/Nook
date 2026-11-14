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
