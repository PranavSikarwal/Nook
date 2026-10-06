# Add configurable focused-Panel shortcuts

Type: task
Status: open
Blocked by: none

## Goal

Add a central focused-shortcut dispatcher and a shortcut settings section.

## Work

- Define action identifiers and platform defaults.
- Register action handlers from `App.tsx`.
- Add a recording control in Settings.
- Normalize modifier keys and reject conflicts and reserved combinations.
- Store bindings in the Panel-local settings store.
- Keep the global show-or-hide hotkey independent.

## Done when

Each default shortcut triggers only while the Panel has focus. User bindings
persist across relaunch. Duplicate and reserved bindings are rejected.
