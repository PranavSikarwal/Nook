# Open gaps

## G1. A fixed Nook folder or a folder set in config.toml

- Stage: 2 spec
- Question: Which Notes folder gets the notes that the tool creates?
- Options:
  1. A fixed folder named `Nook` in the default account. The user can move a note later.
  2. A folder name set in `config.toml` (for example `[tools.notes] folder`). The Daemon must check the name and pass it to the Panel, and Cedar must check it.
  3. The default folder of Notes. The tool needs no folder logic, but the assistant then writes into the main list of the user.
- Recommendation: Option 1. One fixed folder keeps the Cedar rule, the approval text and the script simple, and it keeps the assistant's notes apart from the notes of the user. A setting can come later if a user needs it. Under option 1 the Cedar condition `folder == "Nook"` can never fail, because the Daemon sets `folder` to a constant. It stays as a guard that denies if the value is ever wrong. Under option 2 it becomes a real check.
- Decision:
- Decided by:
- Date:
- Status: open

## G2. Append to an existing note

- Stage: 2 spec
- Question: Does the tool also append text to an existing note?
- Options:
  1. No append. The tool only creates a new note.
  2. Append to a note that the tool created. The Daemon or the Panel must store the note id, and the user can edit or delete the note in Notes, so the id can go stale.
  3. Append to any note. The tool must find notes, which means it reads private data. This is out of scope of the design.
- Recommendation: Option 1. Appending needs a way to find the note, and the first version has none. A later design can add it with its own review.
- Decision:
- Decided by:
- Date:
- Status: open

## G3. Automation entitlement in the release signing setup

- Stage: 2 spec
- Question: Does the release setup get the Automation entitlement and the usage description?
- Options:
  1. Check the setup when the work starts, write the result into the spec, and add the usage description (`NSAppleEventsUsageDescription`) and an entitlements file with `com.apple.security.automation.apple-events`. Keep the ad hoc signature and leave the hardened runtime off.
  2. Add the usage description only. The entitlement waits until a hardened runtime build exists.
  3. Wait for a Developer ID signed and notarized release, and add both then.
- Recommendation: Option 1. The check is done and is in the "Signing check" section of the spec: the builds are ad hoc signed, have no entitlements file and no usage description. The usage description is required now, or macOS shows no prompt. The entitlements file costs nothing and a later hardened runtime build then works.
- Decision:
- Decided by:
- Date:
- Status: open

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
