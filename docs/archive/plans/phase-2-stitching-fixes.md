# Phase 2 stitching fixes

## Purpose

This plan fixes the bugs where Panel, Daemon, and Worker connect during a
reply. It covers problems 1, 2, 4, 6, 7, 8, and 13 in
`.scratch/phase-2-tools-and-shortcuts/problems.md`.

Cedar and `policy/tools.yaml` wiring (problem 3) stays out of scope. It gets
its own plan.

## Decisions

- The Deny button returns a rejected tool result. The agent continues the
  reply.
- Escape cancels the reply. It sends `cancel` only, not a deny decision.
- Toggle History has no default shortcut. macOS reserves `Cmd+H` for Hide.
- New Chat stays blocked while a reply is active, as the spec says. Opening a
  Chat from History cancels the active reply first.

## Work

### 1. Supervisor keeps the reply listener (problem 1)

`WorkerSupervisor::send_request` must not register a listener for
`ApprovalDecision`. The decision shares the reply's `request_id`, and
registering it replaces the reply's sender.

Test: extend `fake_worker.py` with a run that emits `approval_requested` and
waits for a decision. A supervisor test sends the decision and expects the
reply's `text_delta` and `message_finished` on the original receiver.

### 2. Daemon routes decisions by call id (problem 2)

Add `pending_approvals` to `AppState`, keyed by `call_id`, holding the Chat
id, the Panel request id, and the Worker request id.

1. `handle_send_message` records the entry when `ApprovalRequested` passes
   through.
2. `ApprovalDecision` removes the entry by `call_id`. It checks `chat_id`. It
   replies with an `invalid_request` error for unknown ids or a wrong Chat.
3. `Cancel` and the end of `handle_send_message` remove every entry for that
   reply, so a late decision cannot resume it.

### 3. Worker streams one clean reply (problem 4)

In `RealAgentRunner.run`:

1. Drop chunks whose metadata marks a middleware-internal call
   (`lc_internal_call`) or has `lc_source == "summarization"`.
2. When text resumes from a different model call (a new chunk id), emit a
   paragraph break first.

Test: a fake agent stream with two model calls and one summarization call.

### 4. Panel binds events to their reply (problem 6)

1. Each send gets a stable local message id. The event handler for that send
   edits only the message with that id.
2. `id` stays stable. `message_id` holds the Worker id. The approval card
   matches on `message_id`.
3. `isStreaming` and `activeApproval` change only for the active send.
4. New Chat does nothing while a reply is active, from both the shortcut and
   the button.
5. Opening a Chat from History cancels the active reply first.

### 5. Cancel targets the right request (problem 7)

1. The Tauri `send_message` command returns the Daemon request id.
2. Its cleanup clears `active_request_id` only if the id still matches.
3. The Panel passes the stored id to `cancelMessage`.

### 6. Escape sends one message (problem 8)

Escape during an approval clears the card and sends `cancel` only. The Worker
already cancels pending approvals for a cancelled request.

### 7. Remove the `Cmd+H` default (problem 13)

Toggle History defaults to unbound. Settings shows "Not set" for an unbound
action.

### 8. Documents

1. Update `docs/spec/06-tools-and-shortcuts.md` for the Deny rule, the Escape
   rule, and the Toggle History default.
2. Mark the fixed problems in `problems.md` and record each fix.

## Validation

1. `cargo test` in `daemon/`.
2. `uv run pytest` in `worker/`.
3. `npx tsc -b` and `npm run lint` in `panel/`.
4. `cargo check` in `panel/src-tauri/`.
