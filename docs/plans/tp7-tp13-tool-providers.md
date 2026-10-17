# Tool providers, sandbox, and policy controls (TP7 to TP13)

This plan turns Stage 2 of
[the tool-platform plan](tool-platform-closeout-and-next-phase.md) into work.
[ADR 0007](../adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md)
records the decisions behind it.

## Decisions

1. Privileged tools work on macOS and Linux. On Windows they stay off.
2. The Daemon runs every privileged tool in a native OS sandbox. Seatbelt is
   the macOS sandbox. Bubblewrap is the Linux sandbox.
3. Web search, web fetch, and the new search tools stay in the Worker.

## The provider contract (TP7)

A provider is a group of tools that the Daemon owns. It has these parts:

- **Tool identity.** A built-in tool id is `nook:<name>`. An MCP tool id is
  `mcp:<server>:<tool>`. The model sees `<name>` or `mcp__<server>__<tool>`,
  cut to 64 characters of `a-z`, `0-9`, `_`, and `-`.
- **Descriptor.** Each tool has an id, a model name, a description, and a JSON
  Schema for its arguments.
- **Facts.** Before Cedar runs, the provider turns the arguments into facts:
  a resource summary and Cedar context values such as `path`, `root`,
  `executable`, `server`, and `tool`. The Daemon adds `chat_id`, `approval_tier`,
  `grant_scope`, and the argument digest.
- **Call.** After approval, the provider runs the tool with a cancel signal and
  returns text plus an error flag.
- **Limits.** Timeouts, output size, and file size come from `config.toml` and
  have hard maximums in the Daemon.
- **Failure messages.** A tool returns short fixed messages such as `Path is
  outside the allowed roots`. It never returns a stack trace, a host path
  outside a root, or an environment value.

### Messages

| Direction | Message | Purpose |
| --- | --- | --- |
| Daemon to Worker | `tool_catalog` | Lists the Daemon tools with schemas. Sent once after `ready`. |
| Worker to Daemon | `execute_tool` | Asks the Daemon to authorize and run one call. |
| Daemon to Worker | `tool_result` | Returns `ok`, `error`, or `denied` with text. |

The Worker builds one LangChain tool for each catalog entry. Each tool checks
the arguments against the schema and sends `execute_tool`. The Daemon sends
`tool_call_started` and `tool_call_finished` to the Panel itself.

A denial ends the reply, as it does for web tools. A cancel aborts the running
call and kills its process group.

### Sandbox

One Rust interface runs a command with a read-root list, a write-root list, a
network flag, a filtered environment, a working directory, a timeout, and an
output limit. The self-test starts when the Daemon starts. A failure removes
terminal and MCP tools from the catalog and logs the reason.

Done when:

- A prototype proves the sandbox blocks reads outside the roots, writes outside
  the roots, network access, and environment leaks, and enforces timeout and
  output limits, on macOS and Linux.
- A call through `execute_tool` returns a result, a denial, and a cancel.

## TP9. Filesystem tools

Tools: `read_file`, `list_directory`, `write_file`.

- Config: `[tools.filesystem]` with `roots`. Each root has an absolute `path`
  and `access` of `read` or `read_write`.
- A path must be absolute and inside a root. The Daemon opens it through
  capability handles, so `..` and symlinks cannot leave the root.
- A symlink that points outside its root is an error. A symlink inside the root
  is followed.
- `write_file` refuses to replace a file unless `overwrite` is true.
- Size limits: 1 MiB to read, 1 MiB to write.
- Cedar actions: `fs_read`, `fs_list`, `fs_write`. Context: `path`, `root`.
- Grants: `once`, `chat_path` (this exact path), and `chat_dir` (this directory
  and everything below it). A write offers only `once` and `chat_path`.

## TP10. Terminal tool

Tool: `run_command` with `argv`, `cwd`, and `timeout_seconds`.

- Config: `[tools.terminal]` with `enabled`, `network`, `timeout_seconds`, and
  `max_output_bytes`. It needs at least one filesystem root.
- The command runs in the sandbox. It can read the roots and the system
  directories, and it can write to `read_write` roots. The default network is
  off.
- The environment holds only `PATH`, `HOME` (a private temporary directory),
  `LANG`, and `TERM`.
- `argv[0]` is a bare name found on a fixed `PATH`, or an absolute path inside a
  root or a system directory.
- Cedar action: `run_command`. Context: `executable`, `cwd`. The policy forbids
  `sudo`, `su`, and `doas`.
- Grants: only `once`. Every command asks.

## TP8. MCP servers

- Config: `[[tools.mcp_servers]]` with `name`, `command`, `args`, `env`, and
  `network`.
- The Daemon starts each server inside the sandbox, speaks MCP over stdio
  (`initialize`, `tools/list`, `tools/call`), and stops it on exit.
- A tool is exposed only if its name, description, and input schema are valid
  and its id does not collide with another tool. An invalid tool or a server
  that fails to start exposes nothing, and the Daemon logs why.
- Cedar action: `mcp_call`. Context: `server`, `tool`.
- Grants: `once` and `chat_tool` (this tool for this chat).
- The YAML entry `mcp:*` sets the display name and the approval actions for all
  MCP tools.

## TP12. Policy preferences and persistent grants

- The Daemon stores persistent grants in PostgreSQL and loads them at startup.
- A grant shows its tool, scope, and target. The Panel lists grants and revokes
  them.
- A user can set a tool's approval preference to `ask` or `deny`. The Daemon
  validates the change against the tool catalog and applies it only if it
  validates. Cedar stays the final decision: a preference never turns a Cedar
  forbid into an allow.
- Persistent grants exist for `web_fetch` per host (`always_allow_host`) and for
  `mcp_call` per tool (`always_allow`). Filesystem and terminal tools do not
  offer a persistent grant.

## TP13. Search providers

Tools: `news_search`, `image_search`, `video_search`, `book_search`. They run in
the Worker with DuckDuckGo and return JSON with a fixed result model. Each has a
YAML entry in the `allow` tier, a Cedar action, a result cap, and tests for
denial and for untrusted content.

## TP11. Browser and native-system designs

Two documents in `docs/design/` (`browser-automation.md` and `native-system-tools.md`) define browser automation and native-system
tools. Each names the platforms, the credential and cookie rules, the download
rules, the approval content, the isolation, and the tests. They add no code.

## Order of delivery

One branch and one commit series for each item, stacked: TP7, TP9, TP10, TP8,
TP12, TP13, TP11.

## Verification

- Rust: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`.
- Linux: the same tests inside the `rust:1-bookworm` container with bubblewrap
  installed, using `--security-opt seccomp=unconfined --security-opt
  apparmor=unconfined --security-opt systempaths=unconfined`.
- Worker: `ruff`, `pyright`, `pytest`.
- Contracts: `pytest tests/contracts`.
- Panel: lint, unit tests, build, and the deterministic native suite.
