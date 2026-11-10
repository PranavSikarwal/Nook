# The Panel runs desktop-app tools and the Daemon authorizes them

## Status

Proposed

## Date

October 17, 2026

## Context

ADR 0007 makes the Daemon run privileged tools inside a native sandbox. A tool
that controls another desktop app cannot follow that rule. macOS refuses Apple
events from any process that runs under a Seatbelt profile, even a profile that
allows everything. The Daemon has no window and no desktop session. The Panel is
a normal desktop app and can send Apple events after the user allows it.

The first such tool writes a new note to the Notes app. See
`docs/design/desktop-app-tools.md`.

## Decision

1. Authorization stays in the Daemon. The Worker sends `execute_tool`. The
   Daemon checks the arguments, asks Cedar, and shows the approval. ADR 0004 still
   holds.
2. After an allow, the Daemon sends the Panel a `native_request` with a one-time
   nonce. The Panel runs one operation from a fixed list in Rust and answers with
   `native_result`. The web view cannot call the operation or read the nonce.
3. Each operation is a fixed script or call that Nook ships. The model supplies
   data, never code.
4. The Daemon offers the tool only when a Panel reports that it can run it.

## Alternatives considered

### Run the app server outside the sandbox

A `sandbox = false` setting would let a third-party server run with the full
access of the user. Cedar and the approval would still ask, but the code that
runs after the allow would be code from the internet.

### Give the Daemon desktop permissions

The Daemon is a background process with no window. macOS would ask for
Automation permission for the Daemon binary, and the user could not tell why.

## Consequences

- The trust boundary now includes the Panel. A flaw in the web view must not reach
  the operation. The nonce check and the fixed operation list limit this.
- The Panel and the Daemon share a new message pair, which needs schemas, examples,
  and tests.
- Every new operation needs its own design, because each one grants access to
  another app.
- Platforms without the app do not offer the tool.
