# The Daemon runs desktop-app tools outside the sandbox

## Status

Accepted

## Date

November 14, 2026

## Context

ADR 0007 makes the Daemon run privileged tools inside a native sandbox. A tool
that controls another desktop app cannot follow that rule. macOS refuses Apple
events from any process that runs under a Seatbelt profile, even a profile that
allows everything.

The first such tools work with Apple Notes (create, list, read, append, edit,
and delete notes) inside one chosen folder.

## Decision

1. The Daemon runs desktop-app tools itself, outside the OS sandbox, as a plain
   child process. For Notes tools on macOS, it runs `/usr/bin/osascript`.
2. Authorization stays in the Daemon. The Worker sends `execute_tool`. The
   Daemon checks the arguments, asks Cedar, and shows the approval card.
   ADR 0004 still holds. The Panel only renders the approval card and the
   Settings field for the folder.
3. Each operation is a fixed script that Nook ships. The model supplies data,
   never code or command-line options. The Daemon prefixes every data argument
   with `:` so `osascript` cannot read a value as an option.
4. The Daemon runs `/usr/bin/osascript` with its environment set to
   `LANG=en_US.UTF-8` only, working folder `/`, no standard input, a 60-second
   timeout, its own process group, and one run at a time under an internal lock.
5. The tools run on macOS only. On Linux and Windows, the tools are not offered.

## Alternatives considered

### Run desktop-app tools in the Panel

The earlier design had the Daemon authorize the tool and send a request to the
Panel to run the AppleScript, because the Panel is a desktop app with a window.
This adds IPC messages, moves tool execution into the UI process, and splits the
tool path across two processes. The Daemon is the core of Nook and runs the
operations itself.

### Run third-party servers outside the sandbox

A `sandbox = false` setting would let third-party tools run without isolation.
Running outside the sandbox is permitted only for fixed, first-party tools that
Nook ships and verifies.

## Consequences

- The Daemon runs `/usr/bin/osascript` outside the sandbox for these tools.
  Strict argument checks, `:` argument prefixes, constant scripts, and Cedar
  approvals limit the attack surface.
- The original reason against running in the Daemon was that the Daemon is a
  background process without a window, so the macOS Automation prompt could name
  the Daemon binary (`nookd`) instead of the app, or show no prompt. The user
  must approve Automation permissions in macOS on the first call.
- If the user denies Automation permission, macOS refuses subsequent calls with
  `Error: macOS did not allow Nook to control Notes` until changed in System
  Settings.
- The tools exist on macOS only. On Linux and Windows, the tools are not offered.
