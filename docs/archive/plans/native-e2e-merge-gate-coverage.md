# Native E2E merge gate coverage

The manual merge gate runs native WebdriverIO tests against Nook's Tauri Panel and
Rust Daemon. Twenty-two scenarios use a deterministic protocol-compatible Worker.
One live scenario uses the configured Python Worker, model endpoint, and web
search. Only that live scenario consumes a model request.

## Deterministic scenarios

The deterministic phase launches the real Tauri application and Daemon with an
isolated PostgreSQL database. A test Worker emits scripted protocol events, so
these cases do not call the model or the public search provider.

| ID | Area | Check | What a failure catches |
| --- | --- | --- | --- |
| D01 | Startup | Delay Daemon startup, verify input and send stay disabled until ping succeeds. | Send-before-ready race or readiness gate regression. |
| D02 | Chat isolation | Start a new chat after a completed or pending interaction. | Old transcript, approval, or request state leaking to a new chat. |
| D03 | History | Open and close History. | Drawer state or event-handler regression. |
| D04 | History | Select a saved test chat and verify its transcript. | Wrong row selection or stale transcript. |
| D05 | History | Delete an unselected test chat. | Deletion affecting another test chat. |
| D06 | History | Delete the selected test chat. | Invalid selection or stale transcript after deletion. |
| D07 | Request lifecycle | Submit one plain prompt and verify its matching terminal response. | Missing reply, wrong message ID, or wrong request routing. |
| D08 | Cancellation | Cancel a streaming response and send a later request. | Cancel not reaching the worker or stale events changing later state. |
| D09 | Single active request | Draft another prompt during streaming, finish the first, then submit the draft. | Accidental concurrent submit, lost draft, or stranded first reply. |
| D10 | Approval | Trigger a protected fetch and inspect the card's chat, message, and call IDs. | Missing, duplicated, or misattached approval. |
| D11 | Approval | Deny a protected action. | Denial failing open or approval state not clearing. |
| D12 | Approval | Allow a protected action once. | Decision not reaching the Worker or action running more than once. |
| D13 | Grant scope | Grant chat-and-host access and retry the same host in the same chat. | Grant not recorded or overbroad approval prompt. |
| D14 | Grant scope | Try the granted host from another chat. | Grant crossing chat boundaries. |
| D15 | Grant scope | Try another host in the granted chat. | Grant crossing host boundaries. |
| D16 | Single active request | Draft a URL during pending approval and verify it remains unsent. | Duplicate request or approval while streaming. |
| D17 | History markup | Inspect selection and deletion controls and count nested buttons. | Invalid nested buttons or controls sharing an action. |
| D18 | Persistence | Reload the Panel and inspect restored test chats. | Lost or cross-database chat state. |
| D19 | Settings isolation | Change settings and inspect the isolated config. | Test settings writing to normal app config or Keychain. |
| D20 | Event correlation | Deliver stale events after cancellation or a newer request. | Old request changing current transcript or streaming state. |
| D21 | Daemon approval | Submit an approval decision with a mismatched chat ID. | Invalid decision consuming another chat's pending approval. |
| D22 | Daemon approval | Force Worker forwarding to fail and retry the decision. | Pending approval disappearing before the Worker accepts it. |

A deterministic test pass proves the Panel-to-Tauri-to-Daemon paths and the
Daemon-to-Worker protocol handling exercised by the fake Worker. It does not
prove model behavior, search-provider availability, or that a third-party page
is reachable.

## Live scenario

The live phase runs once per manually dispatched gate. It uses the real Python
Worker, configured model endpoint, and web search.

| ID | User request | Assertions | Model use |
| --- | --- | --- | --- |
| L01 | Ask for current news headlines in India and sources. | The Worker requests approval, search and fetch complete, the same assistant message reaches `complete`, its answer contains current news text, and its own transcript has source links. | One model conversation, with tool calls inside that conversation. |

The live phase does not repeat the same query, test multiple phrasings, or use the
model to simulate approvals. Deterministic cases cover those state transitions.

## What a green gate means

A green gate means the deterministic scenarios passed on the tested PR commit
and the single live news check returned a completed, sourced answer. It does not
prove that every headline is correct, that every external website will remain
available, or that the app works on Linux or Windows.

The workflow must fail if the PR head changes during the run. A passing result
for an older commit must not satisfy the merge rule for a newer commit.

## Manual trigger and merge rule

A maintainer starts the workflow by entering the PR number in GitHub Actions. The
workflow checks out and tests that PR's exact head SHA. It reports `Native Tauri
E2E Gate` on that SHA after the deterministic and live phases finish.

The `main` branch rule must require that status context. GitHub repository
settings must also prohibit direct pushes and require pull requests. The workflow
file alone cannot enforce those merge rules.

The live endpoint credentials belong in a protected GitHub environment. A
reviewer must approve access to that environment before L01 runs. The workflow
must never print credential values or include them in uploaded artifacts.
