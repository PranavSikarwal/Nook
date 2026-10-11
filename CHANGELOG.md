# Changelog

This file records user-visible changes for each Nook release.

## Unreleased

## v0.1.4 - 2026-10-11

### Changed

- Stage `nookd` and a native PyInstaller Worker runtime in macOS, Linux, and Windows package builds.
- Keep PostgreSQL as an external prerequisite. Installers no longer require PostgreSQL to be running.
- Keep `nookctl` as an optional standalone download. GUI installers do not install it.
- Inspect the macOS DMG, Ubuntu Debian package, and Windows MSI for the bundled daemon and Worker runtime before release.
- Require the `Native Tauri E2E Gate` status on `main` and run deterministic native tests against the exact PR head before merge.

### Fixed

- Reject macOS and Linux architectures that do not have published installers.
- Preserve existing configuration and allow application installation before PostgreSQL is available.
- Use the packaged Worker executable for its protocol smoke test without reading `.env` settings or launching a shell command.
- Keep live current-news checks on web search instead of entering page-fetch approval and leaving the answer streaming.
- Extract Windows MSI files with Windows Installer before verifying the bundled daemon and Worker.

## v0.1.3

### Added

- Added Windows named-pipe IPC between the desktop Panel and daemon. The Panel and daemon keep the existing JSON-lines message protocol.
- Kept Unix domain sockets for macOS and Linux.
- Added a Windows integration test for named-pipe ping and a Windows Panel build check in CI.

### Fixed

- Scoped the Windows named pipe to the current account SID and restricted its access control list to that account and SYSTEM.
- Recreated the pipe server after accept errors and added a short delay before retrying.

### Limitations

- The Windows release package remains a preview. The release workflow does not yet bundle `nookd.exe` or the Python Worker runtime.

## v0.1.2 - 2026-10-10

### Fixed

- Fixed macOS and Windows compilation by gating the macOS app-reopen event behind the macOS target.

### Packaging

- Published macOS Apple Silicon, Ubuntu x86_64, and Windows x86_64 packages.
- Published separate `nookd` and `nookctl` binaries for macOS and Ubuntu.

### Limitations

- The macOS DMG does not include `nookd`. The desktop app needs the daemon installed separately.
- Linux packages do not bundle the Python Worker runtime or install PostgreSQL.
- The Windows package is a preview client. Local daemon IPC is not implemented in this release.

## v0.1.1 - 2026-10-06

### Fixed

- Fixed Linux and Windows compilation by gating the macOS `RunEvent::Reopen` handling to macOS.

### Packaging

- Published packages for macOS Apple Silicon, Ubuntu x86_64, and Windows x86_64.
- Published separate macOS and Ubuntu daemon and CLI binaries.

## v0.1.0 - 2026-10-06

### Added

- Added the first packaged Nook desktop release for macOS, Ubuntu Linux, and Windows.
- Added a self-starting installer that installs the Panel, daemon, and CLI binaries on macOS and Debian-based Linux.

### Limitations

- The desktop packages did not bundle the daemon or Python Worker runtime.
- Windows shipped as a preview Panel without local daemon IPC.
