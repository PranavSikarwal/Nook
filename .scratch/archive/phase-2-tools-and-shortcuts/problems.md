# Phase 2 problems: approval flow, reply stitching, and history

Status: open
Branch reviewed: `fix/approval-card-and-history-rendering` at `5cd15f3`
Compared against: `docs/spec/06-tools-and-shortcuts.md`, `docs/plans/phase-2-tools-and-shortcuts.md`, and issues 01 to 06

## Summary

Issues 04 and 06 are marked resolved. The code does not deliver what they
describe. Each layer works on its own, but the connections between Panel,
Daemon, and Worker break in several places:

1. After any approval decision, the reply never finishes. The Daemon drops the
   reply's event stream.
2. Cedar and `policy/tools.yaml` are never loaded. The Worker makes every
   authorization decision itself with hardcoded values.
3. The Worker merges every model call in the graph into one text stream. Text
   from before a tool call, text after it, and summarization output run
   together in one message.
4. The Panel applies every incoming event to the last assistant message. It
   ignores which reply or Chat the event belongs to. Replies leak into each
   other and into other Chats.
5. History saves only the merged text. Tool calls, approvals, and cancelled
   state are lost or shown wrongly on reload.

Existing tests pass: 52 Worker tests and 12 Daemon tests. None of them sends an
approval decision through the Daemon, and the Panel has no tests. That is why
these problems did not show up.

All findings come from reading the code. Problem 1 was not reproduced at
runtime. Its cause is direct in the code, so a Daemon test with the fake Worker
should confirm it quickly.

## Severity key

- Blocker: the feature does not work.
- High: wrong behavior that users will hit in normal use.
- Medium: spec gap or wrong behavior in less common paths.
- Low: polish or missing detail.

---

## 1. Approval decision cuts off the running reply

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: blocker

**Spec.** After a grant, the Worker runs the tool and streams the result. After
a denial, the reply ends. Either way the Panel receives `message_finished`.

**Code.** The Daemon forwards the decision with
`supervisor.send_request(DaemonWorkerRequest::ApprovalDecision { request_id: w_id, .. })`
(`daemon/crates/nook-core/src/server.rs:416-427`). `send_request` registers a
new listener for any request that has a `request_id`
(`daemon/crates/nook-core/src/supervisor.rs`, `send_request`,
`inner.active_listeners.insert(id, tx)`). The decision uses the same
`request_id` as the running `run` request. So:

1. The insert replaces the reply's sender in `active_listeners`.
2. The old sender is dropped. `event_rx.recv()` in `handle_send_message`
   returns `None` (`server.rs:630`).
3. `handle_send_message` exits its loop without saving the assistant Message
   and without sending `message_finished`.
4. Every later Worker event for that reply goes to the decision's receiver,
   which the Daemon threw away (`let _ = sup.send_request(...)`).

**What you see.**

- Allow: the tool runs, but the answer never appears. The bubble stays on
  "Thinking...". Input stays blocked because `isStreaming` stays true.
- Deny or Escape: the bubble stays in the `streaming` state forever.
- History shows the user Message with no assistant reply, because
  `save_assistant_message` never runs.

**Fix direction.** Do not register a listener for `ApprovalDecision`. Either
make `request_id` optional in the listener registration for this request type,
or add a separate send method that writes to stdin without touching
`active_listeners`.

## 2. Daemon routes the decision to an arbitrary reply

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: high

**Spec.** The Daemon tracks a pending call by Chat and request, forwards the
decision for that call, and rejects unknown or expired call ids.

**Code.** `server.rs:411-414` picks `open_requests.values().copied().next()`.
That is the first entry of a `HashMap`, in arbitrary order. The handler ignores
`chat_id`, keeps no record of pending call ids, and never replies to the Panel
(`id: _`).

**What you see.** With one reply in flight this works by luck. With two
replies in flight (see problem 6b), the decision can go to the wrong Worker
request. The Worker then drops it because `request_id` does not match
(`worker/src/nook_worker/approval.py:137`). The real approval never resolves
and that reply waits forever.

**Fix direction.** Record `call_id -> (chat_id, panel request id, worker
request id)` when `ApprovalRequested` passes through. Look up the decision by
`call_id`, check `chat_id`, remove the entry, and send an error back for
unknown ids.

## 3. Cedar and `tools.yaml` are not wired in

Severity: high

**Spec.** Every tool request reaches Cedar. The Daemon records grants. Grants
bind to Chat id plus tool plus argument digest, or Chat id plus host. A bad
Cedar policy or schema fails Daemon startup. The Panel builds approval buttons
from `policy/tools.yaml`.

**Code.**

- `CedarAuthorizer` in `daemon/crates/nook-core/src/policy.rs` is used only by
  its own unit tests. Nothing in `server.rs`, `nookd/src/main.rs`, or
  `config.rs` loads it. Startup never reads the policy files.
- No code reads `policy/tools.yaml`.
- The Worker decides on its own in `_create_registry_runner` and
  `_check_tool_approval` (`worker/src/nook_worker/agent.py:118-160`). Tiers and
  descriptions are hardcoded in `tool_specs` (`agent.py:176-195`). Actions are
  hardcoded as `["allow_once", "allow_for_chat_host", "deny"]`
  (`agent.py:136`).
- `nook:web_search` has tier `allow` and runs with no check at all.
- Grants live in Worker memory (`approval.py:45`), not in the Daemon. The
  Daemon restarts the Worker on every settings save (`server.rs:223-240`), so
  saving settings wipes all host grants. A crash wipes them too.
- No exact-call grant exists. `allow_once` stores nothing, which works by
  accident. The argument digest in `ScopedGrant` is never computed.
- When the host cache is full, `self._approved_hosts.pop()` removes an
  arbitrary entry (`approval.py:82`).
- Fetch limits are duplicated as constants in
  `worker/src/nook_worker/tools/fetch.py:14-18`. They do not come from YAML.

**What you see.** Approval appears to work for `web_fetch`, but the documented
authorization model is not in effect. Editing `tools.yaml` or the Cedar files
changes nothing.

**Fix direction.** Load Cedar and YAML in the Daemon at startup and fail
closed on errors. Add a Worker-to-Daemon authorization request before each
executor runs. Move grant storage to the Daemon. Send YAML actions in
`approval_requested`.

## 4. Worker merges all model output into one message

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: high

This is the main text stitching problem.

**Code.** `RealAgentRunner.run` (`agent.py:269-280`) streams with
`stream_mode="messages"` and turns every `AIMessageChunk` into a `text_delta`
for one `message_id`. It ignores `_metadata`, so it cannot tell which model
call or graph node produced the chunk.

**What you see.**

a. **Pre-tool and post-tool text run together.** The model often writes a
   short line before calling a tool, such as "Let me fetch that page." After
   the tool returns, it writes the answer. Both go into the same string with
   no separator: `Let me fetch that page.Here is a summary...`. Markdown
   headings or lists at the start of the second part do not render because
   they are not at the start of a line.

b. **Summarization output leaks into the reply.** `SummarizationMiddleware`
   calls the model inside the graph with metadata
   `lc_source: "summarization"` and no `nostream` tag
   (`langchain/agents/middleware/summarization.py:886-888`). When a Chat passes
   `summarize_at_tokens`, the summary text streams into the visible reply and
   is saved to history.

c. **The Daemon saves the merged text.** `full_text.push_str(&delta)`
   (`server.rs:644`) stores exactly what the Worker streamed. History reloads
   show the same merged text.

**Fix direction.** Filter chunks by metadata: keep only the main agent model
node and drop chunks with `lc_source == "summarization"`. Insert a paragraph
break, or start a new text segment, when a tool call separates two model
turns. Problem 5 covers the related tool events.

## 5. Tool call events are never sent or shown

Severity: medium

**Spec.** Phase 2 adds `tool_call_started` and `tool_call_finished` for
executed tools.

**Code.** The Worker defines `ToolCallStartedEvent` and
`ToolCallFinishedEvent` (`worker/src/nook_worker/protocol.py:130-148`) but
never emits them. The Daemon forwards them (`server.rs:655-685`). The Panel
declares them in `types.ts:77-78` but `handleDaemonEvent`
(`panel/src/App.tsx:181-273`) has no branch for them. The database saves only
text, so tool calls and approvals are not in history.

**What you see.** No sign that a search or fetch happened, apart from the
approval card for `web_fetch`. After reload, nothing shows which tools ran or
what the user approved.

## 6. Panel applies events to the wrong message or Chat

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: high

**Code.** Every branch in `handleDaemonEvent` (`App.tsx:181-273`) edits
`prev[prev.length - 1]` when it is an assistant message. No branch checks
`event.chat_id`, `event.id`, or `event.message_id` against the current Chat or
the message it is editing.

**What you see.**

a. **Stop, then send again.** `handleStop` sets `isStreaming` to false before
   the Daemon confirms (`App.tsx:114-121`). The user can send a new Message at
   once. Late deltas from the old reply append to the new bubble. The old
   reply's `message_finished` marks the new bubble complete while it is still
   streaming.

b. **New Chat while streaming.** The New Chat shortcut is guarded
   (`App.tsx:63`), but the New Chat button calls `handleNewChat` with no guard
   (`App.tsx:123-131`, passed to `InputBar` at `App.tsx:566` and `App.tsx:649`).
   It clears messages and sets `isStreaming` to false without cancelling the
   reply. The old reply's deltas then create an assistant bubble in the new
   Chat. The old reply's text is saved to the old Chat, so the two views
   disagree.

c. **Open a Chat from History while streaming.** `handleSelectChat`
   (`App.tsx:519-537`) loads another Chat and leaves the reply running. Its
   deltas append to the loaded Chat's last assistant message.

**Fix direction.** Store the active reply's `id` and `chat_id`. Drop events
that do not match. Find the target message by `message_id` instead of taking
the last one. Cancel or block the active reply before switching Chats.

## 7. Stop and Escape lose track of the active request

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: high

**Code.** The Tauri state keeps one `active_request_id`
(`panel/src-tauri/src/lib.rs:19`). `send_message` overwrites it
(`lib.rs:558-563`). When an older stream ends, its cleanup task sets it to
`None` (`lib.rs:589-597`), even if a newer request had replaced it.
`cancel_message` then finds `None` and returns without sending anything
(`lib.rs:665-668`). The Panel calls `cancelMessage()` with no target
(`App.tsx:117`).

**What you see.** After the sequence in problem 6a, Stop and Escape do nothing
for the newer reply.

**Fix direction.** Return the request id from `send_message` to the Panel and
pass it to `cancelMessage(targetId)`. Clear the stored id only if it still
equals the id that finished.

## 8. Escape during approval races two messages

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: high

**Spec.** Escape denies the pending request and ends the reply as
`cancelled`. A later decision cannot resume it.

**Code.** Escape calls `handleApprovalDecide('deny')` and `handleStop()` in
the same tick (`App.tsx:300-305` and `App.tsx:74-79`). Each opens its own
socket and sends without waiting for the other. Arrival order at the Worker is
not fixed.

- If deny arrives first, the tool returns an error string
  (`agent.py:138-139`). The agent keeps generating until the cancel arrives.
  Some of that text can stream before the cancel lands.
- With problem 1 in place, the deny also cuts off the reply's event stream, so
  the Panel never sees `message_finished`.

**Spec conflict.** The spec disagrees with itself here. Flow step 9 says "A
denial resumes the agent with a tool-rejected result." The failure table says
"User denies or presses Escape: The Worker ends the reply as `cancelled`."
The current code follows step 9 for the Deny button. You need to choose one
rule before the fix.

**Fix direction.** Send one message for Escape, either `cancel` alone or a
decision that the Worker treats as cancel. Let the Worker cancel any pending
approval for that request (it already does in `cancel_for_request`).

## 9. Cancelled replies show as complete

Severity: medium

**Spec.** On cancel, the Panel marks the assistant Message as interrupted.

**Code.** `App.tsx:240` maps every status other than `error` to `complete`.
`TranscriptView` (`panel/src/components/TranscriptView.tsx`) has no rendering
for `cancelled`. An empty cancelled reply shows "(Model produced no text
response)" (`TranscriptView.tsx:62-66`).

**What you see.** A stopped reply looks like a finished one, or like a model
failure.

## 10. History reload renders incomplete or wrong data

Severity: medium

**Code.** `handleSelectChat` (`App.tsx:519-537`) maps rows through `any`,
defaults a missing `role` to `assistant`, and drops `message_id`.

**What you see.**

- Merged text from problem 4 is what reloads.
- Replies cut off by problem 1 are missing entirely.
- Cancelled replies look complete (problem 9).
- No tool or approval record (problem 5).
- Attachments added through the file picker are sent as base64, so the user
  bubble has no `path`. Retry filters on `path` (`App.tsx:436-442`) and drops
  those attachments silently.
- The user bubble labels every non-image attachment as `text`, including PDFs
  (`App.tsx:367`).

## 11. Approval card misses required fields

Severity: medium

**Spec.** The card shows tool name, purpose, arguments, URL or host, reason
for the prompt, and the scope of every action.

**Code.** `ApprovalCard.tsx` shows tool name, `explanation`, and
`resource_summary`. It does not show arguments or the scope of each action.
The explanation is always "Web fetch requires approval to access ..."
(`agent.py:134`), whatever the tool.

## 12. Pending approvals never expire

Severity: medium

**Spec.** The failure table covers "Approval call id unknown or expired".

**Code.** The Worker waits on the future with no timeout (`approval.py:126`).
The Daemon has no expiry either. If the Panel window closes or the decision is
lost, the reply waits until the Worker restarts.

## 13. Shortcut defaults and validation

Status: resolved in docs/plans/phase-2-stitching-fixes.md
Severity: low

- `Cmd+H` is the default for Toggle History (`panel/src/lib/shortcuts.ts:42`).
  macOS reserves `Cmd+H` for Hide. The spec lists it as the default and also
  says to reject reserved combinations. That is a second spec conflict to
  resolve.
- No reserved-combination check exists. `SettingsModal.tsx:190-192` checks
  duplicates only.
- `Escape` is the default for both `cancel_active_work` and
  `close_auxiliary_view`. The duplicate check would reject this pair if a user
  recorded it.
- The recorder detects itself by matching button text "Press key combo"
  (`App.tsx:295`). Any text change breaks shortcut suppression during
  recording.

## 14. Web search is missing the region argument

Severity: low

**Spec.** `nook:web_search` accepts query, region, and result limit.

**Code.** `WebSearchInput` has `query` and `max_results` only
(`worker/src/nook_worker/tools/search.py:10-14`).

## 15. Tests do not cover the connected flow

Severity: high

Issue 06 asks for integration tests for grants, denial, Escape cancellation,
and a new Message after cancellation, plus Panel tests for shortcuts and
approval cards.

- The Daemon tests have no approval case. `fake_worker.py` does not emit
  `approval_requested`.
- The Panel has no test files and no `test` script in `package.json`.
- Worker tests check `ApprovalManager` alone, not through the Daemon.

A single Daemon test with the fake Worker that sends `approval_requested`,
then an `approval_decision`, then expects `message_finished` would have caught
problem 1.

---

## Decisions needed from you

1. Deny button: should it end the reply as `cancelled`, or return a rejected
   tool result and let the agent continue? (Problem 8.)
2. Toggle History default: keep `Cmd+H` or pick a combination macOS does not
   reserve? (Problem 13.)
3. Scope of the fix: wire Cedar and YAML now (problem 3), or first fix the
   stream and stitching bugs (problems 1, 2, 4, 6, 7, 8) and track Cedar as a
   separate issue?

## Suggested fix order

1. Problem 1, with a Daemon test. Nothing else can be checked until replies
   finish after a decision.
2. Problems 2, 7, and 8, so decisions and cancels reach the right request.
3. Problem 6, so the Panel stops mixing replies.
4. Problem 4, so text stops merging.
5. Problems 9, 10, 5, and 11 for rendering and history.
6. Problem 3, Cedar and YAML wiring.
7. Problems 12 to 14.
