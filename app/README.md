# Nook Panel

The Nook Panel is a lightweight macOS overlay application built with AppKit and SwiftUI using Swift Package Manager without the Xcode IDE (ADR 0002).

## Architecture

- **Bundle**: `build/Nook.app` built by `scripts/bundle.sh`.
- **UI Agent**: Runs with `LSUIElement = true` and `NSApplication.setActivationPolicy(.accessory)` with no Dock icon.
- **Overlay Window**: `NookPanel`, an `NSPanel` subclass with `.nonactivatingPanel` and `.borderless` style masks.
- **Window Level**: `.floating`, with collection behaviors `.canJoinAllSpaces` and `.fullScreenAuxiliary` so it displays over full-screen spaces.
- **Global Hotkey**: Carbon `RegisterEventHotKey` (default Option+Space) which requires no Accessibility permissions.
- **Dismissal**: Escape key via local event monitor, and outside clicks via global mouse event monitoring.

## Open point 3 resolution

**Question**: Can a non-activating `NSPanel` take keyboard input without making the app active?

**Answer**: Yes.
By subclassing `NSPanel` and overriding:
```swift
override var canBecomeKey: Bool { true }
override var canBecomeMain: Bool { true }
```
and presenting with `panel.makeKeyAndOrderFront(nil)`, AppKit routes keyboard events to the panel's first responder while the current frontmost application (such as Terminal, Safari, or an editor) retains its active menu bar and system focus.

## Build and run

Build the application bundle:
```sh
scripts/bundle.sh
```

Run tests:
```sh
(cd app && swift run NookTests)
```

Launch the app:
```sh
open build/Nook.app
```

## Manual overlay check list

Verify the overlay behavior manually after building the bundle:

1. **No Dock icon**: Launch `open build/Nook.app`. Verify no icon appears in the macOS Dock or App Switcher (Cmd+Tab).
2. **Hotkey toggle**: Press `Option+Space`. The panel appears centered in the upper third of the screen. Press `Option+Space` again; the panel hides.
3. **Full-screen overlay**: Put an application (e.g. Safari or Terminal) into macOS full-screen mode. Press `Option+Space`. The panel floats directly over the full-screen window.
4. **Keyboard input**: Focus remains on the panel text field. Typing input enters text into the field without switching the active app menu bar.
5. **Escape dismissal**: Press `Escape`. The panel closes immediately.
6. **Click-away dismissal**: Open the panel with `Option+Space`, then click anywhere outside the panel frame. The panel closes immediately.
