# Tauri Panel replaces the SwiftPM Panel

## Status

Accepted

## Date

October 12, 2026

## Context

ADR 0002 selected a macOS SwiftUI and AppKit Panel built with Swift Package
Manager. Nook now packages a Tauri Panel for macOS, Linux, and Windows. The
Panel implementation is in `panel/` and uses a React frontend with Rust Tauri
code for desktop integration.

The project needs one current Panel architecture. Keeping the Swift Panel as the
active decision would misdirect new implementation and release work.

## Decision

Nook uses the Tauri Panel as its desktop interface. It communicates with the
Daemon through user-local IPC. macOS and Linux use Unix sockets. Windows uses a
named pipe scoped to the current account.

This decision supersedes ADR 0002. ADR 0002 remains a record of the earlier
macOS implementation choice.

## Consequences

- New Panel work belongs in `panel/`.
- Panel documentation describes Tauri and React rather than SwiftUI and AppKit.
- Release checks validate the Tauri packages for supported platforms.
- The repository retains ADR 0002 as historical context.
