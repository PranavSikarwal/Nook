# Native-system tools design

Status: proposed. No code exists. Implementation starts only after approval.

## Why

Some actions need the desktop, not a file or a process: put text on the
clipboard, show a notification, open a link in the default browser. They help
the assistant finish a task without a copy and paste by the user. They also
reach private data (the clipboard) and other applications.

## The executor is the Panel

The Daemon has no window and no desktop session. The Panel is a Tauri app with
a window and the permissions of a desktop application. So the Panel runs these
tools, and the Daemon authorizes them. This is the reverse of the file and
command tools.

1. The Worker sends `execute_tool` as for any Daemon tool.
2. The Daemon normalizes the call, asks Cedar, and shows the approval.
3. After an allow, the Daemon sends the Panel a `native_request` with the call
   id, the operation, the validated arguments, and a one-time nonce.
4. The Panel runs the operation from a fixed list. It refuses any other
   operation and any request without a nonce that the Daemon issued for that
   call. The nonce stops a replay of an old request.
5. The Panel replies with `native_result`. The Daemon returns it to the Worker
   as `tool_result`.

If no Panel is connected, the call fails with `Error: the desktop is not
available`. Nothing queues for later.

## Capabilities

The first version has three, and every one is separate. Each can be turned off
in `config.toml` and in Settings.

| Capability | Operation | Risk |
| --- | --- | --- |
| Clipboard write | Put text on the clipboard. | Low. It replaces what the user copied. |
| Clipboard read | Read the current text. | High. It can hold a password. |
| Notification | Show a title and a line of text. | Low. Text is cut to a fixed size. |
| Open link | Open an `http` or `https` link in the default browser. | Medium. It can lead to a harmful page. |

Screen capture, window control, keyboard and mouse input, accessibility APIs,
contacts, calendar, and messages are out of scope. Each needs its own design
because each needs an operating system permission that gives broad access.

## Platforms

| Platform | Clipboard | Notification | Open link |
| --- | --- | --- | --- |
| macOS | Yes | Yes | Yes |
| Linux | Yes, on X11 and Wayland through the Tauri plugin | Yes | Yes |
| Windows | Not in the first version | Not in the first version | Not in the first version |

Windows follows the other privileged tools: no sandbox exists, so nothing is
offered. Linux without a clipboard service returns `Error: no clipboard is
available`.

## Approval

- **Clipboard read** always asks. The prompt says what will happen and shows no
  clipboard text. The Panel reads the text only after the user allows it. The
  Panel cuts the result to 8 KiB. No grant covers it, not even for the chat.
- **Clipboard write** shows the first 200 characters of the text and its length.
  It offers allow once and allow for the chat.
- **Notification** shows the title and text. It offers allow once and allow for
  the chat. The Daemon limits it to 3 notifications per minute.
- **Open link** shows the full link and its host. It offers allow once only.
  Policy forbids a link with a username, a non-web scheme, or an address that
  `web_fetch` would refuse.

## Credentials and data handling

The Daemon never stores clipboard text. It goes into the reply for that call and
the chat transcript like any tool result. The prompt for a read says so. A
result that looks like a secret (a long random string, a private key header) is
not detected or hidden, because detection is unreliable. The prompt is the
control.

## Isolation

The Panel exposes no general-purpose command to the web view for these
operations. The operations live in Rust, behind the nonce check. The web view
can neither trigger them nor read a nonce. The Tauri capability file lists only
the plugins these operations use.

## Failure behavior

| Case | Result |
| --- | --- |
| No Panel connected | `Error: the desktop is not available`. |
| Operation not allowed on this platform | The tool is not offered. |
| User dismisses the approval | A fixed denial reason, as for any tool. |
| Panel does not answer within 10 seconds | `Error: the desktop did not answer`. |
| Request has a wrong or used nonce | The Panel refuses it and logs it. |

## Tests

- Contracts: schema and examples for `native_request` and `native_result`.
- Policy: clipboard read has no grant action. An unsafe link is forbidden. The
  notification rate limit holds.
- Panel: an operation outside the list is refused. A wrong or reused nonce is
  refused.
- Native flow: with a fake clipboard service, the clipboard read returns text
  only after the user allows it, and a deny returns nothing.
- Cancel: a cancelled call does not run the operation.

## Open questions for the approval

1. Is clipboard read worth its risk in the first version?
2. Does the Daemon or the Panel own the notification rate limit?
3. Which of the three capabilities ships first?
