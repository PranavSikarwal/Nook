# Panel

The Panel is a SwiftUI and AppKit app in `app/`, built with Swift Package Manager and the command line tools (ADR 0002). It runs as an agent app with no Dock icon. It keeps no data of its own. It asks the Daemon for everything over the socket described in `01-contracts.md`.

## Layout

```
app/
  Package.swift
  Sources/Nook/
    NookApp.swift            entry point, no Dock icon
    PanelController.swift    the NSPanel, show and hide, hotkey
    DaemonClient.swift       socket, JSON lines, reconnect
    Protocol.swift           Codable types that match contracts/
    Views/                   compact input, transcript, history, settings
    Attachments.swift        paste, picker, validation, file copy
  Tests/NookTests/
  scripts/bundle.sh          builds the .app and signs it
```

## The overlay

1. The Panel is an `NSPanel` with the style masks `.nonactivatingPanel` and `.borderless`, window level `.floating`, and collection behavior `.canJoinAllSpaces` plus `.fullScreenAuxiliary`. That lets it appear over full-screen apps and on every Space.
2. A borderless panel does not take keyboard input by default. Subclass `NSPanel` and return `true` from `canBecomeKey`, then check that text entry works without the app becoming active. This is open point 3 in the overview.
3. The global hotkey uses the Carbon `RegisterEventHotKey` call, which needs no Accessibility permission. The default is Option+Space. Settings can change it.
4. Option+Space toggles the Panel. Escape closes it. A click outside closes it.
5. Closing hides the Panel and keeps the Daemon connection open. Opening always starts a fresh Chat with a new UUID.

## States

| State | What it shows |
| --- | --- |
| Compact | One text field with a paperclip button and a History button. Roughly one line tall. |
| Expanded | The current Chat's Transcript above the input. The Panel grows when the first question is sent and keeps its height afterward. |
| History | A list of past Chats, newest first, each with its title and date. It replaces the Transcript area. |
| Past Chat | A past Chat's Transcript with the input below, so the user can keep asking. |

A "New chat" button returns to Compact with a fresh Chat id.

## Transcript

1. Each Message is a bubble. The assistant's text renders as Markdown, with code blocks in a monospaced font and a Copy button on each code block.
2. Text arrives as `text_delta` events and is appended as it comes. The view stays scrolled to the bottom unless the user scrolls up.
3. A Stop button replaces the send button while a reply is in progress. It sends `cancel`.
4. An error shows inline under the question with a Retry button. Retry sends the same text and Attachments again as a new `send_message`.
5. Attachments in a sent Message show as image thumbnails or file chips.
6. Pick one SwiftPM Markdown library or use `AttributedString(markdown:)`. Task P3 decides after a test with code blocks, lists, and tables. Record the choice in the task.

## History

1. The History button sends `list_chats` and shows the result. Titles that are still empty show "New chat".
2. Clicking a Chat sends `get_chat` and shows its Transcript.
3. Each row has a delete button with a confirmation. It sends `delete_chat`.
4. `chat_titled` events update the list live.

## Attachments

1. Cmd+V checks the pasteboard. An image becomes an Attachment. File URLs become Attachments. Plain text pastes into the field as normal.
2. The paperclip button opens an `NSOpenPanel` that allows multiple files.
3. Each Attachment is validated against the limits in `02-data-model.md`. A rejected file shows a red chip with the reason, and it is not sent.
4. Accepted files are copied into `~/Library/Application Support/Nook/attachments/<chat_id>/<id>-<name>` before sending.
5. Chips sit above the input and can be removed before sending.

## Settings

A small window opened from a gear icon or the menu bar item. Fields are the base URL, the model name, the API key, and the hotkey.

1. The Panel loads the base URL and model with `get_settings`. The API key field shows "stored" when `has_api_key` is true and is empty otherwise.
2. Save sends `set_settings`. The Daemon writes the config file and the Keychain. The Panel never reads or writes either one.
3. The hotkey is a Panel-only setting stored in the app's user defaults.

## Daemon connection

1. Connect to the Unix socket on launch. If it fails, retry every second with a cap of 10 seconds.
2. While disconnected, the Panel shows "Nook daemon not running" and disables the input.
3. Decode each line into the `Codable` types in `Protocol.swift`. An unknown event type is logged and skipped.

## Bundling

`scripts/bundle.sh` runs `swift build -c release`, creates `Nook.app/Contents/{MacOS,Info.plist}`, sets `LSUIElement` to `true`, and signs the bundle with `codesign` (ad hoc is enough for local use). Notarization is out of scope because the app is not distributed.

## Tests

1. Decoding tests read every line in `contracts/examples/` and check that the Swift types accept them.
2. A fake Daemon, a small script that listens on a socket and replays recorded events, lets the Panel run without the real Daemon.
3. Manual check list in `app/README.md` for the overlay behavior, since it cannot be unit tested: appears over a full-screen app, takes typing, closes on Escape and on click-away, and toggles with the hotkey.
