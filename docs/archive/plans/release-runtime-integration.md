# Release runtime integration plan

## Goal

Make the macOS, Linux, and Windows application packages contain the Panel, daemon,
and a runnable Worker. Keep PostgreSQL as an explicit prerequisite and report a
failed database check before the user submits a request. This work does not
bundle or manage PostgreSQL.

## Current gaps

- The macOS DMG contains the Panel but not `nookd` or the Worker runtime.
- Linux packages contain the Panel but not the daemon or Worker runtime.
- Windows packages do not build `nookd.exe`, although Windows named-pipe IPC now
  exists.
- `install.sh` downloads `nookd` and `nookctl` as separate files on macOS. Its
  Linux package path does not add the daemon or Worker. Its local setup attempts
  `uv sync`, suppresses failure, and can report success with no Worker.
- The package workflow publishes platform packages before a package-content test
  confirms their runtime files.

## Implementation

1. Add PyInstaller to the Worker release build dependencies and create a
   platform-neutral Worker entry point. Build the executable with PyInstaller's
   one-folder layout on the matching GitHub runner for macOS ARM64, Ubuntu
   x86_64, and Windows x86_64. One-folder output preserves the shared libraries
   and data files required by native Python dependencies.
2. Run a packaged Worker protocol smoke test before bundling it. The test sends
   the protocol shutdown event and verifies the ready event. It must not load
   model credentials or connect to PostgreSQL.
3. Resolve Worker `.env` only for source checkouts. Packaged Workers receive
   settings through the daemon environment and must not search a build-time
   source path.
- Build `nookd` and `nookctl` on every supported runner. Keep `nookctl` as a
   separate optional release asset.
3. Stage `nookd` and the Worker executable as Tauri resources. Put them at fixed
   resource paths for each operating system.
4. Update daemon discovery to use the bundled Worker executable first. Keep the
   development virtual environment and explicit worker-command override paths.
5. Add package tests that extract the DMG, `.deb`, AppImage, and Windows
   installer. Assert that each contains the Panel, daemon, and Worker.
6. Add clean-install tests on native macOS, Ubuntu, and Windows runners. Start
   the packaged app, verify daemon ping, run a deterministic request, and check
   that the configured PostgreSQL endpoint is reachable.
7. Change `install.sh` to install or upgrade the application package without
   separately installing `nookctl`. Fail if any required package file is absent.
   Preserve config and database files.
8. Update README download, install, upgrade, and PostgreSQL prerequisite steps to
   match the tested packages.
9. Keep Windows listed as preview until its full Windows installer test passes.
   Promote it only after named-pipe IPC, daemon packaging, Worker packaging, and
   deterministic request checks all pass.

## Verification

- Run the Worker executable smoke test on all three build runners.
- Inspect package contents before publishing.
- Install each package in a clean runner and run the deterministic native request.
- Verify a missing PostgreSQL service produces a clear setup error and does not
  claim that Nook is ready.
- Run repository CI, SonarCloud, native macOS E2E, Linux package tests, and
  Windows named-pipe plus installer tests before release.

## Completion criteria

- macOS, Linux, and Windows installers include the Panel, daemon, and Worker
  runtime for their declared architecture.
- A clean install starts the daemon and Worker without a repository checkout,
  user-installed `uv`, or separate daemon download.
- Package tests fail if any required runtime file is missing.
- README install and upgrade instructions match the packages CI tested.
- PostgreSQL remains documented as a required external service in this release.
