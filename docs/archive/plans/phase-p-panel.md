# Phase P implementation plan: Panel

The Panel is a macOS overlay chat interface built with AppKit and SwiftUI. It communicates with `nookd` over the Unix domain socket at `~/Library/Application Support/Nook/daemon.sock`. It runs with no Dock icon (`LSUIElement = true`) and floats above full-screen windows on any Space.

## Architectural context

- Built with Swift Package Manager without Xcode IDE (ADR 0002).
- Single window is an `NSPanel` subclass with `.nonactivatingPanel`, `.borderless`, `.floating`, `.canJoinAllSpaces`, and `.fullScreenAuxiliary`.
- Returns `true` from `canBecomeKey` to receive keyboard focus without activating the application (open point 3).
- Uses Carbon `RegisterEventHotKey` for the global Option+Space hotkey without requiring Accessibility permissions.
- Socket communication uses line-delimited JSON matching `contracts/panel-daemon.schema.json`.

## Phasing and tasks

### P0. Overlay spike and app bundle script

1. Create `app/Package.swift` configured for macOS 14+ (`v14`), defining an executable target `Nook` and a test target `NookTests`.
2. Implement `app/Sources/Nook/NookPanel.swift`, subclassing `NSPanel` with `canBecomeKey = true`.
3. Implement `app/Sources/Nook/PanelController.swift` configuring window level, collection behaviors, Esc key monitoring, and click-away detection with `NSEvent.addGlobalMonitorForEvents`.
4. Implement global hotkey registration using Carbon `RegisterEventHotKey` with default Option+Space.
5. Create `scripts/bundle.sh` to compile `swift build -c release`, build `Nook.app` bundle layout, generate `Info.plist` with `LSUIElement = true`, and sign with `codesign --force --deep --sign -`.
6. Document results and verification checklist in `app/README.md`.

### P1. Protocol types and daemon client

1. Implement `app/Sources/Nook/Protocol.swift` with Swift `Codable` structs matching `contracts/panel-daemon.schema.json` (requests, responses, events, error info, attachment shapes).
2. Write unit tests in `app/Tests/NookTests/ProtocolTests.swift` validating serialization and deserialization against JSON sample files in `contracts/examples/`.
3. Implement `app/Sources/Nook/DaemonClient.swift` using `NWConnection` or `FileHandle` Unix domain socket client connecting to `~/Library/Application Support/Nook/daemon.sock`.
4. Add auto-reconnection loop: retry once per second up to 10 seconds with disconnected state notifications.
5. Provide line-delimited JSON streaming parser dispatching typed messages to listeners.

### P2. Compact and expanded views with streaming

1. Build `app/Sources/Nook/Views/CompactInputView.swift`: single-line text input, send button, attachment paperclip button, and history toggle button.
2. Build `app/Sources/Nook/Views/TranscriptView.swift`: vertical scroll list displaying message bubbles for user and assistant roles.
3. Build `app/Sources/Nook/Views/ContentView.swift` managing layout state transitions:
   - Compact: single input bar.
   - Expanded: transcript view expands above the input on the first sent message.
4. Integrate with `DaemonClient.sendMessage`:
   - Send generates a fresh UUID `chat_id` on first message.
   - Streams incoming `text_delta` chunks into the assistant message bubble.
   - Scrolls transcript to bottom automatically during streaming unless user scrolls up.

### P3. Markdown rendering and code blocks

1. Test Markdown rendering approaches for code blocks, tables, and lists.
2. Implement assistant message bubble renderer with:
   - Syntax-highlighted monospaced code blocks.
   - Copy button on each code block that writes block text to `NSPasteboard.general`.
   - Formatted inline text, lists, and tables.
3. Record benchmark and choice in `docs/adr/` if a third-party dependency is introduced.

### P4. Chat history

1. Build `app/Sources/Nook/Views/HistoryView.swift` showing past conversations ordered newest first.
2. Add title and timestamp display for each conversation row.
3. Wire History button in input bar to dispatch `list_chats` and toggle the history list in place of the transcript.
4. Handle row selection: dispatch `get_chat`, load transcript messages, and switch view to past chat mode for continued conversation.
5. Add delete confirmation button on each row: dispatches `delete_chat` and removes the row from the local view.
6. Listen for `chat_titled` broadcast events from `DaemonClient` and update conversation titles in real time.

### P5. Attachments handling

1. Implement `app/Sources/Nook/Attachments.swift` handling file picker and clipboard operations.
2. Implement Cmd+V clipboard monitor checking `NSPasteboard` for image data or file URLs.
3. Wire paperclip button to open `NSOpenPanel` allowing multi-file selection.
4. Add client-side validation enforcing limits:
   - Maximum 5 attachments.
   - Maximum 10 MB file size per attachment.
   - Allowed MIME types (PNG, JPEG, WebP, GIF, PDF, plain text, Markdown, code, JSON, CSV).
   - Display red chip with rejection reason for invalid files.
5. Copy valid attachments into `~/Library/Application Support/Nook/attachments/<chat_id>/<id>-<name>` before sending.
6. Display attachment chips above the input bar with remove buttons.

### P6. Settings window

1. Implement `app/Sources/Nook/Views/SettingsView.swift` containing fields for base URL, model name, API key, and global hotkey.
2. Populate base URL and model on open by sending `get_settings`. Show API key field as "stored" when `has_api_key` is true.
3. Implement save action: sends `set_settings` with updated fields.
4. Store custom hotkey configuration in `UserDefaults`.

### P7. Errors, cancellation, and disconnected state

1. Display inline error bubble with Retry button on `DaemonMessage.Error`.
2. Retry button resends original text and attachments under a new request id.
3. Display Stop button replacing send button during active streaming; clicks send `ClientMessage.Cancel`.
4. Display banner "Nook daemon not running" and disable input when socket is disconnected.

## Verification checklist

- `swift build`: Compiles with zero warnings.
- `swift test`: All protocol tests pass against contract examples.
- `scripts/bundle.sh`: Produces `Nook.app` signed with ad-hoc identity.
- Manual checklist in `app/README.md`:
  - Option+Space toggles overlay.
  - Overlay appears over full-screen spaces.
  - Escape closes overlay.
  - Clicking outside closes overlay.
  - Typing works without activating the application.
  - Streaming replies update text in real time.
