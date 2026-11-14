# Spec: Notes tool run by the Daemon outside the sandbox

```
Stage: 3 spec review
Gate 1: pending
Gate 2: pending
```

Branch: `feat/notes-tool`. Pull request: #40.

This spec starts from the draft `docs/design/desktop-app-tools.md` of October 17,
2026. The draft moved here with this task. The draft and the first version of
this spec had the Panel run the tool, and the owner chose that route on October
17, 2026 (see `docs/plans/app-integrations.md`, option 2). On November 14, 2026
the owner changed the route: the Daemon runs the tool itself, outside the
command sandbox, because the Daemon is the core of Nook and the Panel is only
the view (gap G4). ADR 0008 records the decision, and this task rewrites it and
changes its status to Accepted.

## Why

The owner wants the assistant to write to the Notes app. A Notes server from the
internet cannot do this inside the command sandbox, because macOS refuses Apple
events (the messages that one Mac app sends to another) from any process that
runs under a sandbox profile. So the tool cannot use the sandbox that the
command tool and the MCP servers use.

The Daemon already owns every tool call: it checks the arguments, asks Cedar,
shows the approval and runs the tool. This design keeps that whole path in the
Daemon. The Daemon starts `/usr/bin/osascript` itself, as a plain child process
with one fixed script. The Panel does not change.

The draft before this one had clipboard, notification and open-link tools. The
owner removed them because the user can do each of them without the assistant.

## Scope

In scope: one tool, `nook:notes_create_note`. It creates a new note in a folder
named `Nook` in the default Notes account. It works on macOS only.

Not in scope: reading, searching, editing, moving or deleting notes, other
folders, other accounts, Reminders, Calendar, Mail, Messages, the clipboard,
screen capture and any other app. Each of those needs its own design, because
reading a note shows private data and a delete cannot be undone.

The Panel starts the Daemon. A Daemon that something else starts, such as
`nookctl` or a person in a terminal, is not a supported case for this tool.

This is not a release. No version file changes. The change adds one line under
"Unreleased" in `CHANGELOG.md`.

## Design

### Platforms

| Platform | Support | Reason |
| --- | --- | --- |
| macOS | Yes | Notes and Apple events exist. |
| Linux | No | No Notes app. The tool is not offered. |
| Windows | No | No Notes app. The tool is not offered. |

### Where each tool runs

This design needs an accurate picture of the sandbox, because the Notes tool is
the first tool that must run a program outside it.

| Tool | Runs in | OS sandbox |
| --- | --- | --- |
| File tools | The Daemon, in process | No. The allowed folders and Cedar limit them. |
| Command tool and MCP servers | The Daemon, as a child process | Yes. Seatbelt on macOS, bubblewrap on Linux. |
| Web search and web fetch | The Worker | No. |
| Notes tool (new) | The Daemon, as a child process | No. macOS refuses Apple events from a sandboxed process. |

The Daemon builds its tool list from a `ProviderRegistry` and sends it to the
Worker as `tool_catalog`. A tool that the Daemon owns reaches the Daemon as
`execute_tool`. The Notes tool is one more `Provider` in that registry, so the
Worker, the Panel and the contracts do not change.

### The call, step by step

1. The model calls `nook:notes_create_note`. The Worker checks the arguments and
   sends `authorize_tool`, as for any tool.
2. The Daemon checks the arguments against the schema, asks Cedar and shows the
   approval to the user.
3. After an allow, the Worker sends `execute_tool`. `NotesProvider::call` checks
   the arguments again, builds the HTML of the note and runs `osascript`.
4. The Daemon returns the result to the Worker as `tool_result`.

### Offering the tool

`providers/notes.rs` holds `NotesProvider`, a `Provider` behind
`ProviderRegistry`. Its constructor takes the path of the program to run. A
release build always passes `/usr/bin/osascript`.

`assemble` in `providers/mod.rs` adds `NotesProvider` when the build targets
macOS (or is an `e2e` build, see "The test build") and the program file exists.
When the file is missing, the Daemon does not offer the tool and logs why once at
start. The Daemon does not run `osascript` to test it, because a test run would
show the macOS prompt.

`NotesProvider` holds no state, so a folder change that calls `rebuilt` adds it
again. The tool never leaves the catalog while the Daemon runs. A test pins this.

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

### Run in the Daemon

`NotesProvider::call` checks the arguments itself, because it is the last step
before a process starts. The arguments must be exactly `title` and `body`, both
strings, with the same limits as the schema and no line break in the title. If
they fail, it returns `Error: the arguments are not valid` and starts no
process.

The provider builds the HTML first, in one small function that tests call
directly. It escapes `&`, `<` and `>`, and it puts each line in a `div`. Notes
takes the title of a note from the first line of its body, so the first `div`
holds the escaped title as a heading, and the body lines follow. The manual test
checks the title in the Notes list.

Six rules keep the operation closed. The model supplies data and never code or
an option.

1. The program is the path that the constructor got. A release build passes
   `/usr/bin/osascript`.
2. The script is a constant in `notes.rs`. It takes one argument (`on run
   argv`). The text of the model is never part of the script text.
3. The one argument is the HTML. It always starts with `<`, so `osascript`
   cannot read it as an option. The provider passes no raw title and no raw body
   on the command line.
4. The process starts with an empty environment and with no standard input.
5. The process runs in its own process group with `kill_on_drop`. The provider
   kills the group at the 60 second limit and when the call is cancelled. It
   reuses `kill_group` from `sandbox.rs`, which becomes `pub(crate)`.
6. The provider reads at most 4096 bytes of standard output and of standard
   error. The model sees only the fixed texts of the failure table. The Daemon
   logs the exit status and the AppleScript error number, never the output
   text, because a script error can contain part of the note.

The script creates the `Nook` folder in the default account when it is missing,
and then creates the note in it.

Cedar keeps its rule that forbids `osascript` for the command tool. The Notes
tool does not use the command tool.

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
(AppleScript error -1743 in the standard error of `osascript`), and macOS does
not ask again until the user changes the setting in System Settings, Privacy and
Security, Automation.

macOS shows the name of the responsible app in this prompt. The Panel starts the
Daemon as a child process, so the responsible app should be Nook, and macOS
should read the usage description from the `Info.plist` of `Nook.app`. This is
not checked, because it needs a Mac (Risk 1).

The 60 second limit leaves time for a person to answer the macOS prompt, and for
Notes to start.

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
with no `--entitlements` flag, so it gets only the usage description. It does not
change the signing identity or turn on the hardened runtime. A real signed and
notarized release stays a later task. The plan has a step that builds the bundle
on a Mac and runs `codesign -d --entitlements :- Nook.app` and `plutil -p
Info.plist` to check the result. The result goes into the task record before the
pull request merges.

If the manual test shows that macOS names `nookd` in the prompt, or shows no
prompt, the usage description must also go into the `nookd` binary. That is a
new gap for the owner (see Risk 1).

### Data handling

The Daemon stores the title and the body only in the chat transcript, as for any
tool argument. The provider writes them nowhere except the note. The Daemon logs
the operation name and the status, never the title or the body. The tool cannot
read a note, so the assistant never sees what is already in Notes.

### The test build

Warning: a setting that lets the Daemon run another program in place of
`osascript` must not exist in a release build. `nookd` reads the setting
`NOOK_E2E_OSASCRIPT` only under `#[cfg(feature = "e2e")]` and passes the path to
the registry builder. `nook-core` reads no environment variable. An `e2e` build
also offers the tool on every platform, so the native tests run on Ubuntu.

### Failure behavior

| Case | Result |
| --- | --- |
| Platform is not macOS (release build) | The tool is not offered. |
| `/usr/bin/osascript` does not exist | The tool is not offered, and the Daemon logs why at start. |
| User denies the approval | A fixed denial reason, as for any tool. |
| Arguments break the schema | The Daemon refuses the call before any prompt. |
| The provider finds invalid arguments | `Error: the arguments are not valid`. No process starts. |
| macOS refuses Automation | `Error: macOS did not allow Nook to control Notes`. |
| Notes does not answer within 60 seconds | `Error: Notes did not answer`. The Daemon kills the process group. |
| `osascript` cannot start | `Error: Notes could not create the note`. |
| Any other `osascript` failure | `Error: Notes could not create the note`. The Daemon logs the exit status and the error number. |
| The call is cancelled | The Daemon kills the process group. A note can still exist if the script had finished. |

### Tests

- Policy: the tool needs a grant with scope `once` or `chat`. No permanent grant
  is offered. A call that carries any other folder is denied. The real policy
  files are used.
- Daemon, through the real socket and the fake Worker, with a fake `osascript`
  (a script that writes its arguments to a file and exits with a chosen code).
  The tests run on Linux:
  - The tool is in the catalog when the provider is built, and it stays after a
    folder change.
  - The program runs only after an allow. A deny leaves the file empty. A cancel
    before the approval does not run it.
  - The program gets exactly one argument, it starts with `<`, and the
    environment is empty. A title such as `-e` or `-l` arrives inside the HTML.
  - A body with quotes, backslashes, `$(...)` and a line break reaches the
    program as plain text in that one argument.
  - The HTML conversion escapes `&`, `<` and `>`.
  - Arguments that are not exactly a valid `title` and `body` start no process.
  - An exit with the error number -1743 returns the macOS message. A slow
    program is killed at the limit and returns the timeout message, and its
    process group is gone. A cancel kills the process group.
  - The failure messages never contain the output text of the program.
- Native flow: the `e2e` build of `nookd` runs a fake program in place of
  `osascript`, as in the Daemon tests, and the native test checks the same
  allow, deny and cancel cases through the Panel.
- Manual, on a Mac: one real note, to check the macOS prompt and its app name,
  the title in the Notes list, the HTML, a title that starts with `-`, the run
  with an empty environment, the refusal path, and the signing result from the
  "Signing check" section. No automated test touches the real Notes app.

### Commands

Each changed component runs its commands from `docs/testing.md`:

1. Daemon, from `daemon/`: `rtk cargo test` and `rtk cargo clippy --workspace
   --all-targets -- -D warnings`.
2. Contracts: `rtk uv run --with pytest --with jsonschema pytest tests/contracts
   panel/e2e/test_fake_worker.py`. No schema, example or message changes, so
   these only have to keep passing.
3. Native: `rtk npm run test:e2e:deterministic` from `panel/`. The plan task
   finds where the native tests build `nookd` and adds the `e2e` feature there.

The Panel code and the web view do not change, so the Panel Rust commands and
the npm commands do not apply. The Panel bundle settings change, and the Mac
build step checks them.

## Central docs that change

1. `docs/architecture.md`: the Notes tool runs in the Daemon as a child process
   outside the sandbox. Add it to "Tool boundary".
2. `docs/tools-and-authorization.md`: a "Notes tool" section with the tool, its
   approval actions, the Cedar action and the failure messages. It also corrects
   the section "Tools that run in the Daemon": the table in "Where each tool
   runs" says which tools use the sandbox.
3. `docs/testing.md`: the new Daemon tests, the fake `osascript` and the
   `NOOK_E2E_OSASCRIPT` setting of the `e2e` build.
4. `docs/release.md`: the Automation usage description and entitlement, and the
   note that ad hoc builds may ask again after an update.

Other files that change: `docs/adr/0008-panel-runs-desktop-app-tools.md` (renamed
to `docs/adr/0008-daemon-runs-desktop-app-tools-outside-the-sandbox.md`, rewritten
for this route, status Accepted, and its link to the draft updated),
`docs/adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md` (one short
paragraph that names the Notes tool as the exception), `CHANGELOG.md`
("Unreleased"), `policy/tools.yaml`, `policy/cedar/` (schema and policies),
`policy_assets.rs` (the approval actions), `daemon/crates/nook-core/src/providers/`
(`notes.rs`, `mod.rs`), `daemon/crates/nook-core/src/sandbox.rs` (`kill_group`
becomes `pub(crate)`), `daemon/crates/nookd/src/main.rs` (the `e2e` setting),
`panel/src-tauri/tauri.conf.json`, `scripts/bundle.sh` and a new entitlements
file in `panel/src-tauri/`.

`docs/contracts.md` does not change, because no message changes. `docs/mcp-servers.md`
does not change. `docs/plan.md` does not change. The housekeeping task owns
`CHECKLIST.md` and `docs/plans/`, so this task leaves them. Four of those files
point to `docs/design/desktop-app-tools.md`, which moved to this folder, and
`docs/plans/app-integrations.md` describes the Panel route (see "Risks").

## Rule changes

None. This task changes no file under `.agents/rules/`, `AGENTS.md`,
`CLAUDE.md` or `CONTRIBUTING.md`.

## Open gaps

`open-gaps.md` holds four questions, each with options and a recommendation. G1
to G3 come from the draft. G4 came from spec review round 1, and the owner
decided it on November 14, 2026.

1. G1: a fixed `Nook` folder, or a folder set in `config.toml`.
2. G2: append to an existing note, or not.
3. G3: the Automation entitlement. The check is in "Signing check" above.
4. G4: where the operation runs. Decided by the owner: the Daemon runs it,
   outside the sandbox.

## Risks

1. macOS may blame the wrong app in the Automation prompt. The Panel starts the
   Daemon, so I expect the prompt to name Nook, but I cannot check it on Linux.
   If it names `nookd` or shows no prompt, the usage description must go into
   the `nookd` binary, which is a new gap. The owner tests this on a Mac.
2. `osascript` may need an environment variable that the empty environment
   removes. The manual test checks it. If it fails, the first fix is to pass
   `HOME` only, and that is a small fix to the provider.
3. The tool runs a program outside the sandbox. A flaw in the argument handling
   or in the HTML would run in a process with the rights of the user. The
   closed operation limits this: a fixed program, a constant script, one
   argument that starts with `<`, a schema and a second check, Cedar, and an
   approval for every call. The tests use hostile titles and bodies.
4. A cancel or a timeout can leave a note behind, because the script may have
   finished before the Daemon kills it.
5. I cannot run Notes on Linux. The HTML, the title rule, the empty environment
   and the macOS prompt are checked only in the manual test on a Mac. The owner
   will run it before the pull request merges.
6. An ad hoc signature changes per build, so macOS may ask again after each
   update. A signed release removes this, and it needs its own task.
7. The move of the draft leaves four dangling paths to
   `docs/design/desktop-app-tools.md` in `CHECKLIST.md`,
   `docs/plans/app-integrations.md`, `docs/plans/tool-platform-open-gaps.md` and
   `docs/plans/tp7-tp13-tool-providers.md`. `docs/plans/app-integrations.md` also
   still names the Panel route as the choice. The housekeeping task owns those
   files. The orchestrator tells the owner, and a later small change updates the
   paths to `docs/tasks/task-PR-40/specs/design.md` and the route.
