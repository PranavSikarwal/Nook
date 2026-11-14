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
  An unknown text falls back to "Request denied. I stopped here." A Worker test
  reads the two Daemon constants from the Daemon source and checks them against
  the table keys, so a changed Daemon text fails a test. No file in
  `contracts/` changes.
  (b) Add a `reason_code` field to the `authorization_result` and `tool_result`
  messages. The Daemon sets it and the Worker reads it. This changes both
  schemas, their examples, the contract test, the Daemon and the Worker. It
  is a task of its own.
- Recommendation: (a). The change stays small and inside the Worker. The test
  that reads the Daemon constants catches a changed text. That test works only
  in a full checkout. Use (b) only if the owner wants the Worker to hold no
  Daemon text.
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

## G4. Other refusals that keep the wrong notice

- Stage: 3 spec review (found by the round 1 review, finding S2)
- Question: the file and command tools refuse some calls for reasons other than
  a folder rule: `Path must be absolute`, `Path must not contain ..`, `Path is
  not valid`, `Content is larger than the write limit`, and the argument errors
  of the command tool. The user denied none of them, and each ends with
  "Request denied. I stopped here." Does this task fix them?
- Options:
  (a) Out of scope. They keep the current notice. They are mistakes of the
  model, not a rule that the user can change. A later task can handle them,
  with G3 (b), which lets the model correct the call.
  (b) Add them to the table with one notice such as "I stopped here. The model
  sent a path that Nook cannot use." This adds a third message and the exact
  reason list to the Worker.
  (c) Use the generic notice of G1 (c) for these reasons only.
- Recommendation: (a). The user cannot fix these in Settings, so a fixed
  message cannot name a next step. The real fix is to let the model retry,
  which is G3 (b).
- Decision:
- Decided by:
- Date:
- Status: open

## G5. Who updates the row of item 11 in `docs/plans/open-items-fixes.md`

- Stage: 3 spec review (found by the round 2 review, finding S4)
- Question: the row says "Open ... Waits for a decision." After this merge it is
  wrong. `docs/plans/README.md` on the branch of PR #42 also says "The notice for
  a refused call remains open in PR 41", and that line goes stale too. The
  housekeeping task (PR #42) owns `docs/plans/` and changes neither. Who changes
  the row and the README line, and when?
- Options:
  (a) This pull request edits the one table cell, as a plan task. This edits a
  file that the owner gave to the housekeeping task, and the spec changes: the
  task returns to gate 1.
  (b) A later small change edits the cell and the README line after both pull
  requests merge. The owner starts it, or accepts it as a `deferred` gap at
  gate 2.
  (c) The owner asks the housekeeping task to change the row and the README
  line in PR #42, with the commit of this fix as its evidence.
- Recommendation: (b). The owner gave `docs/plans/` to the housekeeping task, so
  this pull request keeps out of it. Option (c) is as good, if PR #42 is still
  open when the owner decides.
- Decision:
- Decided by:
- Date:
- Status: open
