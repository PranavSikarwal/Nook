# Spec: Notes tool run by the Daemon outside the sandbox

```
Stage: 3 spec review
Gate 1: pending
Gate 2: pending
```

Branch: `feat/notes-tool`. Pull request: #40.

This spec replaces the draft `docs/design/desktop-app-tools.md` of October 17,
2026, and this task deletes that file. The draft and the first version of this
spec had the Panel run the tool, and the owner chose that route on October 17,
2026 (see `docs/plans/app-integrations.md`, option 2). On November 14, 2026 the
owner changed the route: the Daemon runs the tool itself, outside the command
sandbox, because the Daemon is the core of Nook and the Panel is only the view
(gap G4). ADR 0008 records the decision, and this task rewrites it and changes
its status to Accepted.

## Why

The owner wants the assistant to write to the Notes app. A Notes server from the
internet cannot do this inside the command sandbox, because macOS refuses Apple
events (the messages that one Mac app sends to another) from any process that
runs under a sandbox profile. So the tool cannot use the sandbox that the
command tool and the MCP servers use.

The Daemon already owns every tool call: it checks the arguments, asks Cedar,
shows the approval and runs the tool. This design keeps that whole path in the
Daemon. The Daemon starts `/usr/bin/osascript` itself, as a plain child process
with one fixed script. The Panel code does not change.

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
the first tool that starts a program outside it.

| Tool | Runs in | OS sandbox |
| --- | --- | --- |
| File tools | The Daemon, in process | No. The allowed folders and Cedar limit them. |
| Command tool and MCP servers | The Daemon, as a child process | Yes. Seatbelt on macOS, bubblewrap on Linux. |
| Web, news, video and book search, and web fetch | The Worker | No. |
| Notes tool (new) | The Daemon, as a child process | No. macOS refuses Apple events from a sandboxed process. |

The Daemon builds its tool list from a `ProviderRegistry` and sends it to the
Worker as `tool_catalog`. A tool that the Daemon owns reaches the Daemon as
`execute_tool`. The Notes tool is one more `Provider` in that registry, so the
Worker, the Panel code and the message schemas do not change.

### The call, step by step

A tool that the Daemon owns never uses `authorize_tool`. The Daemon denies that
message for such a tool with `Tool runs in the Daemon`.

1. The model calls `nook:notes_create_note`. The Worker checks the arguments
   against the descriptor schema and sends `execute_tool`.
2. The Daemon checks the schema again, builds the facts, asks Cedar and shows
   the approval to the user. A deny ends the call with a fixed denial reason, as
   for any Daemon tool.
3. After an allow, the Daemon sends `tool_call_started`, runs
   `NotesProvider::call`, then sends `tool_call_finished` and `tool_result`.

### Offering the tool

`providers/notes.rs` holds `NotesProvider`, a `Provider` behind
`ProviderRegistry`. Its constructor takes the path of the program to run.

`nookd` decides whether the platform has Notes, and `nook-core` has no platform
or feature test for it:

1. `build_registry(tools, notes_program: Option<PathBuf>)` gets a new parameter.
2. `nookd` passes `Some("/usr/bin/osascript")` on macOS. In an `e2e` build it
   passes the path in `NOOK_E2E_OSASCRIPT` on every platform. Anywhere else it
   passes `None`.
3. `Carry` in `providers/mod.rs` holds a clone of `notes_program`, so `rebuilt`
   after a folder change keeps the tool.
4. `assemble` adds `NotesProvider` when the path is `Some` and the file exists.
   When the path is `Some` and the file is missing, the registry reports the
   source as `off` with a reason, so the Tools tab says why, and the Daemon logs
   it once. The Daemon never runs `osascript` to test it, because a test run
   would show the macOS prompt.
5. `mod notes` has the same `#[cfg(unix)]` gate as `mcp` in `providers/mod.rs`,
   because the Windows job compiles the whole workspace.

The provider reports its own source: id `notes`, display name "Notes tool", state
`ready`, one tool. `ToolSourceInfo.source_id` is any non-empty string in the
schema, so no schema changes. `docs/contracts.md` lists the ids in one sentence,
and that sentence gets the new id.

`tools.yaml` lists the tool on every platform, but the registry holds it only
where the path is `Some`. A call for it elsewhere is denied with `Tool runs in
the Worker` (the check at `tool_approval.rs`), and a denial ends the reply. The
model never sees the tool there, so the call needs a forged message. The docs
say this.

### The tool

`nook:notes_create_note` takes two arguments. The descriptor schema in the
Daemon checks the types and the sizes before Cedar:

- `title`: a string of 1 to 200 characters.
- `body`: a string of up to 20,000 characters of plain text. It may be empty.
- Both are required (`required`), and no other argument is allowed
  (`additionalProperties: false`).

The schema has no `pattern`. The Worker checks arguments with Python
`jsonschema`, where `$` also matches before a final line feed. The Daemon uses the
Rust crate, where it does not. A title that ends with a line feed would pass the
Worker and fail the Daemon, and a refusal ends the reply. So one function in
`notes.rs` checks the content, in Rust only. `facts` and `call` both use it:

1. Remove one trailing line break from the title (`\n` or `\r\n`).
2. Refuse a title that is now empty or that still has a line break (`\r` or
   `\n`).
3. Refuse a NUL character in the title or the body. A NUL passes JSON and the
   schema, but it cannot go into a process argument.

A refusal gives the fixed reason `Invalid arguments`, before any approval is
shown.

It returns `Created the note "<title>" in the Nook folder` or an error.

The tool description tells the model that it creates a new note in the `Nook`
folder, cannot read or change a note, and exists on macOS only.

### Run in the Daemon

The same function runs at the start of `NotesProvider::call`, because `call` is
the last step before a process starts. If it refuses, `call` returns `Error: the
arguments are not valid` and starts no process.

The provider builds the HTML next, in one small function that tests call
directly. It escapes `&`, `<` and `>`, and it puts each line in a `div`. Notes
takes the title of a note from the first line of its body, so the first `div`
holds the escaped title as a heading, and the body lines follow. The manual test
checks the title in the Notes list.

The process gets exactly these arguments:

```
/usr/bin/osascript -e <SCRIPT> <HTML>
```

`SCRIPT` is this constant in `notes.rs`:

```
on run argv
  set noteHtml to item 1 of argv
  tell application "Notes"
    tell default account
      if not (exists folder "Nook") then make new folder with properties {name:"Nook"}
      make new note at folder "Nook" with properties {body:noteHtml}
    end tell
  end tell
end run
```

The script makes the `Nook` folder in the default account when it is missing,
and then the note. It uses `item 1 of argv` as a string value, with no `do shell
script`, `run script`, `load script` or `System Events`. The Mac test runs this
script, and it can change only after a review of this spec.

Eight rules keep the operation closed. The model supplies data and never code or
an option.

1. The program is the path that the constructor got. A release build gets
   `/usr/bin/osascript` from `nookd`.
2. The arguments are the list above. The script is a constant and the text of
   the model is never part of it.
3. The HTML always starts with `<`, so `osascript` cannot read it as an option.
   The provider passes no raw title and no raw body on the command line.
4. The environment holds only `LANG=en_US.UTF-8`, so `osascript` reads a title
   with non-ASCII characters as UTF-8. The working directory is `/`. There is no
   standard input.
5. The provider runs one `osascript` at a time (one `tokio::sync::Mutex`), so two
   parallel first calls cannot both create the `Nook` folder.
6. The process runs in its own process group (`process_group(0)`). The provider
   reuses `GroupKill`, `read_capped` and `join_reader` from `sandbox.rs`, which
   become `pub(crate)` and `#[cfg(unix)]`. It does not copy the timeout and
   read loop. A cancel aborts the task, so no code after an `await` runs, and the
   group kill sits in the `GroupKill` drop guard.
7. The limit is 60 seconds. The provider reads at most 4096 bytes of standard
   output and of standard error, and it drains the rest of each pipe, so a long
   output cannot block `osascript`.
8. Success is exit status 0. A refusal by macOS is a kept standard error that
   contains `(-1743)`. Every other result is a failure. The model sees only the
   fixed texts of the failure table. The Daemon logs the exit status and the
   AppleScript error number, never the output text, because a script error can
   contain part of the note.

Cedar keeps its rule that forbids `osascript` for the command tool. The Notes
tool does not use the command tool.

### Approval and policy

`policy/tools.yaml` gets the entry `nook:notes_create_note` in the `ask` tier
with the actions:

- `allow_once`: "Create this note one time."
- `allow_for_chat`: "Allow this tool for the rest of this chat."
- `deny`.

`APPROVAL_ACTIONS` in `policy_assets.rs` already holds these three ids, so that
file does not change. There is no `always_allow`, because every note is new
content in the account of the user. `allow_for_chat` has no count limit per
chat (see Risk 8).

The approval shows the folder (`Nook`), the title, and the first 200 characters
of the body with its total length. `NotesProvider::facts` builds this summary.

Cedar gets the action `notes_create_note` with the context values `chat_id`,
`folder`, `approval_tier`, `grant_scope` and `argument_digest`. The policy
permits the call only when `folder` is `Nook` and the grant scope is `once` or
`chat`. Any other case is a deny.

The draft also had Cedar forbid a long body and a title with a line break. The
schema and the check in `notes.rs` already refuse both before Cedar, and the
context of Cedar holds only strings. One check in one place is simpler, so Cedar
keeps only the folder and grant rules.

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
3. The repository has no `Info.plist` file, no entitlements file and no
   `NSAppleEventsUsageDescription`. `scripts/bundle.sh` writes its own
   `Info.plist` without that key.
4. `.github/workflows/package.yml` has no signing or notarization step.

What this means:

- A build without the hardened runtime needs no entitlement to send Apple
  events. It does need `NSAppleEventsUsageDescription`. Without the key, macOS
  refuses the call and shows no prompt.
- A hardened runtime build needs the entitlement
  `com.apple.security.automation.apple-events` as well.
- An ad hoc signature changes with each build, so macOS may ask for Automation
  permission again after an update.

The task adds three things:

1. `panel/src-tauri/Info.plist` with `NSAppleEventsUsageDescription` and the
   text "Nook needs to control Notes so that the assistant can save a note that
   you approve." Tauri merges a file with this name into the bundle `Info.plist`.
2. `panel/src-tauri/entitlements.plist` with the key
   `com.apple.security.automation.apple-events`, and the setting
   `bundle.macOS.entitlements` in `tauri.conf.json` that points to it.
3. The same usage description in the `Info.plist` that `scripts/bundle.sh`
   writes. That script signs with no `--entitlements` flag, so it gets only the
   usage description.

The release build runs `npm run tauri build`, so `tauri.conf.json` is the release
path. The task does not change the signing identity or turn on the hardened
runtime. A real signed and notarized release stays a later task. The plan has a
step that builds the bundle on a Mac and runs `codesign -d --entitlements :-
Nook.app` and `plutil -p Info.plist` to check that Tauri merged the file. The
result goes into the task record before the pull request merges.

If the manual test shows that macOS names `nookd` in the prompt, or shows no
prompt, the usage description must also go into the `nookd` binary. That is a
new gap for the owner (see Risk 1).

### Data handling

The Daemon stores the title and the body only in the chat transcript, as for any
tool argument. The provider writes them nowhere except the note. The Daemon logs
the operation name and the status, never the title or the body.

One path logs a value today. `ProviderRegistry::facts` builds `Invalid
arguments: {error}` from the `jsonschema` error, and that error text prints the
refused value. `tool_approval.rs` logs it. This task changes that message so it
names the keyword and the property and never the value. The change also protects
the file tools. A test checks the message text.

The tool cannot read a note, so the assistant never sees what is already in
Notes.

### The test build

Warning: a setting that lets the Daemon run another program in place of
`osascript` must not exist in a release build. `nookd` reads the setting
`NOOK_E2E_OSASCRIPT` only under `#[cfg(feature = "e2e")]`, in the same way as
`NOOK_E2E_WORKER_COMMAND`, and passes the path to `build_registry`. No variable
sets the program path in `nook-core`. Release builds of `nookd` use no feature.
`panel/e2e/run.mjs` turns `e2e` on only for the native test build.

### Failure behavior

| Case | Result |
| --- | --- |
| Platform is not macOS (release build) | `nookd` passes `None`. The tool is not offered. |
| `/usr/bin/osascript` does not exist | The tool is not offered. The Tools tab shows the source `notes` as `off` with a reason, and the Daemon logs it once. |
| A call for a tool in `tools.yaml` that the registry lacks | The Daemon denies it with `Tool runs in the Worker`, and the reply ends. The docs say it. |
| User denies the approval | A fixed denial reason, as for any tool. |
| Arguments break the schema | The Daemon refuses the call before any prompt, with the fixed reason `Invalid arguments` and a log line without the value. |
| The title ends with one line break | The provider removes it and goes on. |
| The title is empty, has another line break, or the title or the body has a NUL | The provider refuses the call before any prompt, with `Invalid arguments`. |
| `call` finds invalid arguments | `Error: the arguments are not valid`. No process starts. |
| macOS refuses Automation | `Error: macOS did not allow Nook to control Notes`. |
| Notes does not answer within 60 seconds | `Error: Notes did not answer`. The Daemon kills the process group. |
| `osascript` cannot start | `Error: Notes could not create the note`. |
| Any other failure (non-zero exit, killed by a signal) | `Error: Notes could not create the note`. The Daemon logs the exit status and the error number. |
| The program writes more than 4096 bytes | The provider keeps 4096 bytes, drains the rest and judges only the exit status. |
| The call is cancelled | The drop guard kills the process group. A note can still exist if the script had finished. |

### Tests

- Policy: the tool needs a grant with scope `once` or `chat`. No permanent grant
  is offered. A call that carries any other folder is denied. The real policy
  files are used.
- Registry, with a fake program path: `build_registry(tools, None)` has no Notes
  tool. With a path to a fake program it has the tool and the source `notes`. A
  path to a missing file gives the source `off` with a reason. `rebuilt` after a
  folder change keeps the tool.
- Daemon, through the real socket and the fake Worker, with a fake `osascript` (a
  script that writes its arguments, its environment and its working directory to
  a file and exits with a chosen code). The tests run on Linux:
  - The program runs only after an allow. A deny leaves the file empty. A cancel
    before the approval does not run it.
  - The program gets exactly three arguments: `-e`, the script and the HTML. The
    script is byte for byte the same for two calls with different titles. The
    HTML starts with `<`. The environment is exactly `LANG=en_US.UTF-8`, and the
    working directory is `/`.
  - A title such as `-e` or `-l` arrives inside the HTML.
  - A body with quotes, backslashes, `$(...)` and a line break reaches the
    program as plain text in the HTML argument.
  - The HTML conversion escapes `&`, `<` and `>`.
  - A title ending with a line break is trimmed. An empty title, an inner line
    break and a NUL in the title or the body start no process and show no
    prompt. A body that breaks the schema starts no process.
  - A schema refusal puts no argument value in the log message.
  - A fake that prints a full sentence ending in `(-1743)` gives the macOS
    message. A fake that exits with another code, or is killed by a signal,
    gives the general failure. A path to a file that is not executable gives the
    cannot-start failure. A fake that prints 1 MiB and exits 0 is a success.
  - A slow fake is killed at the limit and gives the timeout message, and its
    process group is gone. A cancel kills the process group.
  - Two parallel calls never run the fake at the same time.
  - The failure messages never contain the output text of the program.
- Native flow: `panel/e2e/run.mjs` builds `nookd` with `--features e2e` already.
  It also sets `NOOK_E2E_OSASCRIPT` and writes the fake script. `panel/e2e/
  fake_worker.py` gets a command that calls the tool, in the way of the file and
  command tools. A new spec in `panel/e2e/specs/` checks that the program ran only
  after Allow, that a deny leaves the file empty, and that a cancel before the
  approval does not run it. `panel/e2e/specs/tools.spec.mjs` counts four tool
  sources, and it counts five once the Notes source shows.
- Manual, on a Mac: one real note, to check the macOS prompt and its app name,
  the script, the title in the Notes list, the HTML, a title that starts with `-`,
  a title with non-ASCII characters, the run with the environment of rule 4, the
  refusal path, and the signing result from the "Signing check" section. No
  automated test touches the real Notes app.

### Commands

Each changed component runs its commands from `docs/testing.md`:

1. Daemon, from `daemon/`: `rtk cargo test` and `rtk cargo clippy --workspace
   --all-targets -- -D warnings`.
2. Contracts: `rtk uv run --with pytest --with jsonschema pytest tests/contracts
   panel/e2e/test_fake_worker.py`. No schema, example or message changes, so
   these only have to keep passing. `panel/e2e/test_fake_worker.py` covers the new
   fake Worker command.
3. Native: `rtk npm run test:e2e:deterministic` from `panel/`.

The Panel code and the web view do not change, so the Panel Rust commands and
the npm lint, unit and build commands do not apply. The Panel bundle settings
change, and the Mac build step checks them.

## Central docs that change

1. `docs/architecture.md`: the Notes tool runs in the Daemon as a child process
   outside the sandbox. Add it to "Tool boundary".
2. `docs/tools-and-authorization.md`: a "Notes tool" section with the tool, its
   approval actions, the Cedar action, the failure messages and the denial
   `Tool runs in the Worker` for a call where the tool is not offered. It also
   corrects the section "Tools that run in the Daemon": the table in "Where each
   tool runs" says which tools use the sandbox.
3. `docs/contracts.md`: the sentence that lists the tool source ids gets `notes`.
   No message changes.
4. `docs/testing.md`: the new Daemon tests, the fake `osascript`, the
   `NOOK_E2E_OSASCRIPT` setting of the `e2e` build and the new native spec.
5. `docs/release.md`: the Automation usage description and entitlement, and the
   note that ad hoc builds may ask again after an update.

Other files that change: `docs/adr/0008-panel-runs-desktop-app-tools.md` (renamed
to `docs/adr/0008-daemon-runs-desktop-app-tools-outside-the-sandbox.md`, rewritten
for this route, status Accepted, with its link to the draft removed; it keeps the
old reason against the Daemon, that the prompt may name the Daemon binary, as a
recorded consequence),
`docs/adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md` (one short
paragraph that names the Notes tool as the exception), `CHANGELOG.md`
("Unreleased"), `policy/tools.yaml`, `policy/cedar/` (schema and policies),
`daemon/crates/nook-core/src/providers/` (`notes.rs`, and `mod.rs` for
`build_registry`, `Carry`, `assemble` and the schema message),
`daemon/crates/nook-core/src/sandbox.rs` (`GroupKill`, `read_capped` and
`join_reader` become `pub(crate)`), `daemon/crates/nookd/src/main.rs` (the
program path and the `e2e` setting), `daemon/crates/nook-core/tests/common/mod.rs`
(the harness gets the new parameter), `panel/e2e/run.mjs`,
`panel/e2e/fake_worker.py`, a new spec in `panel/e2e/specs/`,
`panel/e2e/specs/tools.spec.mjs`, `panel/src-tauri/tauri.conf.json`,
`panel/src-tauri/Info.plist`, `panel/src-tauri/entitlements.plist` and
`scripts/bundle.sh`.

`docs/mcp-servers.md` does not change. `docs/plan.md` does not change. The
housekeeping task owns `CHECKLIST.md` and `docs/plans/`, so this task leaves
them (see "Risks").

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
   the `nookd` binary, which is a new gap. The owner tests this on a Mac. A
   `nookd` that someone starts by hand still offers the tool on a Mac, and the
   prompt names another app. That case is out of scope.
2. `osascript` may need an environment variable or a working folder that the
   closed environment of rule 4 removes. The manual test checks it. If it fails,
   the first fix is to add `HOME`, and that is a small fix to the provider.
3. The tool runs a program outside the sandbox. A flaw in the argument handling
   or in the HTML would run in a process with the rights of the user. The
   closed operation limits this: a fixed program, a constant script, one HTML
   argument that starts with `<`, a schema and a second check, Cedar, and an
   approval for every call. The tests use hostile titles and bodies.
4. A cancel or a timeout can leave a note behind, because the script may have
   finished before the Daemon kills it. Killing `osascript` does not stop Notes.
5. I cannot run Notes on Linux. The script, the HTML, the title rule, the
   environment and the macOS prompt are checked only in the manual test on a
   Mac. The owner will run it before the pull request merges. The reason that
   macOS refuses Apple events from a sandboxed process is also not checked.
6. An ad hoc signature changes per build, so macOS may ask again after each
   update. A signed release removes this, and it needs its own task.
7. The move of the draft leaves dangling paths to
   `docs/design/desktop-app-tools.md`. On `origin/main` they are in
   `CHECKLIST.md`, `docs/plans/app-integrations.md`,
   `docs/plans/tool-platform-open-gaps.md`, `docs/plans/tp7-tp13-tool-providers.md`
   and `docs/tasks/task-PR-42/plans/plan.md`. `docs/plans/app-integrations.md`
   also still names the Panel route as the choice. The housekeeping task owns the
   `docs/plans/` files and `CHECKLIST.md`, and the owner of PR 42 owns the last
   one. The orchestrator tells the owner, and a later small change updates the
   paths to `docs/tasks/task-PR-40/specs/design.md` and the route. The list must
   be checked again against `main` when this branch merges it.
8. `allow_for_chat` lets the model create notes for the rest of the chat with no
   new approval and no count limit.
