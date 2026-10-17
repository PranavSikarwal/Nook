# Phase T implementation plan: Tauri cross-platform migration

Phase T migrates the Nook overlay UI from macOS-only AppKit and SwiftUI to a cross-platform Tauri v2 desktop application. It delivers the exact same visual design and overlay behavior across macOS, Ubuntu Linux, and Windows, and packages native installers (`.dmg`, `.deb`, `.AppImage`, `.msi`).

## Architectural context

- **Current state**: The Rust daemon (`nookd`) and Python worker (`nook_worker`) are cross-platform. The current UI panel (`app/`) uses macOS-only AppKit and SwiftUI (`NSPanel`, Carbon `RegisterEventHotKey`).
- **Target architecture**: A Tauri v2 application combining a Rust core process with a web frontend.
- **IPC layer**: The frontend connects to `nookd` over the existing line-delimited JSON protocol defined in `contracts/panel-daemon.schema.json`.
- **Window capabilities**:
  - Frameless, transparent, floating overlay.
  - Draggable by window background or header region.
  - Persists in the background across focus changes; does not auto-hide on click-away.
  - Global hotkey: `Option+Space` on macOS, `Alt+Space` on Ubuntu and Windows.
  - Dismissible with the hotkey or `Escape`.

## Phasing and tasks

### T1. Tauri v2 scaffolding and window management

1. Scaffold Tauri v2 workspace in `panel/` with Vite and TypeScript.
2. Configure `tauri.conf.json`:
   - Frameless, transparent window without system titlebar.
   - Always-on-top / floating level.
   - Hidden from taskbar / Dock when idle (`skipTaskbar: true`).
   - Draggable regions via `data-tauri-drag-region`.
3. Configure global shortcut plugin (`@tauri-apps/plugin-global-shortcut`):
   - Register `Option+Space` on macOS and `Alt+Space` on Linux and Windows.
   - Toggle window visibility without resetting active chat state.
4. Implement window persistence:
   - Window remains visible when user clicks other windows.
   - Dismissible exclusively via global hotkey, Escape, or close control.

### T2. UI design replication

1. Build matching dark translucent theme:
   - Dark background (`#1e1e1e` / `rgba(30, 30, 30, 0.95)`), rounded corners (16px), subtle border, and shadow.
2. Implement view modes:
   - **Compact input**: Centered input bar, attachment paperclip button, history button, settings button, send button.
   - **Expanded transcript**: Message bubbles for user and assistant, auto-scrolling during streaming.
3. Implement Markdown rendering:
   - Monospaced code blocks with syntax highlighting.
   - Copy button on code blocks writing to system clipboard.
   - Formatted inline text, lists, and tables.
4. Implement auxiliary views:
   - **History drawer**: List of past conversations sorted by updated date, click to load, delete confirmation.
   - **Attachment chips**: Previews for images, text, and PDF files with removal chips.
   - **Settings view**: Model base URL, model name, and API key configuration.
   - **Controls**: Stop button during streaming, inline error display with Retry button.

### T3. Daemon client and contract integration

1. Implement WebSocket or Unix domain socket / named pipe client in Tauri Rust backend.
2. Map all message types matching `contracts/panel-daemon.schema.json`.
3. Expose typed Tauri commands and events to the frontend:
   - `send_message`, `list_chats`, `get_chat`, `delete_chat`, `set_settings`, `ping`.
   - Stream `text_delta`, `message_started`, `message_finished`, `chat_titled`, and `error` events to the UI.
4. Add auto-reconnection loop when daemon restarts.

### T4. Packaging and multi-platform distribution

1. **macOS**:
   - Configure bundle target for `.dmg` and `.app`.
   - Ad-hoc codesigning configuration.
2. **Ubuntu Linux**:
   - Configure bundle target for `.deb` and `.AppImage`.
   - Verify X11 and Wayland global shortcut support.
3. **Windows**:
   - Configure bundle target for `.msi` and `.exe` installer.
   - Verify `Alt+Space` hotkey and `WS_EX_TOPMOST` window behavior.
4. Add GitHub Actions matrix build workflow (`.github/workflows/package.yml`) to compile and package all three platform installers automatically.
