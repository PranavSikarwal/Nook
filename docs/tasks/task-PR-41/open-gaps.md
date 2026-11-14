# Open gaps: PR #41

Each entry records one question and its decision. The format is in
`.agents/rules/task-records.md`, "Open gaps".

## G1. The exact notice for a folder-rule denial

- Stage: 2 spec
- Question: what text does the reply end with when the Daemon refuses a path
  because of a folder rule, and for which rules?
- Options:
  (a) Two fixed messages, one for `Path is outside the allowed folders` and one
  for `The folder is read-only`:
  "I stopped here. That path is outside your allowed folders. To allow it, add
  its folder in Settings, Folders, and ask again." and
  "I stopped here. That folder is read-only. To let me write there, change its
  access in Settings, Folders, and ask again."
  (b) One fixed message, for the outside-folders reason only. The read-only case
  keeps "Request denied. I stopped here."
  (c) One generic notice for every denial that the user did not cause. It quotes
  the Daemon reason: "I stopped here. The request was refused: <reason>."
- Recommendation: (a). Both reasons are folder rules, the user did not deny
  either one, and Settings, Folders fixes both. A fixed text tells the user the
  next step. Option (c) names no next step.
- Decision:
- Decided by:
- Date:
- Status: open

## G2. How the Worker knows that a denial is a folder rule

- Stage: 2 spec
- Question: how does the Worker tell a folder-rule denial from another denial?
- Options:
  (a) The Worker matches the fixed reason text that the Daemon already sends.
  `docs/contracts.md` lists the texts as part of the contract. An unknown text
  falls back to "Request denied. I stopped here." No file in `contracts/`
  changes.
  (b) Add a `reason_code` field to the `authorization_result` and `tool_result`
  messages. The Daemon sets it and the Worker reads it. This changes both
  schemas, their examples, the contract test, the Daemon and the Worker. It
  is a task of its own.
- Recommendation: (a). The change stays small and inside the Worker. The risk of
  a changed text is a fallback to the old notice, and the Daemon tests pin the
  texts. Use (b) only if the owner wants the Worker to hold no Daemon text.
- Decision:
- Decided by:
- Date:
- Status: open

## G3. Does a folder-rule denial still end the reply?

- Stage: 2 spec
- Question: today any denial stops the model at the next chunk. The model cannot
  try another path after a folder-rule refusal. Does this change?
- Options:
  (a) Keep it. The task changes only the notice.
  (b) Let the model continue after a folder-rule refusal, so it can pick an
  allowed path. This changes the rule "A denial ends the reply" in
  `docs/tools-and-authorization.md`, and it needs its own spec.
- Recommendation: (a). Item 11 is about the wrong notice. Option (b) is a new
  behavior and a possible later task.
- Decision:
- Decided by:
- Date:
- Status: open
