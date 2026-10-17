# The Daemon runs privileged tools in a native OS sandbox

## Status

Accepted

## Date

October 17, 2026

## Context

Stage 2 of the tool platform adds tools that read and write local files, run
commands, and call MCP servers. These tools can harm the user's machine if a
model, a prompt injection, or a bug controls their arguments.

ADR 0004 makes Cedar in the Daemon the final authorization engine. Today the
Worker runs every tool after the Daemon allows it. For a privileged tool that
leaves two steps, authorization and execution, in two processes. A bug in the
Python Worker could run the tool without the sandbox or with other arguments
than the ones the user approved.

Nook supports macOS and Linux for privileged tools. Windows has no sandbox yet.

## Decision

1. The Daemon runs every privileged tool. The Worker sends one `execute_tool`
   request. The Daemon normalizes it, asks Cedar, asks the user, runs the tool,
   and sends the output back in `tool_result`. The arguments that run are the
   arguments that were authorized. Web search and web fetch stay in the Worker
   because they hold no local authority.
2. The Daemon runs commands and MCP servers inside a native OS sandbox behind
   one Rust interface. macOS uses Seatbelt (`sandbox-exec`) with a
   deny-by-default profile. Linux uses bubblewrap (`bwrap`) with unshared
   namespaces. Each run gets explicit read roots, write roots, a network flag, a
   filtered environment, a timeout, and an output limit.
3. File tools run in the Daemon through capability handles on the configured
   roots, so a path cannot leave a root through `..` or a symlink.
4. The Daemon fails closed. If the sandbox is missing or its self-test fails,
   the Daemon does not expose terminal or MCP tools. On Windows it exposes
   neither.
5. A new Cedar action exists for each privileged tool. Policies can forbid by
   executable name, root, or tool. YAML never overrides Cedar.

## Alternatives considered

### Container or virtual machine

A container gives strong isolation and the same behavior on both systems. The
user must install and run a container engine, startup is slow, and mapping
approved folders adds a second set of path rules.

### WebAssembly runtime

A WebAssembly runtime is strict and portable. It cannot run ordinary shell
commands or existing binaries, so it does not fit a terminal tool.

### The Worker runs tools with a sandbox profile from the Daemon

This changes less of the current message flow. It leaves authorization and
execution in two processes, and the Python Worker would hold the power to skip
the sandbox.

### A newer Apple mechanism on macOS

Checked on October 17, 2026. Apple has published no replacement for
`sandbox-exec` that confines a single macOS process. App Sandbox needs a signed
app with entitlements and covers the whole app, not one child. Endpoint
Security and System Extensions observe processes and do not confine them. The
Containerization framework and the `container` tool run Linux in a virtual
machine, so a command would see Linux and not the user's macOS tools. Apple
issue [containerization#737](https://github.com/apple/containerization/issues/737),
opened May 12, 2026, asks for a replacement and a "lite" mode. It is still open
with no Apple answer. The `Backend` enum in `sandbox.rs` is the place for a
successor when one exists.

### Landlock alone on Linux

Landlock restricts file access from inside the process and needs no helper
binary. Its network rules need a recent kernel, and it cannot hide the rest of
the file system. Bubblewrap gives a private mount, process, and network
namespace on older kernels.

## Consequences

- The Worker needs a tool catalog from the Daemon, because the Daemon owns the
  privileged tools. The Daemon sends it right after the Worker reports ready.
- The Daemon owns timeouts, output limits, and cancellation for these tools.
- Seatbelt is deprecated by Apple, but current browsers and coding agents still
  rely on it. If Apple removes it, macOS terminal and MCP tools stop until a
  replacement exists. This ADR then needs a successor. The Daemon checks the
  sandbox at every start and turns the tools off when the check fails, so a
  removal shows up as missing tools and not as an unsandboxed run.
- Bubblewrap must be installed on Linux. The Daemon reports when it is missing.
- Tests must prove each boundary on macOS and Linux: reads outside the roots,
  writes outside the roots, network access, environment leaks, timeout, and
  output size.

## Sources

- [Bubblewrap](https://github.com/containers/bubblewrap)
- [cap-std, capability-oriented file system access](https://github.com/bytecodealliance/cap-std)
