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

## G4. A long-lived desktop link or no link

- Stage: 3 spec review (found by round 1, as the "design decision for gate 1")
- Question: How does the Daemon reach the Panel when it needs the Panel to run a tool?
- Options:
  1. A long-lived desktop link. The Panel opens one connection at start and registers its operations. The Daemon offers the tool only while the link is up, and it sends `native_request` on the link. This follows ADR 0008, decision 4, "The Daemon offers the tool only when a Panel reports that it can run it". It needs four new messages, a link lifecycle in the Panel, a handle in `AppState` and a registry swap on each link change.
  2. No link. The Daemon offers the tool whenever it runs on macOS. It sends `native_request` on the event stream of the chat that made the call, and the Panel answers with `native_result` on a new short connection. There is no registration, no lifecycle and no registry swap. The tool is offered to the model even when no Panel is connected (for example with `nookctl`), and the call then fails after a wait. This also changes decision 4 of ADR 0008.
- Recommendation: Option 1. It keeps the catalog honest, so the model never sees a tool that no Panel can run, and it matches the accepted ADR. Option 2 is smaller. Choose it if the owner values less code over an honest catalog.
- Decision:
- Decided by:
- Date:
- Status: open
