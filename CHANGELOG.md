# Changelog

This file records user-visible changes for each Nook release.

## Unreleased

### Changed

- Package the Panel with `nookd` and a platform-built Worker runtime for macOS, Linux, and Windows release builds.
- Keep PostgreSQL as an external prerequisite. The installer checks the configured database URL and does not install PostgreSQL.
- Keep `nookctl` optional instead of downloading it during GUI installation.
- Add package checks for bundled daemon and Worker files. Native clean-install checks remain required before claiming platform support.

### Fixed

- Fail installation when the selected package omits a required daemon or Worker runtime.
- Use the actual PyInstaller one-folder executable path when building and smoke testing the Worker.

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
