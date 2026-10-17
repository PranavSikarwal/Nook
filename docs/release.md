# Release and installation

Nook packages the Panel, Daemon, and Worker runtime together. PostgreSQL remains
a user-managed dependency.

## Packages

GitHub Releases publish a macOS Apple Silicon disk image, a Debian package, and
Windows installer packages. Package verification checks that the bundled Daemon
and Worker runtime exist in each platform artifact.

The macOS installer places `Nook.app` in the user Applications directory. It
keeps application data, including `config.toml`, in
`~/Library/Application Support/Nook` during upgrades.

## Linux requirements

The Debian package recommends `bubblewrap`. The command tool and MCP servers
need it, and the Daemon checks it at startup. Without `bubblewrap`, or where the
system blocks user namespaces, those tools are off and the Daemon logs why. The
file tools, web tools, and search tools work without it. macOS needs nothing
extra, because it ships `sandbox-exec`. Windows has no sandbox, so file access
beyond the web tools stays off there.

## Configuration

`database_url` identifies the PostgreSQL database. The installer does not
install PostgreSQL and does not rewrite the configured database URL. Nook can
be installed while the database is unavailable. Requests report a database
error until the Daemon can connect.

## Release checks

A release change keeps application version metadata aligned, updates
`CHANGELOG.md`, and passes CI, package verification, and the native Tauri test
gate. `CONTRIBUTING.md` contains the release procedure.
