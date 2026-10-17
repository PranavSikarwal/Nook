# App integrations: Notes and Google Docs

Status: proposed. Waiting for a decision on the Notes route.

## Why

The owner wants the assistant to write to the Notes app, to files, and to online
documents such as Google Docs. File writes exist (TP9). Notes and Google Docs do
not.

## What the Notes trial found

Tested on macOS 27.0.1 on October 17, 2026.

1. `osascript` runs inside the command sandbox. A script that does not talk to
   another app works.
2. A script that talks to Notes fails inside the sandbox. With the deny-by-default
   profile, `osascript` cannot find the app. With a profile that adds the Apple
   event rules for Notes, it reaches the app and fails with error -10004 (a
   privilege violation).
3. The same error appears with a profile that allows everything
   (`(allow default)`). The same script works outside `sandbox-exec`. macOS
   refuses Apple events from any process that runs under a Seatbelt profile.
4. So an MCP server that drives Notes through AppleScript cannot work in the
   Nook sandbox. No change to the profile fixes it.

Google Docs was not tested. A Google Docs MCP server needs network access, a
folder for its token, and a Google Cloud OAuth client that only the owner can
create.

## Options for Notes

1. **Run the Notes server outside the sandbox.** Add a per-server setting such as
   `sandbox = false`. The server then has the full access of the user. A server
   from the internet gets that access. Cedar and the approval prompt still ask
   before each call.
2. **A Notes tool that the Panel runs.** The Panel is a desktop app and can send
   Apple events when the user grants Automation permission to Nook. The Daemon
   authorizes the call as for any tool, and the Panel runs a fixed script with
   the text passed as data and not as script code. This is the executor design
   in `docs/design/native-system-tools.md`, with a Notes tool instead of the
   clipboard tools.
3. **Do not support Notes.** Files and Google Docs stay.

Recommendation: option 2. The code that touches Notes is Nook's own, short, and
fixed. Option 1 trusts third-party code with the whole user account.

## Google Docs

Google Docs works through an MCP server with `network = true` and a write path
for its token. The owner creates the OAuth client and signs in once. The plan
adds a sample `config.toml` block and a fake MCP server test, because a real
server cannot run in CI. It needs no code change in Nook.

## Not in scope

Computer use (screen capture, mouse, keyboard). It needs its own design.

## Decisions needed

1. Which Notes option?
2. Should the plan write the Google Docs sample and test now?
