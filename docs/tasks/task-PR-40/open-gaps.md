# Open gaps

## G1. A fixed Nook folder or a folder set in config.toml

- Stage: 2 spec
- Question: Which Notes folder gets the notes that the tool creates?
- Options:
  1. A fixed folder named `Nook` in the default account. The user can move a note later.
  2. A folder name set in `config.toml` (for example `[tools.notes] folder`). The Daemon must check the name and pass it to the Panel, and Cedar must check it.
  3. The default folder of Notes. The tool needs no folder logic, but the assistant then writes into the main list of the user.
- Recommendation: Option 1. One fixed folder keeps the Cedar rule, the approval text and the script simple, and it keeps the assistant's notes apart from the notes of the user. A setting can come later if a user needs it.
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
