# Add approval pause, decision, and cancellation flow

Type: task
Status: open
Blocked by: 01, 03

## Goal

Pause an agent tool call, show a Panel approval card, and resume or cancel the
active reply safely.

## Work

- Configure Deep Agents tool interruption.
- Forward pending approvals through the Daemon.
- Render metadata-defined actions in the Transcript.
- Record configured grants and submit decisions.
- Reject pending approval on Escape.
- Clear pending-call state before the user can send another Message.

## Done when

Allowing a call resumes the same reply. Denying or pressing Escape ends the
reply as cancelled. A later decision cannot resume the cancelled reply.
