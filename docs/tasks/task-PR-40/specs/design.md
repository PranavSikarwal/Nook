# Spec: Notes tool run by the Panel

```
Stage: 3 spec review
Gate 1: pending
Gate 2: pending
```

Branch: `feat/notes-tool`. Pull request: #40.

This spec starts from the draft `docs/design/desktop-app-tools.md` of October 17,
2026. The draft moved here with this task. The owner chose this route on October
17, 2026 (see `docs/plans/app-integrations.md`, option 2). ADR 0008 records the
decision, and this task changes its status to Accepted.

## Why

The owner wants the assistant to write to the Notes app. A Notes server from the
internet cannot do this inside the command sandbox, because macOS refuses Apple
events (the messages that one Mac app sends to another) from any sandboxed
process. The Panel is a normal desktop app, so it can send Apple events when the
user allows it. The Panel runs the tool, and the Daemon authorizes each call.

The draft before this one had clipboard, notification and open-link tools. The
owner removed them because the user can do each of them without the assistant.

## Scope

In scope: one tool, `nook:notes_create_note`. It creates a new note in a folder
named `Nook` in the default Notes account. It works on macOS only.

Not in scope: reading, searching, editing, moving or deleting notes, other
folders, other accounts, Reminders, Calendar, Mail, Messages, the clipboard,
screen capture and any other app. Each of those needs its own design, because
reading a note shows private data and a delete cannot be undone.

This is not a release. No version file changes. The change adds one line under
"Unreleased" in `CHANGELOG.md`.

## Design

### Platforms

| Platform | Support | Reason |
| --- | --- | --- |
| macOS | Yes | Notes and Apple events exist. |
| Linux | No | No Notes app. The tool is not offered. |
| Windows | No | No Notes app. The tool is not offered. |

### What the code does today, and what changes

Two facts about the current code shape this design.

1. Each Panel request uses its own short connection to the Daemon. `ask_daemon`
   in `panel/src-tauri/src/lib.rs` opens a connection, sends one message and reads
   the reply. `send_message` keeps one connection for the events of one reply.
   The Daemon cannot send a message to the Panel at any other time. The draft
   said "the Daemon sends the Panel a `native_request`", but no channel for that
   exists.
2. The Daemon builds its tool list from a `ProviderRegistry` and sends it to the
   Worker as `tool_catalog`. A folder change already builds the registry again
   and sends a new catalog. The same path can add or remove the Notes tool.

So the design adds one long-lived connection, the **desktop link**, and one
provider that uses it.

### The desktop link

The Panel opens the desktop link when it starts. It is a normal connection to the
Daemon socket that stays open. The Rust side of the Panel owns it. The web view
never sees it. The Panel has no reconnect code today, so the section "Link
lifecycle" below defines it.

Four new messages in `contracts/panel-daemon.schema.json`:

1. `register_desktop` (Panel to Daemon) carries `id` and `operations`, a list of
   operation names that this Panel can run. On macOS the list is
   `["notes_create_note"]`. On Linux and Windows the Panel sends no
   registration. The Daemon ignores an operation name that it does not know.
2. `desktop_registered` (Daemon to Panel) carries `id`. It is the answer to a
   `register_desktop` that the Daemon accepted. The connection stays open.
3. `native_request` (Daemon to Panel) carries `call_id`, `operation`,
   `arguments` (an object) and `nonce` (a random string). The Daemon sends it
   only on the desktop link.
4. `native_result` (Panel to Daemon) carries `id`, `call_id`, `nonce`, `status`
   (`ok` or `error`) and `output` (a string of at most 1000 characters). The
   Daemon sends no reply to it.

Only one desktop link is active, and the first live link wins. A second
`register_desktop` gets an `error` reply (code `invalid_request`, message `A
desktop is already connected`), and the Daemon keeps the first link. The Daemon
never closes a live link to make room. A closed local socket shows end of file at
once, so a Panel that quits or crashes frees the link. When the link closes, the
Daemon drops it and fails each waiting call with `Error: the desktop is not
available`.

The Daemon keeps the link and the waiting calls in one `DesktopLink` handle in
`AppState`. One lock protects it. A waiting call is a record with the call id,
the nonce and a channel for the result. The Daemon removes the record when the
result arrives, when the call is cancelled and when the call times out.

### Link lifecycle

1. **Start.** The Panel starts the link task after `ensure_daemon_started` at
   application start. It runs on macOS builds and on `e2e` builds only.
2. **Connect.** The task connects with the plain `connect_local_daemon`. It never
   starts a Daemon, so it cannot start one again after `RunEvent::Exit` stopped
   it. The next chat request starts a Daemon through `connect_to_daemon`, and the
   task then connects on its next try.
3. **Retry.** After a loss or a failed connect, the task tries again every 5
   seconds, with no limit, until the Panel exits. A failed local connect costs
   almost nothing.
4. **Refusal.** The Daemon may answer `register_desktop` with an `error`. This
   happens with an old Daemon that does not know the message, and with a second
   desktop. The Panel logs the error, offers no tool and tries again after 60
   seconds, so it never loops fast. The Daemon keeps the connection of an old
   Daemon open, so the Panel closes its side first.
5. **Exit.** The task ends with the Panel. The Daemon sees the end of file and
   drops the link.

Panel tests cover each rule: a Daemon that is down, a Daemon that comes back, an
`error` reply, and exit. The tests use a fake Daemon on a socket.

### Offering the tool

`providers/notes.rs` holds `NotesProvider`, a `Provider` behind
`ProviderRegistry`. The registry builder adds it only while the `DesktopLink`
handle holds a registered link with `notes_create_note` in its operations. The
Tools tab shows a source `desktop` (display name "Desktop tools") with state
`ready` while the link is up.

Four rules keep the swap correct. Each one has a test.

1. **Where the state lives.** `Carry` in `providers/mod.rs` also holds a clone of
   the `DesktopLink` handle, and `assemble` reads it. A folder change calls
   `rebuilt`, so it keeps the Notes tool while the link is up. Today `Carry`
   holds only the sandbox and the MCP servers, and a folder change would drop the
   tool without this.
2. **One lock.** One function reads the registry, builds the new one, replaces
   it in `ProviderHandle` and calls `update_tools`. It runs under the write lock
   of `state.config`, the same lock that `folders.rs` takes. A link event and a
   folder change then cannot lose each other's update (`AGENTS.md` Design 6).
   The folder path calls this function too. `folders.rs` already holds that
   lock in `apply`, and the lock is not re-entrant, so the function takes the
   held guard as an argument and never takes the lock a second time.
3. **Waiting approvals.** After each link change, the Daemon calls
   `deny_stale_approvals`. For a waiting approval of the Notes tool, the reason
   is `The desktop is not available`. Today an unknown tool gets the reason
   `Path is outside the allowed folders`, which is wrong here.
4. **Catalog text.** `docs/contracts.md` says the catalog goes out "each time the
   allowed folders change". It now also goes out when the desktop link changes.

Because the tool exists in the catalog only while a Panel can run it, a call
that finds no link is rare. Two cases remain:

- The link closes while the call waits. The call holds the registry it took, so
  it fails with `Error: the desktop is not available`.
- The model calls the tool after the swap but before its Worker has the new
  catalog. The registry no longer has the tool, and `tools.yaml` still lists it.
  The check at `tool_approval.rs:370-377` then denies the call with the reason
  `Tool runs in the Worker`, and a denial ends the reply ("Denial ends the reply"
  in `docs/tools-and-authorization.md`). The window is the short time between the
  swap and the rebuild of the Worker. A distinct reason would need a way to tell a
  Daemon tool from a Worker tool in that check, which is more than this task
  needs. A test pins this behavior, and the docs say it.

Nothing queues for later.

### The call, step by step

1. The Worker sends `execute_tool` for `nook:notes_create_note`, as for any tool
   that the Daemon owns.
2. The Daemon checks the arguments against the schema, asks Cedar and shows the
   approval.
3. After an allow, `NotesProvider::call` makes a nonce, stores the waiting
   record, and sends `native_request` on the desktop link. It then waits for the
   result for 70 seconds.
4. The Panel checks the request. It refuses an operation outside its fixed list.
   It refuses a nonce that it already saw on this link. It checks the arguments
   again (see "Run in the Panel"). It then runs the operation and sends
   `native_result`.
5. The Daemon accepts a `native_result` only when it arrives on the desktop link
   that got the request and matches the `call_id` and the nonce of a waiting
   record. It uses the record once. A second result for the same call, or a
   result for an unknown call, is ignored and logged.
6. The Daemon returns the result to the Worker as `tool_result`.

Cancel aborts the waiting future, as for any tool. The Panel may already run the
script, so a note can exist after a cancel. The Panel gets no cancel message.

What the nonce does: the draft said that the Panel refuses a request "without a
nonce that the Daemon issued". The Panel cannot check who issued a nonce. The
nonce stops two things: the Panel runs no request twice, and the Daemon accepts
no forged or repeated result. The trust anchor is the desktop link itself. The
socket is private to the user, and `approval_decision` has the same trust level.
Any process of the same user that can reach the socket can already approve
tools, so the design adds no new weakness there. The nonce is a replay guard, not
a login.

### The tool

`nook:notes_create_note` takes two arguments. The descriptor schema in the
Daemon checks them before Cedar:

- `title`: a string of 1 to 200 characters, with no line break (`pattern`
  `^[^\r\n]+$`).
- `body`: a string of up to 20,000 characters of plain text. It may be empty.
- No other argument is allowed.

It returns `Created the note "<title>" in the Nook folder` or an error.

The tool description tells the model that it creates a new note in the `Nook`
folder, cannot read or change a note, and exists on macOS only.

### Run in the Panel

A new module `panel/src-tauri/src/desktop.rs` owns the link and the operations.
Its public interface is small: start the link, and run one named operation with
its arguments. Everything else stays private.

The Panel checks the arguments itself, because it is the last step before a
process starts. The arguments must be exactly `title` and `body`, both strings,
with the same limits as the Daemon schema and no line break in the title. If
they fail, the Panel sends `status: error` with the fixed text `Error: the
arguments are not valid`, and it starts no process.

The Panel runs `/usr/bin/osascript` with a script that is a constant in the
module. The script takes one argument, the HTML of the note (`on run argv`). The
text of the model is never part of the script text. The Panel passes no raw
title and no raw body on the command line. The HTML always starts with `<`, so
`osascript` cannot read it as an option. The process starts with an empty
environment and a 60 second limit, and the Panel kills it at the limit.

The Panel builds the HTML first. It escapes `&`, `<` and `>`, and it puts each
line in a `div`. Notes takes the title of a note from the first line of its
body, so the first `div` holds the escaped title as a heading, and the body
lines follow. The manual test checks the title in the Notes list.

The script creates the `Nook` folder in the default account when it is missing,
and then creates the note in it.

The Panel does not run an `osascript` that the model or the Daemon supplies.
Cedar keeps its rule that forbids `osascript` for the command tool.

### Approval and policy

`policy/tools.yaml` gets the entry `nook:notes_create_note` in the `ask` tier
with the actions:

- `allow_once`: "Create this note one time."
- `allow_for_chat`: "Allow this tool for the rest of this chat."
- `deny`.

There is no `always_allow`, because every note is new content in the account of
the user.

The approval shows the folder (`Nook`), the title, and the first 200 characters
of the body with its total length. `NotesProvider::facts` builds this summary.

Cedar gets the action `notes_create_note` with the context values `chat_id`,
`folder`, `approval_tier`, `grant_scope` and `argument_digest`. The policy
permits the call only when `folder` is `Nook` and the grant scope is `once` or
`chat`. Any other case is a deny.

The draft also had Cedar forbid a long body and a title with a line break. The
descriptor schema already refuses both before Cedar, and the context of Cedar
holds only strings. One check in one place is simpler, so Cedar keeps only the
folder and grant rules.

### Permission from macOS

The first call makes macOS ask whether Nook may control Notes. The user can
refuse. A refusal returns `Error: macOS did not allow Nook to control Notes`
(AppleScript error -1743), and macOS does not ask again until the user changes
the setting in System Settings, Privacy and Security, Automation.

The 60 second limit is longer than the 15 seconds of the draft. The first call
waits for a person to answer the macOS prompt, and it may also wait while Notes
starts.

### Signing check (gap G3)

I checked the signing setup on the base commit `e0231bc`:

1. `panel/src-tauri/tauri.conf.json` sets `bundle.macOS.signingIdentity` to `-`.
   That is an ad hoc signature (a signature with no developer certificate).
2. `scripts/bundle.sh` signs with `codesign --force --deep --sign "-"` unless
   `SIGN_IDENTITY` is set. It adds no `--options runtime`.
3. The repository has no entitlements file and no `NSAppleEventsUsageDescription`.
   `scripts/bundle.sh` writes its own `Info.plist` without that key.
4. `.github/workflows/package.yml` has no signing or notarization step.

What this means:

- A build without the hardened runtime needs no entitlement to send Apple
  events. It does need `NSAppleEventsUsageDescription`. Without the key, macOS
  refuses the call and shows no prompt.
- A hardened runtime build needs the entitlement
  `com.apple.security.automation.apple-events` as well.
- An ad hoc signature changes with each build, so macOS may ask for Automation
  permission again after an update.

The task adds both: the usage description in the `Info.plist` of the Tauri
bundle and in `scripts/bundle.sh`, and an entitlements file with the automation
key that `tauri.conf.json` references. The release build runs `npm run tauri
build`, so `tauri.conf.json` is the release path. `scripts/bundle.sh` signs
with no `--entitlements` flag, so it gets only the usage description. It does not change the signing identity
or turn on the hardened runtime. A real signed and notarized release stays a
later task. The plan has a step that builds the bundle on a Mac and runs
`codesign -d --entitlements :- Nook.app` and `plutil -p Info.plist` to check the
result. The result goes into the task record before the pull request leaves
draft.

### Data handling

The Daemon stores the title and the body only in the chat transcript, as for any
tool argument. The Panel writes them nowhere except the note. The Panel logs the
operation name and the status, never the title or the body. The tool cannot read
a note, so the assistant never sees what is already in Notes.

### Isolation

The Panel adds no Tauri command and no capability permission for this operation.
The operation lives in Rust, behind the desktop link. The web view can neither
start it nor read a nonce.

### The test build

Warning: a setting that lets the Panel run another program in place of
`osascript` must not exist in a release build. The Panel reads the setting
`NOOK_E2E_OSASCRIPT` only under `#[cfg(feature = "e2e")]`. The `e2e` build also
registers the desktop link on every platform, so the native tests run on Ubuntu.

### Failure behavior

| Case | Result |
| --- | --- |
| The link closes while the call waits | `Error: the desktop is not available`. |
| The call runs after the link closed and the registry changed | The Daemon denies it with `Tool runs in the Worker`, and the reply ends. The next catalog removes the tool. |
| A waiting approval when the link closes | The Daemon denies it with the reason `The desktop is not available`. |
| A second desktop registers | The Daemon answers with an `error` and keeps the first link. |
| An old Daemon refuses `register_desktop` | The Panel logs it, offers no tool and tries again after 60 seconds. |
| Platform is not macOS | The tool is not offered. |
| User denies the approval | A fixed denial reason, as for any tool. |
| macOS refuses Automation | `Error: macOS did not allow Nook to control Notes`. |
| Notes does not answer within 60 seconds | `Error: Notes did not answer`. |
| Any other `osascript` failure | `Error: Notes could not create the note`. The Panel logs the detail. |
| The Daemon gets no result within 70 seconds | `Error: the desktop did not answer`. |
| Request has an operation outside the list | The Panel refuses it and logs it. |
| Request has a repeated nonce | The Panel refuses it and logs it. |
| Result has a wrong, unknown or used nonce or call id | The Daemon ignores it and logs it. |
| Arguments break the schema | The Daemon refuses the call before any prompt. |
| The Panel finds invalid arguments | `Error: the arguments are not valid`. No process starts. |

### Tests

- Contracts: schema and examples for `register_desktop`, `desktop_registered`,
  `native_request` and `native_result`. The contract test in
  `tests/contracts/test_contracts.py` covers them.
- Policy: the tool needs a grant with scope `once` or `chat`. No permanent grant
  is offered. A call that carries any other folder is denied. The real policy
  files are used.
- Daemon, through the real socket and the fake Worker: the tool is absent from
  the catalog until a desktop link registers, and absent again after the link
  closes. A call reaches the link only after an allow. A result with a wrong
  nonce, a wrong call id or a second result is ignored. A link that closes
  fails the waiting call and denies a waiting approval with its own reason. A
  second registration gets an `error` and the first link stays. A timeout
  returns the fixed error. A cancel removes the waiting record. A folder change
  while the link is up keeps the tool. A call for the tool after the link closed
  and the registry changed is denied with `Tool runs in the Worker`, and the
  reply ends.
- Panel: an operation outside the list is refused. A repeated nonce is refused.
  Arguments that are not exactly a valid `title` and `body` start no process.
  The HTML conversion escapes `&`, `<` and `>`. A body with quotes, backslashes
  and `$(...)` reaches the program as one plain argument. Titles such as `-e`
  and `-l` also arrive inside the HTML and not as an option. The link lifecycle
  tests use a fake Daemon on a socket (see "Link lifecycle").
- Native flow: the `e2e` build runs a fake program in place of `osascript`. It
  writes its arguments to a file. The test checks that the program ran only
  after Allow, that a deny leaves the file empty, and that a cancel before the
  approval does not run it. No automated test touches the real Notes app.
- Manual, on a Mac: one real note, to check the macOS prompt, the title in the
  Notes list, the HTML, a title that starts with `-`, the run with an empty
  environment, the refusal path, and the signing result from the "Signing check"
  section.

### Commands

Each changed component runs its commands from `docs/testing.md`. This task adds
the Panel Rust commands, because no gate runs them today (CI runs only `cargo
check` in `panel/src-tauri`):

1. Daemon, from `daemon/`: `rtk cargo test` and `rtk cargo clippy --workspace
   --all-targets -- -D warnings`.
2. Panel Rust, from `panel/src-tauri/`: `rtk cargo test` and `rtk cargo clippy
   --all-targets -- -D warnings`. These are new in `docs/testing.md`, and the
   `Code Quality and Tests` job gets a step for each. If the base commit already
   fails clippy, or a test of `panel/src-tauri/src/lib.rs` fails on the CI
   runner, the first plan task records the failure as a gap, and the owner
   decides. The CI step runs only after the first plan task shows that both
   commands pass on the base.
3. Contracts: `rtk uv run --with pytest --with jsonschema pytest tests/contracts
   panel/e2e/test_fake_worker.py`.
4. Native: `rtk npm run test:e2e:deterministic` from `panel/`.

The web view does not change, so the npm lint, unit and build commands do not
apply.

## Central docs that change

1. `docs/architecture.md`: the Panel also runs desktop tools. Add the desktop
   link to "Processes" and "Tool boundary".
2. `docs/contracts.md`: a "Desktop link" section with the four messages and the
   nonce rule. The schemas and examples change in the same commit as the code.
3. `docs/tools-and-authorization.md`: a "Desktop tools" section with the Notes
   tool, its approval actions, the Cedar action and the failure messages. It
   names the denial `Tool runs in the Worker` for a call after the link closed.
4. `docs/testing.md`: the new Daemon, Panel and native tests, the Panel Rust
   commands, and the fake `osascript` setting of the `e2e` build.
5. `docs/release.md`: the Automation usage description and entitlement, and the
   note that ad hoc builds may ask again after an update.

Other files that change: `docs/adr/0008-panel-runs-desktop-app-tools.md` (status
Accepted, the desktop link, and the nonce meaning), `CHANGELOG.md` ("Unreleased"),
`policy/tools.yaml`, `policy/cedar/` (schema and policies), `contracts/`,
`.github/workflows/ci.yml` (the Panel Rust test step), `panel/src-tauri/tauri.conf.json`,
`scripts/bundle.sh` and a new entitlements file in `panel/src-tauri/`.
`docs/contracts.md` also changes the catalog sentence (see "Offering the tool").

`docs/mcp-servers.md` does not change. `docs/plan.md` does not change. The
housekeeping task owns `CHECKLIST.md` and `docs/plans/`, so this task leaves
them. Four of those files point to `docs/design/desktop-app-tools.md`, which moved
to this folder (see "Risks").

## Rule changes

None. This task changes no file under `.agents/rules/`, `AGENTS.md`,
`CLAUDE.md` or `CONTRIBUTING.md`.

## Open gaps

`open-gaps.md` holds four questions, each with options and a recommendation. G1
to G3 come from the draft. G4 came from spec review round 1.

1. G1: a fixed `Nook` folder, or a folder set in `config.toml`.
2. G2: append to an existing note, or not.
3. G3: the Automation entitlement. The check is in "Signing check" above.
4. G4: a long-lived desktop link, or no link.

## Risks

1. The desktop link is new surface. A process of the same user can register as a
   desktop and receive the title and body of each note. It can also answer with
   a false `ok`, so the model believes a note exists. The first live link wins,
   so a process that registers first keeps the real Panel off until it closes.
   That process can already send `approval_decision`, and it can read the
   database URL in `config.toml`, so the user boundary does not change.
2. The tool changes how the Daemon and the Panel connect. A wrong nonce or
   ownership check could let a forged result pass. The Daemon tests cover this.
3. A cancel can leave a note behind, because the Panel gets no cancel message.
4. I cannot run Notes on Linux. The HTML, the title rule and the macOS prompt
   are checked only in the manual test on a Mac. The owner or an agent with a
   Mac must run it before the pull request merges.
5. An ad hoc signature changes per build, so macOS may ask again after each
   update. A signed release removes this, and it needs its own task.
6. The move of the draft leaves four dangling paths to
   `docs/design/desktop-app-tools.md` in `CHECKLIST.md`,
   `docs/plans/app-integrations.md`, `docs/plans/tool-platform-open-gaps.md` and
   `docs/plans/tp7-tp13-tool-providers.md`. The housekeeping task owns those
   files. The orchestrator tells the owner, and a later small change updates the
   paths to `docs/tasks/task-PR-40/specs/design.md`.
