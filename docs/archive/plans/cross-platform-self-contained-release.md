# Cross-platform self-contained release plan

## Purpose

Make each supported platform installer deliver a working Nook desktop app without
asking users to install `nookd`, `nookctl`, the Python Worker, or a database
component separately.

This plan covers macOS, Ubuntu and Debian Linux, and Windows. It does not add
Windows Unix-socket support unless the project adopts a Windows IPC transport.

## Current release gaps

The `v0.1.2` macOS DMG contains `Nook.app`, but the bundle contains only
`nook-panel`. It does not contain `nookd`.

`.github/workflows/package.yml` builds `nookd` and `nookctl` only on macOS and
Ubuntu. It uploads both as separate release assets. The Tauri package step does
not copy either binary into the app package.

The Worker is not packaged in `Nook.app`, Linux packages, or Windows packages.
`nookd` expects a Worker directory with its own `.venv`, or a usable `uv`
installation. A release install therefore cannot start a Worker on a clean
machine.

The installer downloads separate macOS `nookd` and `nookctl` assets after it
installs the DMG. This makes `nookctl` available even though the GUI does not
need it. The installer does not check that the downloaded daemon starts.

The Linux installer assumes a Debian package exists and uses `sudo`. Its
fallback downloads an AppImage but does not install `nookd`, `nookctl`, or the
Worker. The README says the installer has no manual steps, which the released
assets do not meet.

The release workflow does not build a Windows `nookd` or `nookctl`. The Panel
also returns an unsupported local Unix-domain-socket error on Windows. The
Windows installer artifact is a preview client, not a working standalone Nook
installation.

Every platform assumes a PostgreSQL database at the configured URL. Neither the
release packages nor `install.sh` install, start, or validate PostgreSQL.

## Release contract

The release owner must publish one supported install path per platform.

| Platform | Supported install path | Required contents |
| --- | --- | --- |
| macOS Apple Silicon | Signed and notarized DMG | `Nook.app` with Panel, `nookd`, Worker runtime, and an explicit database prerequisite check. |
| Ubuntu and Debian x86_64 | `.deb` package | Panel launcher, `nookd`, Worker runtime, desktop entry, and a declared PostgreSQL dependency. |
| Linux x86_64 without a package manager | AppImage or tarball | Panel launcher, `nookd`, Worker runtime, and a preflight script that reports missing PostgreSQL. |
| Windows x86_64 | Installer only after IPC support exists | Panel, `nookd.exe`, Worker runtime, and the Windows IPC implementation. |

`nookctl` is optional. Release automation may publish it as a separate advanced
CLI asset. The GUI installer must not depend on it.

The project must choose how a release provides PostgreSQL before claiming a
self-contained install. The supported options are:

1. Require a user-managed PostgreSQL instance and make the installer test the
   configured connection before launch.
2. Bundle a managed local PostgreSQL distribution and give Nook ownership of
   its data directory, lifecycle, migrations, and upgrades.
3. Replace PostgreSQL with an embedded store for desktop deployments.

Option 1 has the smallest release change. Options 2 and 3 change the product
runtime and need a separate design decision.

## Implementation plan

### 1. Add Windows local IPC

Keep the existing newline-delimited JSON protocol and existing message types.
The Panel and daemon must use Unix domain sockets on macOS and Linux. Windows
must use named pipes.

1. Add a local IPC module with compile-time platform implementations.
2. Keep `tokio::net::UnixListener` and `tokio::net::UnixStream` on macOS and
   Linux.
3. Add a Windows named-pipe listener in `nookd` and a named-pipe connector in
   the Panel.
4. Use a per-user pipe name. The daemon must grant access only to the current
   Windows user.
5. Route every existing Panel command through the local IPC module without
   changing the JSON request and event protocol.
6. Add Windows tests for ping, chat operations, a deterministic request,
   approval decisions, and cancellation.

Done condition: The same JSON protocol works over Unix sockets on macOS and
Linux and named pipes on Windows.

### 2. Define supported platforms and prerequisites

1. Decide whether Nook requires user-managed PostgreSQL or bundles a database.
2. Record the database choice in an ADR before changing installers.
3. Define supported CPU architectures for each platform. Do not offer an asset
   name that the workflow does not build.
4. Mark Windows as a supported platform only after its named-pipe tests pass.

Done condition: README and release metadata name only supported platform and
architecture combinations.

### 3. Package the runtime as one application unit

1. Build `nookd` for every platform that the Panel supports.
2. Build the Python Worker into a relocatable runtime for each target platform.
   The runtime must contain Python, `nook_worker`, and locked dependencies. It
   must not require `uv`, a repository checkout, or a user-created virtual
   environment.
3. Copy the daemon and Worker runtime into Tauri resources before the package
   step.
4. Make `find_nookd_binary` and `find_worker_directory` resolve only the
   packaged resource paths for installed applications. Keep development paths
   behind development builds or explicit environment overrides.
5. Add a startup preflight that reports a missing Worker runtime or database
   connection as a clear user-facing error.

Done condition: An installed app launches its bundled daemon and Worker with no
external binary lookup.

### 4. Repair platform package jobs

1. Replace the current generic Tauri package step with platform preparation
   steps that place the correct daemon and Worker runtime into the app package.
2. Add a macOS package inspection step that mounts the DMG and verifies:
   - `Nook.app/Contents/MacOS/nook-panel` exists.
   - The bundled daemon exists at the documented path.
   - The bundled Worker runtime exists and imports `nook_worker`.
3. Add a Linux package inspection step that extracts the `.deb` and AppImage,
   then verifies the launcher, daemon, and Worker paths.
4. Add Windows package inspection only after Windows IPC support exists. Verify
   `nook-panel.exe`, `nookd.exe`, and the Worker runtime.
5. Sign and notarize the macOS bundle before publishing it. The current ad hoc
   signing identity is not suitable for a normal downloaded application.
6. Publish checksums for each installer asset.

Done condition: CI fails if a published GUI installer omits a required runtime
component.

### 5. Replace the installer with platform-specific installers

1. Keep `install.sh` for macOS and Linux only. Do not describe it as Windows
   support.
2. On macOS, install only the DMG application. Do not download `nookd` or
   `nookctl` separately after the app bundle becomes complete.
3. On Linux, select a `.deb` only for Debian-based distributions. Provide an
   AppImage or tarball path for other distributions. Do not use `sudo` in a
   user-only install path.
4. Have each installer run a version check and a preflight that validates the
   packaged daemon, Worker runtime, and database prerequisite.
5. Preserve existing configuration and user data during upgrades. Do not remove
   the Nook app-data directory or database during replacement.
6. Keep `nookctl` in a separate optional installation command for advanced
   troubleshooting.

Done condition: A user can install or upgrade the GUI without downloading or
copying any separate runtime asset.

### 6. Correct the README and release page

1. Replace the current Downloads section with a platform matrix that states
   supported architectures, package types, database prerequisite, and Windows
   status.
2. Change the quick-install section to name the exact installer behavior. Do
   not claim a one-command self-contained setup until the bundled runtime and
   database contract exist.
3. State that the macOS DMG installs the Panel, daemon, and Worker together
   after the package fix ships.
4. Describe `nookctl` as optional and link to its separate download or install
   command.
5. Add upgrade instructions for each supported platform. Include how to stop a
   running app, replace the app package, relaunch, and preserve configuration.
6. Generate the GitHub release description from `CHANGELOG.md` after a release
   process is defined.

Done condition: README instructions match the release artifacts inspected by
CI.

### 7. Add release acceptance tests

1. Run package inspection on every target platform.
2. Install each package in a clean platform-specific environment.
3. Verify the Panel starts its packaged daemon and Worker.
4. Verify the database prerequisite check reports a missing or unreachable
   database clearly.
5. Run one deterministic request through the installed application.
6. Run one live request only in a protected release validation environment.
7. Attach installation logs and package inspection output to the release run.

Done condition: Each published GUI installer passes installation, startup, and
request validation on its supported platform.

## README and installer changes before the package fix

Until a release contains the daemon and Worker runtime, the README must state
that packaged installers are incomplete for a clean machine. It must not call
them self-contained.

The macOS installer must either install the separate `nookd` asset and verify it
before it launches the app, or stop claiming that the DMG installs a working
application. It must list `nookctl` as optional.

The Linux installer must state that `.deb` installation needs administrator
rights. It must state that the AppImage path does not install the daemon or
Worker. It must not offer unsupported ARM Linux asset names.

The README must state that Windows is a preview client until the project adds
Windows IPC and packages the daemon and Worker.

## Open decisions

- Choose the PostgreSQL ownership model before implementing a self-contained
  installer.
- Choose a supported Worker runtime packaging method for macOS, Linux, and
  Windows.
- Decide whether the release workflow should create a draft release, run package
  checks, then publish it only after all targets pass.
