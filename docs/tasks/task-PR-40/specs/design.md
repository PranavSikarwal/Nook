# Spec: Notes tools run by the Daemon outside the sandbox

```
Stage: 3 spec review
Gate 1: pending
Gate 2: pending
```

Branch: `feat/notes-tool`. Pull request: #40.

This spec replaces the draft `docs/design/desktop-app-tools.md` of October 17,
2026, and this task deletes that file. The draft and the first versions of this
spec had the Panel run one tool that only creates a note. The owner changed the
design on November 14, 2026 in three steps:

1. The Daemon runs the operations itself, outside the command sandbox, because
   the Daemon is the core of Nook and the Panel is only the view (gap G4).
2. The tools create, list, read, append to, edit and delete notes inside one
   Notes folder (gap G2).
3. The user chooses that folder in the Settings tab (gap G1).

ADR 0008 records the decision, and this task rewrites it and changes its status
to Accepted.

## Why

The owner wants the assistant to work with the Notes app: write notes and keep
them up to date in a folder that the user chose. A Notes server from the
internet cannot do this inside the command sandbox, because macOS refuses Apple
events (the messages that one Mac app sends to another) from any process that
runs under a sandbox profile. So these tools cannot use the sandbox that the
command tool and the MCP servers use.

The Daemon already owns every tool call: it checks the arguments, asks Cedar,
shows the approval and runs the tool. This design keeps that whole path in the
Daemon. The Daemon starts `/usr/bin/osascript` itself, as a plain child process
with one fixed script for each operation. The Panel gets only a Settings field
for the folder name.

The draft before this one had clipboard, notification and open-link tools. The
owner removed them because the user can do each of them without the assistant.

## Scope

In scope: six tools, in the one Notes folder that the user chose (default
`Nook`) in the default Notes account. They work on macOS only.

| Tool | What it does | Arguments |
| --- | --- | --- |
| `nook:notes_create_note` | Creates a note. It makes the folder when it is missing. | `title`, `body` |
| `nook:notes_list_notes` | Lists the notes of the folder: id and title. At most 100. | none |
| `nook:notes_read_note` | Returns the title and the plain text of one note. | `note_id` |
| `nook:notes_append_note` | Adds text at the end of a note. | `note_id`, `note_title`, `text` |
| `nook:notes_edit_note` | Replaces the title and the text of a note. | `note_id`, `note_title`, `title`, `body` |
| `nook:notes_delete_note` | Deletes one note. | `note_id`, `note_title` |

`note_title` is the title that the model believes the note has. The approval
card shows it, because an id means nothing to the user, and the script refuses
the call when the real title differs.

Not in scope: notes outside the chosen folder, other accounts, moving a note,
creating or deleting folders (except that a create makes a missing folder),
attachments, checklists, tags, locked notes, Reminders, Calendar, Mail,
Messages, the clipboard, screen capture and any other app.

The Panel starts the Daemon. A Daemon that something else starts, such as
`nookctl` or a person in a terminal, is not a supported case for these tools.

This is not a release. No version file changes. The change adds one line under
"Unreleased" in `CHANGELOG.md`.

## Design

### Platforms

| Platform | Support | Reason |
| --- | --- | --- |
| macOS | Yes | Notes and Apple events exist. |
| Linux | No | No Notes app. The tools are not offered. |
| Windows | No | No Notes app. The tools are not offered. |

### Where each tool runs

This design needs an accurate picture of the sandbox, because the Notes tools
are the first tools that start a program outside it.

| Tool | Runs in | OS sandbox |
| --- | --- | --- |
| File tools | The Daemon, in process | No. The allowed folders and Cedar limit them. |
| Command tool and MCP servers | The Daemon, as a child process | Yes. Seatbelt on macOS, bubblewrap on Linux. |
| Web, news, video and book search, and web fetch | The Worker | No. |
| Notes tools (new) | The Daemon, as a child process | No. macOS refuses Apple events from a sandboxed process. |

The Daemon builds its tool list from a `ProviderRegistry` and sends it to the
Worker as `tool_catalog`. A tool that the Daemon owns reaches the Daemon as
`execute_tool`. The Notes tools are one more `Provider` in that registry, so the
Worker and the Worker schemas do not change.

### The call, step by step

A tool that the Daemon owns never uses `authorize_tool`. The Daemon denies that
message for such a tool with `Tool runs in the Daemon`.

1. The model calls a Notes tool. The Worker checks the arguments against the
   descriptor schema and sends `execute_tool`.
2. The Daemon checks the schema again, builds the facts, asks Cedar and shows
   the approval to the user. A deny ends the call with a fixed denial reason, as
   for any Daemon tool.
3. After an allow, the Daemon sends `tool_call_started`, runs
   `NotesProvider::call`, then sends `tool_call_finished` and `tool_result`.

### The folder setting

The folder name is `[tools.notes] folder` in `config.toml`, and the default is
`Nook`. The user changes it in the Settings, Tools tab, in a field that sits with
the `notes` source. The Daemon owns the value, and the Panel only shows it.

Three new messages in `contracts/panel-daemon.schema.json`, with examples and the
contract test, in the way of the folder list (`list_roots`, `add_root`):

1. `get_notes_settings` (Panel to Daemon) carries `id`. The answer is
   `notes_settings`.
2. `set_notes_folder` (Panel to Daemon) carries `id` and `folder`. The answer is
   `notes_settings`, or an `error` with the code `invalid_request` and a fixed
   message.
3. `notes_settings` (Daemon to Panel) carries `id`, `folder`, `supported` and an
   optional `reason` of 1 to 200 characters. `supported` is true when the
   platform has Notes (the program path is `Some`), so the Panel hides the field
   only on a platform without Notes. `folder` is the stored value cut to 200
   characters, even when it is not valid. `reason` is set when the tools are off. It is one of two fixed
   texts: `The Notes folder name is not valid` and `Notes was not found on this
   Mac`. So a user can still repair
   a bad name in the field.

A change takes effect at once and does not need a restart, as for the folders of
the file tools. It follows the same path as `folders.rs`, with one difference: the
whole sequence runs inside the write lock of `state.config`, and not only the
first step, because `folders.rs` runs its `settle` step after it releases the
lock, and that leaves a window. The sequence:

1. The Daemon checks the name, writes it to `config.toml` and keeps the comments
   and the other settings. A failed write gives a fixed error and changes
   nothing.
2. The Daemon denies every waiting approval of a Notes tool, with the reason `The
   Notes folder changed`, and drops every chat grant of a Notes tool. The grant
   drop is a new `GrantStore::remove_tools` method in `policy.rs` that removes
   the grants of the given tool ids. `deny_stale_approvals` denies only when
   `facts` fails, and after the change `facts` succeeds with the new folder, so a
   new function denies each pending approval whose tool is a Notes tool, whatever
   its facts say.
3. The Daemon builds the Notes tools again with `rebuilt`, replaces the registry
   and sends the Worker a new tool catalog. The tool descriptions name the
   folder, so the model sees the new one.

Steps 2 and 3 alone leave races, because a call can read the registry, the
grants and the approvals between them. Four rules close them:

1. Both paths that start a Notes call take the read lock of `state.config`:
   `handle_authorize_tool` (and `execute_tool`) from `normalize_with` until the
   call has its approval entry or, when a chat grant allows it, until it is
   delivered, and the decision path from the comparison of step 2 until the
   `Execution` is stored. Under the lock they read the registry once, build the
   facts and the Cedar answer from that registry, check the grants, and store
   that registry in the `Execution`. The change holds the write lock for its
   whole sequence, so each call sees either the old registry with the old grants,
   or the new registry with the grants dropped. The read lock is released before
   the Daemon waits for the user or runs the tool. No code under the read lock may
   read `state.config` again, because a tokio lock is not re-entrant and a waiting
   writer would then deadlock it.
2. `Execution` gets a field `registry: Arc<ProviderRegistry>`, and
   `start_execution` calls that registry. Today it calls `state.providers.call`,
   which reads the current registry again after the lock is gone, so a change in
   between would run the call in the new folder.
3. Each pending Notes approval stores the folder that its card showed. When a
   decision arrives, under the read lock the Daemon compares the stored folder
   with the folder of the current registry, records the grant and stores the
   registry. A mismatch denies the call with `The Notes folder changed`, and no
   grant is recorded.
4. The new function that denies pending Notes approvals skips an entry that has
   `decision_in_flight`, as `deny_stale_approvals` does, because that decision
   compares the folder itself under the read lock. Denying it would send the
   Worker two results for one call. A decision that is already claimed holds the
   read lock until its `Execution` has the registry, so the change waits for it,
   and the call that follows runs with the old registry and the old folder, as its
   card said.

A call that already runs finishes with the folder it started with.

The folder name is valid when, after the Daemon removes spaces at both ends, it
has 1 to 100 characters, no control character and no `:`, and it is not `Recently
Deleted` in any letter case. The same check runs when the Daemon reads
`config.toml`. A name that is not valid leaves the Notes tools off, and the
source `notes` shows the reason `The Notes folder name is not valid`. The Daemon
never falls back to the default, because it must fail closed. The user can point
the tools at the main `Notes` folder. That is the user's choice, and the approval
card names the folder for every call.

### Offering the tools

`providers/notes.rs` holds `NotesProvider`, a `Provider` behind
`ProviderRegistry`. Its constructor takes the path of the program to run and the
folder name.

`nookd` decides whether the platform has Notes, and `nook-core` has no platform
or feature test for it:

1. `build_registry(tools, notes_program: Option<PathBuf>)` gets a new parameter.
   The folder comes from `tools.notes.folder`.
2. `nookd` passes `Some("/usr/bin/osascript")` on macOS. In an `e2e` build it
   passes the path in `NOOK_E2E_OSASCRIPT` on every platform. Anywhere else it
   passes `None`.
3. `Carry` in `providers/mod.rs` holds a clone of `notes_program`, so `rebuilt`
   after a folder change keeps the tools.
4. `assemble` adds `NotesProvider` when the path is `Some`, the file exists and
   the folder name is valid. When the path is `Some` and the file is missing, or
   the name is not valid, the registry reports the source as `off` with a
   reason, so the Tools tab says why, and the Daemon logs it once. The Daemon
   never runs `osascript` to test it, because a test run would show the macOS
   prompt.
5. `mod notes` has the same `#[cfg(unix)]` gate as `mcp` in `providers/mod.rs`,
   because the Windows job compiles the whole workspace.

The provider reports its own source: id `notes`, display name "Notes tools",
state `ready`, six tools. `ToolSourceInfo.source_id` is any non-empty string in
the schema, so no schema changes for it. `docs/contracts.md` lists the ids in one
sentence, and that sentence gets the new id.

`tools.yaml` lists the tools on every platform, but the registry holds them only
where the path is `Some`. A call for one of them elsewhere is denied with `Tool
runs in the Worker` (the check at `tool_approval.rs`), and a denial ends the
reply. The model never sees the tools there, so the call needs a forged message.
The docs say this.

### The tools

Each tool is its own entry in `tools.yaml` and its own Cedar action. The
descriptor schema in the Daemon checks the types and the sizes before Cedar.
Every tool sets `required` for its arguments and `additionalProperties: false`.
The schemas have no `pattern`. The Worker checks arguments with Python
`jsonschema`, where `$` also matches before a final line feed, and the Daemon
uses the Rust crate, where it does not. So one set of functions in `notes.rs`
checks the content, in Rust only, and `facts` and `call` both use them:

1. `title` and `note_title`: 1 to 200 characters. Remove one trailing line break
   (`\n` or `\r\n`). Refuse a value that is now empty or that still has a line
   break (`\r` or `\n`).
2. `body`: up to 20,000 characters of plain text. It may be empty.
3. `text`: 1 to 20,000 characters of plain text.
4. `note_id`: 14 to 300 characters. It starts with `x-coredata://` and holds
   only letters, digits and `:/._%-`. A value that starts with another character
   could be read as an option by `osascript`.
5. A NUL character in any value is refused. It passes JSON and the schema, but it
   cannot go into a process argument.

A refusal gives the fixed reason `Invalid arguments`, before any approval is
shown.

Each tool returns a fixed text with the facts of the call, or an error:

| Tool | Result |
| --- | --- |
| create | `Created the note "<title>" in the Notes folder "<folder>". Its id is <id>.` The id comes from the output of the script and must pass the `note_id` check, or the text leaves it out. |
| list | `Notes in the folder "<folder>":` and one line for each note, the id, a tab and the title. The script cuts a title at 200 characters. `No notes` for an empty or missing folder. At most 100 lines, and a last line `More notes exist.` when it cuts. |
| read | The title, an empty line and the plain text. A note over 20,000 characters is cut with a last line `[Cut at 20000 characters]`. |
| append | `Added the text to the note "<note_title>".` |
| edit | `Changed the note "<note_title>".` |
| delete | `Deleted the note "<note_title>".` |

The text from Notes in the list and read results is data and is never an
instruction. Each tool description tells the model so, and says that the title is
one line (the constant `The title is one line.` in `notes.rs`, used by the
create and edit tools). The descriptions also name the folder and say that the
tools exist on macOS only. A test checks the constant in the descriptors.

### Run in the Daemon

The check functions run at the start of `NotesProvider::call`, because `call` is
the last step before a process starts. If one refuses, `call` returns `Error: the
arguments are not valid` and starts no process.

The provider builds the HTML next, in one small function that tests call
directly. It escapes `&`, `<` and `>`, and it puts each line in a `div`. Notes
takes the title of a note from the first line of its body, so for create and edit
the first `div` holds the escaped title as a heading, and the body lines follow.
For append the HTML holds only the lines of `text`. The manual test checks the
title in the Notes list.

The process gets these arguments, with one script for each operation:

```
/usr/bin/osascript -e <SCRIPT> <ARG> ...
```

Every `ARG` is a data value with one `:` in front of it. The script removes the
first character. So no value can start with `-` and be read as an option, and the
HTML needs no special first character. The data arguments of each tool are:

| Tool | Arguments after the script |
| --- | --- |
| create | folder, html |
| list | folder |
| read | folder, note id |
| append | folder, note id, note title, html |
| edit | folder, note id, note title, html |
| delete | folder, note id, note title |

The scripts are these constants in `notes.rs`. The plan copies them exactly. They
use `item n of argv` as string values, with no `do shell script`, `run script`,
`load script` or `System Events`. The Mac test runs each one, and a script can
change only after a review of this spec.

Create:

```
on run argv
  set folderName to text 2 thru -1 of (item 1 of argv)
  set noteHtml to text 2 thru -1 of (item 2 of argv)
  tell application "Notes"
    tell default account
      if not (exists folder folderName) then make new folder with properties {name:folderName}
      set newNote to make new note at folder folderName with properties {body:noteHtml}
      return id of newNote
    end tell
  end tell
end run
```

List:

```
on run argv
  set folderName to text 2 thru -1 of (item 1 of argv)
  set rows to {}
  tell application "Notes"
    tell default account
      if not (exists folder folderName) then return ""
      set theNotes to notes of folder folderName
      set n to count of theNotes
      if n > 100 then set n to 101
      repeat with i from 1 to n
        set theNote to item i of theNotes
        set nm to name of theNote
        if (length of nm) > 200 then set nm to text 1 thru 200 of nm
        set end of rows to (id of theNote) & tab & nm
      end repeat
    end tell
  end tell
  set AppleScript's text item delimiters to linefeed
  set out to rows as text
  set AppleScript's text item delimiters to ""
  return out
end run
```

The list script asks for 101 notes. The Daemon keeps 100 lines, and a 101st line
makes it add `More notes exist.`.

Read:

```
on run argv
  set folderName to text 2 thru -1 of (item 1 of argv)
  set noteId to text 2 thru -1 of (item 2 of argv)
  tell application "Notes"
    tell default account
      if not (exists folder folderName) then error "no folder" number 9001
      set theNote to note id noteId
      if (id of container of theNote) is not (id of folder folderName) then error "not in folder" number 9001
      set t to plaintext of theNote
      if (length of t) > 20000 then set t to (text 1 thru 20000 of t) & linefeed & "[Cut at 20000 characters]"
      return (name of theNote) & linefeed & linefeed & t
    end tell
  end tell
end run
```

Append, with the folder, the id, the title and the HTML as arguments 1 to 4:

```
on run argv
  set folderName to text 2 thru -1 of (item 1 of argv)
  set noteId to text 2 thru -1 of (item 2 of argv)
  set noteTitle to text 2 thru -1 of (item 3 of argv)
  set addHtml to text 2 thru -1 of (item 4 of argv)
  tell application "Notes"
    tell default account
      if not (exists folder folderName) then error "no folder" number 9001
      set theNote to note id noteId
      if (id of container of theNote) is not (id of folder folderName) then error "not in folder" number 9001
      considering case
        if (name of theNote) is not noteTitle then error "title" number 9002
      end considering
      set body of theNote to (body of theNote) & addHtml
      return "ok"
    end tell
  end tell
end run
```

Edit, with the folder, the id, the title and the new HTML as arguments 1 to 4.
The new HTML holds the new title and the new body:

```
on run argv
  set folderName to text 2 thru -1 of (item 1 of argv)
  set noteId to text 2 thru -1 of (item 2 of argv)
  set noteTitle to text 2 thru -1 of (item 3 of argv)
  set newHtml to text 2 thru -1 of (item 4 of argv)
  tell application "Notes"
    tell default account
      if not (exists folder folderName) then error "no folder" number 9001
      set theNote to note id noteId
      if (id of container of theNote) is not (id of folder folderName) then error "not in folder" number 9001
      considering case
        if (name of theNote) is not noteTitle then error "title" number 9002
      end considering
      set body of theNote to newHtml
      return "ok"
    end tell
  end tell
end run
```

Delete, with the folder, the id and the title as arguments 1 to 3:

```
on run argv
  set folderName to text 2 thru -1 of (item 1 of argv)
  set noteId to text 2 thru -1 of (item 2 of argv)
  set noteTitle to text 2 thru -1 of (item 3 of argv)
  tell application "Notes"
    tell default account
      if not (exists folder folderName) then error "no folder" number 9001
      set theNote to note id noteId
      if (id of container of theNote) is not (id of folder folderName) then error "not in folder" number 9001
      considering case
        if (name of theNote) is not noteTitle then error "title" number 9002
      end considering
      delete theNote
      return "ok"
    end tell
  end tell
end run
```

There are six scripts and no fragment. The plan copies each one exactly.

Eight rules keep the operations closed. The model supplies data and never code or
an option.

1. The program is the path that the constructor got. A release build gets
   `/usr/bin/osascript` from `nookd`.
2. The arguments are the list above. The scripts are constants, and the text of
   the model is never part of a script.
3. Every data argument has the `:` prefix, so `osascript` cannot read it as an
   option. The provider passes no raw value on the command line.
4. The environment holds only `LANG=en_US.UTF-8`, so `osascript` reads a title
   with non-ASCII characters as UTF-8. The working directory is `/`. There is no
   standard input.
5. The provider runs one `osascript` at a time, for every operation, so two
   parallel first calls cannot both create the folder. The lock is one
   `Arc<tokio::sync::Mutex<()>>` in `Carry`, which `assemble` gives to each new
   `NotesProvider`. A lock inside the provider would be new after each folder
   change, and a call that started on the old registry could then run next to a
   call on the new one.
6. The process runs in its own process group (`process_group(0)`). The provider
   reuses `GroupKill`, `read_capped` and `join_reader` from `sandbox.rs`, which
   become `pub(crate)` and `#[cfg(unix)]`. `GroupKill` is a tuple struct with a
   private field, so its field becomes `pub(crate)` too. The code in `assemble`
   that builds `NotesProvider` has the same `#[cfg(unix)]` gate as the code for
   MCP. The provider does not copy the timeout and read loop. A cancel aborts the
   task, so no code after an `await` runs, and the group kill sits in the
   `GroupKill` drop guard.
7. The limit is 60 seconds. The provider keeps at most 4096 bytes of standard
   output for create, append, edit and delete. For list and read it keeps
   `MAX_TOOL_OUTPUT_BYTES + 1` bytes (64 KiB and one byte, from `tool_approval.rs`),
   so the limit is one value. The Daemon already cuts every tool result at
   `MAX_TOOL_OUTPUT_BYTES` and adds `[output truncated]` (`truncate_output` in
   `providers/mod.rs`), so a longer list or read is cut and marked there and the
   provider adds no marker of its own. The extra byte makes sure that `truncate_output`
   sees a result that is too long, because the provider drops the rest of the
   pipe. The provider also drains the rest of each pipe, so a long output cannot
   block `osascript`. A list of 100 notes fits in the limit unless the notes have
   very long ids and titles, and a read of 20,000 characters can pass it when the
   text has many 4-byte characters (80,000 bytes). Both cases are cut and marked.
   It keeps 4096 bytes of standard error. The text from `osascript` is read as UTF-8
   and bad bytes are replaced. `read_capped` sets `truncated` only when the output
   is longer than its limit.
8. Success is exit status 0. Failure is decided by the kept standard error: a
   refusal by macOS contains `(-1743)`, a missing note or a note outside the
   folder contains `(-1728)` or `(9001)`, and a wrong title contains `(9002)`.
   Every other result is a failure. The model sees only the fixed texts of the
   failure table, and the output of a list or a read. The Daemon logs the exit
   status and the AppleScript error number, never the output text, because a
   script error can contain part of a note.

Cedar keeps its rule that forbids `osascript` for the command tool. The Notes
tools do not use the command tool.

### Approval and policy

The approval rules live in the policy files, not in Rust. Each Notes tool has an
entry in `policy/tools.yaml` with its tier and its approval actions, and a Cedar
action with its rules in `policy/cedar/`. Changing an approval later is a change
to these files and needs no Rust change. The Panel draws its buttons from the
YAML. The user can make a tool stricter in the Permissions tab, never looser.

| Tool | Tier | Actions in `tools.yaml` | Cedar permits a grant of scope |
| --- | --- | --- | --- |
| list, read, create, append, edit | `ask` | `allow_once`, `allow_for_chat`, `deny` | `once`, `chat` |
| delete | `ask` | `allow_once`, `deny` | `once` |

There is no `always_allow`, because every call touches the account of the user.
Delete has no `allow_for_chat`, so each delete asks. `APPROVAL_ACTIONS` in
`policy_assets.rs` already holds these three ids, so that file does not change.
`allow_for_chat` has no count limit per chat (see Risk 8).

The approval shows the operation and the folder, and then:

| Tool | The approval shows |
| --- | --- |
| create | The title, and the first 200 characters of the body with its total length. |
| list | Nothing more. |
| read | The note id. |
| append | The note title, and the first 200 characters of the text with its total length. |
| edit | The note title, the new title, and the first 200 characters of the new body with its total length. |
| delete | The note title. |

`NotesProvider::facts` builds these summaries. Cedar gets the action of the tool
with the context values `chat_id`, `folder`, `approval_tier`, `grant_scope` and
`argument_digest`. The rules check the grant scope. They cannot compare `folder`
with a constant, because the user sets it, so the Daemon checks the folder name
when it is set and when it is read.

The draft also had Cedar forbid a long body and a title with a line break. The
schema and the checks in `notes.rs` already refuse both before Cedar, and the
context of Cedar holds only strings. One check in one place is simpler, so Cedar
keeps only the grant rules.

### Permission from macOS

The first call makes macOS ask whether Nook may control Notes. The user can
refuse. A refusal returns `Error: macOS did not allow Nook to control Notes`
(AppleScript error -1743 in the standard error of `osascript`), and macOS does
not ask again until the user changes the setting in System Settings, Privacy and
Security, Automation. The Settings field shows a line that says this.

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
   text "Nook needs to control Notes so that the assistant can work with the
   notes that you approve." Tauri merges a file with this name into the bundle
   `Info.plist`.
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

The Daemon stores the arguments of each call and the result in the chat
transcript, as for any tool. A list or a read puts note text into the
transcript, and the Daemon sends it to the model endpoint like any other text
of the chat. That is why these tools ask before each use, and the approval names
the folder. The Daemon logs the operation name and the status, never a title, a
body, a note id or note text.

One path logs a value today. `ProviderRegistry::facts` builds `Invalid
arguments: {error}` from the `jsonschema` error, and that error text prints the
refused value. `tool_approval.rs` logs it. This task changes that message so it
names the keyword and the property and never the value. The change also protects
the file tools. A test checks the message text.

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
| Platform is not macOS (release build) | `nookd` passes `None`. The tools are not offered. |
| `/usr/bin/osascript` does not exist | The tools are not offered. The Tools tab shows the source `notes` as `off` with a reason, and the Daemon logs it once. |
| The folder name in `config.toml` is not valid | The tools are not offered. The source shows `The Notes folder name is not valid`. |
| `set_notes_folder` with an invalid name | The Daemon answers with an `error` and changes nothing. |
| A call for a tool in `tools.yaml` that the registry lacks | The Daemon denies it with `Tool runs in the Worker`, and the reply ends. The docs say it. |
| User denies the approval | A fixed denial reason, as for any tool. |
| Arguments break the schema | The Daemon refuses the call before any prompt, with the fixed reason `Invalid arguments` and a log line without the value. |
| A title ends with one line break | The provider removes it and goes on. |
| A title is empty or has another line break, a `note_id` is not valid, or a value has a NUL | The provider refuses the call before any prompt, with `Invalid arguments`. |
| `call` finds invalid arguments | `Error: the arguments are not valid`. No process starts. |
| macOS refuses Automation | `Error: macOS did not allow Nook to control Notes`. |
| The note does not exist or is not in the folder | `Error: that note was not found in the Notes folder`. |
| The note title does not match | `Error: the note title does not match`. |
| Notes does not answer within 60 seconds | `Error: Notes did not answer`. The Daemon kills the process group. |
| `osascript` cannot start | `Error: Notes could not complete the request`. |
| Any other failure (non-zero exit, killed by a signal, a locked note) | `Error: Notes could not complete the request`. The Daemon logs the exit status and the error number. |
| The program writes more than the output limit | The provider keeps the limit (for list and read, one byte more than `MAX_TOOL_OUTPUT_BYTES`), drains the rest and judges only the exit status. A list or a read is cut at `MAX_TOOL_OUTPUT_BYTES` and ends with `[output truncated]`. |
| The call is cancelled | The drop guard kills the process group. A change can still exist if the script had finished. |
| The folder changes while a call waits for approval | The Daemon denies the approval with `The Notes folder changed`. |
| A decision arrives for a card that names the old folder | The Daemon denies the call with `The Notes folder changed` and records no grant. |
| `set_notes_folder` cannot write `config.toml` | The Daemon answers with a fixed `error` (`The folder could not be saved`) and changes nothing, as `folders.rs` does. |

### Tests

- Policy: each tool needs a grant of scope `once` or `chat`, and delete needs
  scope `once`. No permanent grant is offered, and delete offers no chat grant in
  `tools.yaml`. The real policy files are used.
- Registry, with a fake program path: `build_registry(tools, None)` has no Notes
  tool. With a path to a fake program it has the six tools and the source `notes`.
  A path to a missing file gives the source `off` with the reason `Notes was
  not found on this Mac`, and an invalid folder name gives the reason `The Notes
  folder name is not valid`. `rebuilt` after a folder change keeps the tools and the same
  lock.
- Registry, schema message: the string that `ProviderRegistry::facts` returns for
  a refused schema holds the keyword and the property and never the value. The
  test runs it for a Notes tool and for a second provider (the `Fixed` provider
  of the unit tests), so the shared path is pinned. It uses the keyword and the
  path of the error, and never its `kind`, because the `kind` of an extra
  property holds the property name that the model chose.
- Provider, called directly: `Provider::call` with invalid arguments returns
  `Error: the arguments are not valid` and starts no process, because the socket
  tests stop at `facts`. The check functions have tests for each rule of "The
  tools".
- Policy store: `GrantStore::remove_tools` removes the chat grants of the given
  tool ids and no other grant.
- Folder change, races: a pending approval of another tool stays open and a grant
  of another tool stays after a folder change. A decision that arrives for a card
  with the old folder is denied and records no grant. The harness owns
  `AppState` (`tests/common/mod.rs`), so the tests force the order without
  sleeps: the test holds `state.config.write()`, sends the decision, which waits at
  the read lock, makes the change in memory (folder, registry, deny, drop),
  releases the guard, and expects the denial and no grant. For the other order, the
  decision finishes first, then the change runs, and the fake receives the old
  folder. A call that a chat grant allows follows the same two orders, and it never
  runs in the new folder with a grant for the old one. A failed write of
  `config.toml` returns the fixed error and leaves the registry, the approvals and
  the grants as they were.
- Folder setting, through the real socket: `get_notes_settings` returns the
  folder, `supported` and `reason`. A bad folder name in `config.toml` gives the
  stored value, `supported` true and the fixed `reason`, and `set_notes_folder`
  with a good name repairs it. With the program path `None`, `supported` is
  false. `set_notes_folder` writes `config.toml`, keeps its
  comments, rebuilds the tools, sends a new catalog and denies every waiting
  approval of a Notes tool with `The Notes folder changed`, also when `facts`
  would succeed with the new folder. A chat grant of a Notes tool does not
  survive the change. An invalid name returns an `error` and changes nothing. A
  second change while a call runs does not break that call.
- Daemon, in the new file `daemon/crates/nook-core/tests/notes_tool_tests.rs`,
  which starts with `#![cfg(unix)]` like its sibling test files, so the Windows
  job still compiles. The tests use the real socket and the fake Worker, with a
  fake `osascript` (a script that writes its arguments, its working folder
  (`pwd -P`) and its exit code to a file, and prints a chosen output). They run on
  Linux and on macOS:
  - The program runs only after an allow. A deny leaves the file empty. A cancel
    before the approval does not run it.
  - Each tool gets the exact arguments of its row in "Run in the Daemon", each
    with the `:` prefix, and the exact script constant. The script is byte for
    byte the same for two calls with different values. The working folder is `/`.
  - A folder, a title or a note id such as `-e` or `-l` arrives with the `:`
    prefix and not as an option.
  - A body with quotes, backslashes, `$(...)` and a line break reaches the
    program as plain text in the HTML argument.
  - The HTML conversion escapes `&`, `<` and `>`.
  - The titles `Title\n` and `Title\r\n` become `Title`, and the trimmed title is
    what the approval summary, the HTML and the result text hold. The titles
    `\n`, `Title\n\n` and `Ti\ntle` start no process and show no prompt. A NUL in
    any value does the same. A body that breaks the schema starts no process. A
    `note_id` without the prefix `x-coredata://`, or with another character,
    starts no process.
  - A body of 6,000 line breaks reaches the program in full. The HTML is under
    100,000 bytes, even at 15 bytes for each empty line, which is under the Linux
    limit of 131,072 bytes for one argument. The macOS limit is higher (1 MiB for
    all arguments and the environment, not checked), and the Mac test sends the
    largest body, 20,000 line breaks, which is about 220,000 bytes.
  - The descriptor descriptions contain the constant `The title is one line.` in
    the create and edit tools, and they name the folder.
  - A fake that prints a full sentence ending in `(-1743)` gives the macOS
    message. One that ends in `(-1728)` or `(9001)` gives the not-found message,
    and `(9002)` gives the title message. A fake that exits with another code, or
    is killed by a signal, gives the general failure. A path to a file that is not
    executable gives the cannot-start failure.
  - A fake that prints 1 MiB and exits 0 is a success for create, append, edit
    and delete. The limit counts the final result text. A read of exactly
    `MAX_TOOL_OUTPUT_BYTES` shows no marker. For a list the fake prints the limit
    minus the header `Notes in the folder "<folder>":` and its line break, which
    shows no marker, and one byte more ends with `[output truncated]`, and a four-byte
    character at the cut leaves valid UTF-8. A list with 101 lines shows 100 and
    `More notes exist.` A read over 20,000 characters shows the cut line of the
    script.
  - A slow fake is killed at the limit and gives the timeout message, and its
    process group is gone. A cancel kills the process group.
  - Two parallel calls never run the fake at the same time, also when a folder
    change replaces the registry between them.
  - The failure messages never contain the output text of the program.
  - The environment of the program is exactly `LANG=en_US.UTF-8`. This test has
    its own `#[cfg(target_os = "linux")]`, because its fake reads
    `/proc/$$/environ`. `Code Quality and Tests` runs it on Linux. A developer on
    a Mac skips it, and the Mac test covers the environment.
- Contracts: the schema, the examples and `tests/contracts/test_contracts.py`
  cover `get_notes_settings`, `set_notes_folder` and `notes_settings`, with
  negative cases: `set_notes_folder` with an empty or a missing `folder`, and
  `notes_settings` without `supported`.
- Panel web view: `rtk npm run test:unit` runs `node --test` on pure functions,
  because the project has no test renderer for components. The logic of the field
  moves into `panel/src/lib/notesFolder.ts`, and `panel/e2e/unit/notes-folder.test.mjs`
  tests it: the field is hidden when `supported` is false, it shows the stored
  folder and the `reason`, and Save is on only when the value changed and is not
  empty after trimming. `panel/src/lib/daemon.ts` gets the browser mock for the
  two requests. The rendered field is covered by the native spec.
- Native flow: `panel/e2e/run.mjs` builds `nookd` with `--features e2e` already.
  It also sets `NOOK_E2E_OSASCRIPT` and writes the fake script. `panel/e2e/
  fake_worker.py` gets a command that calls the create and delete tools, in the
  way of the file and command tools. A new spec in `panel/e2e/specs/` checks that
  the program ran only after Allow, that a deny leaves the file empty, that a
  cancel before the approval does not run it, that the delete approval has no
  chat button, and that a new folder from the Settings field reaches the next
  call. `panel/e2e/specs/tools.spec.mjs` counts four tool sources, and it counts
  five once the Notes source shows. `panel/e2e/test_fake_worker.py` gets one test
  for the new fake Worker command, as each of the other commands has one.
- Manual, on a Mac: each script on real notes, to check the macOS prompt and its
  app name, the title in the Notes list, the HTML, the note ids (they start with
  `x-coredata://`), list, read, append, edit and delete, a delete (and whether
  Notes keeps the note in Recently Deleted), a title that starts with `-`, a
  folder name with non-ASCII characters or a leading `-`, a body of 20,000 line
  breaks, a note over 20,000 characters, append to a note that has a list or an
  image, a title with a line break that was typed with Shift and Return, a wrong
  title, a note in another folder, a locked note, the run with the
  environment of rule 4, the refusal path, the Settings field, and the signing
  result from the "Signing check" section. No automated test touches the real
  Notes app.

### Commands

Each changed component runs its commands from `docs/testing.md`:

1. Daemon, from `daemon/`: `rtk cargo test` and `rtk cargo clippy --workspace
   --all-targets -- -D warnings`.
2. Contracts: `rtk uv run --with pytest --with jsonschema pytest tests/contracts
   panel/e2e/test_fake_worker.py`. The new test of the fake Worker command in
   `panel/e2e/test_fake_worker.py` is part of them.
3. Panel web view, from `panel/`: `rtk npm run lint`, `rtk npm run test:unit` and
   `rtk npm run build`.
4. Native: `rtk npm run test:e2e:deterministic` from `panel/`.

The Panel Rust side gets two thin Tauri commands that call `ask_daemon`, and
they hold no logic, as the other commands in the pattern. The job `Code Quality
and Tests` already runs `cargo check` in `panel/src-tauri`, and it compiles them.
No Panel Rust test is added.

## Central docs that change

1. `docs/architecture.md`: the Notes tools run in the Daemon as a child process
   outside the sandbox, and the Settings field. Add them to "Tool boundary".
2. `docs/tools-and-authorization.md`: a "Notes tools" section with the six
   tools, the folder setting, their approval actions, the Cedar actions and the
   failure messages, and the denial `Tool runs in the Worker` for a call where
   the tools are not offered. It also corrects the section "Tools that run in the
   Daemon": the table in "Where each tool runs" says which tools use the sandbox.
3. `docs/contracts.md`: the new messages `get_notes_settings`,
   `set_notes_folder` and `notes_settings`, and the sentence that lists the tool
   source ids gets `notes`.
4. `docs/testing.md`: the new Daemon, Panel and native tests, the fake
   `osascript` and the `NOOK_E2E_OSASCRIPT` setting of the `e2e` build.
5. `docs/release.md`: the Automation usage description and entitlement, and the
   note that ad hoc builds may ask again after an update.

Other files that change: `docs/adr/0008-panel-runs-desktop-app-tools.md` (renamed
to `docs/adr/0008-daemon-runs-desktop-app-tools-outside-the-sandbox.md`, rewritten
for this route, status Accepted, with its link to the draft removed; it keeps the
old reason against the Daemon, that the prompt may name the Daemon binary, as a
recorded consequence),
`docs/adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md` (one short
paragraph that names the Notes tools as the exception), `CHANGELOG.md`
("Unreleased"), `policy/tools.yaml` (six entries), `policy/cedar/` (schema and
policies, six actions), `contracts/panel-daemon.schema.json` and
`contracts/examples/panel-daemon.ndjson`, `tests/contracts/test_contracts.py`
(negative cases: `set_notes_folder` with an empty or a missing `folder`, and
`notes_settings` without `supported`),
`daemon/crates/nook-core/src/providers/` (`notes.rs`, and `mod.rs` for
`build_registry`, `Carry`, `assemble` and the schema message),
`daemon/crates/nook-core/src/config.rs` (`[tools.notes]`),
`daemon/crates/nook-core/src/protocol.rs` and `server.rs` (the two requests), a
new `daemon/crates/nook-core/src/notes_folder.rs` in the way of `folders.rs`,
`daemon/crates/nook-core/src/tool_approval.rs` (`PendingApprovalEntry` with the
folder, `Execution` with the registry, `handle_authorize_tool`,
`handle_approval_decision`, `start_execution`, and the function that denies the
waiting Notes approvals), `daemon/crates/nook-core/src/policy.rs`
(`GrantStore::remove_tools`), `daemon/crates/nook-core/src/sandbox.rs` (`GroupKill`, `read_capped` and
`join_reader` become `pub(crate)`), `daemon/crates/nookd/src/main.rs` (the
program path and the `e2e` setting), `daemon/crates/nook-core/tests/common/mod.rs`
(the harness gets the new parameter), the new
`daemon/crates/nook-core/tests/notes_tool_tests.rs`, `panel/src/lib/daemon.ts`
and `panel/src/lib/types.ts`, `panel/src/lib/notesFolder.ts`,
`panel/e2e/unit/notes-folder.test.mjs`, `panel/src/components/ToolsTab.tsx` (the
field), `panel/src-tauri/src/lib.rs` (two thin commands),
`panel/e2e/run.mjs`, `panel/e2e/fake_worker.py`, `panel/e2e/test_fake_worker.py`,
a new spec in `panel/e2e/specs/`, `panel/e2e/specs/tools.spec.mjs`,
`panel/src-tauri/tauri.conf.json`, `panel/src-tauri/Info.plist`,
`panel/src-tauri/entitlements.plist` and `scripts/bundle.sh`.

`docs/mcp-servers.md` does not change. `docs/plan.md` does not change. The
housekeeping task owns `CHECKLIST.md` and `docs/plans/`, so this task leaves
them (see "Risks").

## Rule changes

None. This task changes no file under `.agents/rules/`, `AGENTS.md`,
`CLAUDE.md` or `CONTRIBUTING.md`.

## Open gaps

`open-gaps.md` holds six questions, each with options and a recommendation. G1
to G3 come from the draft. G4 came from spec review round 1. The owner decided
all six on November 14, 2026.

1. G1: which folder the tools use. Decided by the owner: the user chooses it in
   a Settings field, and the value is `[tools.notes] folder` in `config.toml`.
2. G2: which operations the tools offer. Decided by the owner: list, read,
   create, append, edit and delete, inside the chosen folder.
3. G3: the Automation entitlement. The check is in "Signing check" above.
4. G4: where the operation runs. Decided by the owner: the Daemon runs it,
   outside the sandbox.
5. G5: where the approval rules live. Decided by the owner: in `tools.yaml` and
   Cedar, as for every tool today.
6. G6: which Notes tools offer `allow_for_chat`. Decided by the owner: every tool
   except delete.

## Risks

1. macOS may blame the wrong app in the Automation prompt. The Panel starts the
   Daemon, so I expect the prompt to name Nook, but I cannot check it on Linux.
   If it names `nookd` or shows no prompt, the usage description must go into
   the `nookd` binary, which is a new gap. The owner tests this on a Mac. A
   `nookd` that someone starts by hand still offers the tools on a Mac, and the
   prompt names another app. That case is out of scope.
2. `osascript` may need an environment variable or a working folder that the
   closed environment of rule 4 removes. The manual test checks it. If it fails,
   the first fix is to add `HOME`, and that is a small fix to the provider.
3. The tools run a program outside the sandbox. A flaw in the argument handling
   or in the HTML would run in a process with the rights of the user. The
   closed operations limit this: a fixed program, constant scripts, data
   arguments with a `:` prefix, a schema and a second check, Cedar, and an
   approval for every call. The tests use hostile values.
4. A cancel or a timeout can leave a change behind, because the script may have
   finished before the Daemon kills it. Killing `osascript` does not stop Notes.
5. I cannot run Notes on Linux. The scripts, the HTML, the title rule, the
   environment and the macOS prompt are checked only in the manual test on a
   Mac. The owner will run it before the pull request merges. Several
   AppleScript facts are not checked: that `plaintext` and `note id` work in the
   places that the scripts use them, that `container of` gives the folder, that
   `body` takes HTML, and that a `delete` moves the note to Recently Deleted. The
   reason that macOS refuses Apple events from a sandboxed process is also not
   checked.
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
8. `allow_for_chat` lets the model use the tools for the rest of the chat with
   no new approval and no count limit, except for delete, which always asks. A
   chat grant for edit lets a steered model replace notes in the folder with no
   new approval, and an edit can lose a list or an image (Risk 10). The owner
   chose this in gap G6. The owner can take `allow_for_chat` from edit and append
   in `tools.yaml` without a Rust change (gap G5).
9. A note can hold text that tries to give the model an instruction (prompt
   injection). A read or a list puts that text in front of the model. The tool
   descriptions say that note text is data. A delete always asks. The other
   calls ask too, unless the user chose `allow_for_chat` for the chat (Risk 8).
10. An edit replaces the whole note, so a list, an image or an attachment in it
    can be lost, and an append rewrites the HTML of the note. The title check
    and the approval limit a wrong target, but not a lost format. The Mac test
    checks an append to a note that has a list or an image.
11. The user can name the main `Notes` folder. Then the tools can read, change
    and delete every note in it. That is the user's choice. The approval card
    names the folder for each call, and a delete always asks.
