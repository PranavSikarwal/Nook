# Desktop app tools design: Notes

Status: proposed. No code exists. Implementation starts only after approval.

## Why

The owner wants the assistant to write to the Notes app. A Notes server from the
internet cannot do this inside the command sandbox, because macOS refuses Apple
events from any sandboxed process (see `../plans/app-integrations.md`). The Panel
is a normal desktop app, so it can send Apple events when the user allows it. The
Panel runs the tool, and the Daemon authorizes each call.

This design replaces an earlier draft with clipboard, notification, and open-link
tools. The owner removed them because the user can do each of them without the
assistant.

## Scope

In scope: one tool, `notes_create_note`. It creates a new note in a folder named
`Nook` in the default Notes account.

Not in scope: reading, searching, editing, moving, or deleting notes, other
folders, other accounts, Reminders, Calendar, Mail, Messages, the clipboard,
screen capture, and any other app. Each of those needs its own design, because
reading a note shows private data and a delete cannot be undone.

## Platforms

| Platform | Support | Reason |
| --- | --- | --- |
| macOS | Yes | Notes and Apple events exist. |
| Linux | No | No Notes app. The tool is not offered. |
| Windows | No | No Notes app. The tool is not offered. |

## The executor is the Panel

The Daemon has no window and no desktop session. The Panel runs the tool. This is
the reverse of the file and command tools, which the Daemon runs.

1. The Worker sends `execute_tool` for `nook:notes_create_note`, as for any tool
   that the Daemon owns.
2. The Daemon checks the arguments against the schema, asks Cedar, and shows the
   approval.
3. After an allow, the Daemon sends the Panel a `native_request` with the call
   id, the operation name `notes_create_note`, the validated arguments, and a
   one-time nonce.
4. The Panel runs the operation from a fixed list. It refuses any other
   operation and any request without a nonce that the Daemon issued for that call.
   The nonce stops a replay of an old request.
5. The Panel replies with `native_result`. The Daemon returns it to the Worker as
   `tool_result`.

If no Panel is connected, the call fails with `Error: the desktop is not
available`. Nothing queues for later.

The Daemon tells the Worker about the tool in the tool catalog only when a Panel
has reported that it can run it. The Panel reports this when it connects, and
only on macOS.

## The tool

`nook:notes_create_note` takes `title` (1 to 200 characters) and `body` (up to
20,000 characters of plain text). It returns `Created the note "<title>" in the
Nook folder` or an error.

The Panel runs `/usr/bin/osascript` with a script that is part of the Panel. The
script is fixed and takes its two inputs as arguments (`on run argv`). The model's
text is never part of the script text. The Panel turns the body into HTML before
it passes it: it escapes `&`, `<`, and `>`, and it puts each line in a `div`. The
script creates the `Nook` folder in the default account when it is missing, then
creates the note there.

The Panel does not run an `osascript` that the model or the Daemon supplies.
Cedar also keeps its existing rule that forbids `osascript` for the command tool.

## Approval

The approval shows the folder (`Nook`), the title, and the first 200 characters
of the body with its total length. It offers **Allow once**, **Allow this tool
for this chat**, and **Deny**. It offers no permanent grant, because every note
is new content in the user's account.

Cedar action `notes_create_note` has the context values `folder`,
`title_length`, and `body_length`. Policy forbids a body of more than 20,000
characters and a title with a newline.

## Permission from macOS

The first call makes macOS ask "Nook wants to control Notes". The user can
refuse. A refusal returns `Error: macOS did not allow Nook to control Notes`, and
the Panel does not ask again until the user changes the setting in System
Settings, Privacy and Security, Automation. The Panel needs a usage description
in `Info.plist` (`NSAppleEventsUsageDescription`). A signed build with the
hardened runtime also needs the entitlement `com.apple.security.automation.apple-events`.
Both are checked when the work starts, with the release signing setup.

## Credentials and data handling

The Daemon stores the title and body only in the chat transcript, as for any tool
argument. The Panel does not write them anywhere. The tool cannot read any note,
so the assistant never sees what is already in Notes.

## Isolation

The Panel exposes no general command to the web view for this operation. The
operation lives in Rust, behind the nonce check. The web view can neither trigger
it nor read a nonce. The Tauri capability file gets no new permission.

## Failure behavior

| Case | Result |
| --- | --- |
| No Panel connected | `Error: the desktop is not available`. |
| Platform is not macOS | The tool is not offered. |
| User denies the approval | A fixed denial reason, as for any tool. |
| macOS refuses Automation | `Error: macOS did not allow Nook to control Notes`. |
| Notes does not answer within 15 seconds | `Error: Notes did not answer`. |
| Request has a wrong or used nonce | The Panel refuses it and logs it. |
| Body above 20,000 characters | The Daemon refuses it before any prompt. |

## Tests

- Contracts: schema and examples for `native_request` and `native_result`.
- Policy: a long body and a title with a newline are forbidden. No permanent grant
  is offered.
- Panel: an operation outside the list is refused. A wrong or reused nonce is
  refused. The HTML conversion escapes `&`, `<`, and `>`, and a body with quotes,
  backslashes, and `$(...)` reaches the script as plain data.
- Native flow: the E2E build runs a fake program in place of `osascript`. It
  writes its arguments to a file. The test checks that the program ran only after
  Allow, that a deny leaves the file empty, and that a cancel does not run it.
  No test touches the real Notes app.
- Manual: one real note on a Mac, to check the Automation prompt and the HTML.

## Open questions for the approval

1. Is a fixed `Nook` folder acceptable, or should the user choose the folder in
   `config.toml`?
2. Should the tool append to an existing note that it created? That needs a way to
   find the note, and the first version leaves it out.
3. Does the release signing setup include the Automation entitlement?
