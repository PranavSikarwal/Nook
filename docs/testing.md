# Testing

Nook uses unit, contract, package, and native desktop tests. The command depends
on the component that changed.

## Component checks

Run Daemon tests and Rust checks from `daemon/`:

```sh
rtk cargo test
rtk cargo clippy --workspace --all-targets -- -D warnings
```

Run Worker tests from `worker/`:

```sh
rtk uv run pytest
```

Run the Panel checks from `panel/`:

```sh
rtk npm run lint
rtk npm run test:unit
rtk npm run build
```

## Contract checks

The schema examples protect compatibility across Panel, Daemon, and Worker.
Run this command after changing protocol files:

```sh
rtk uv run --with pytest --with jsonschema pytest tests/contracts
```

## Native and package checks

The native Tauri test gate contains deterministic desktop scenarios. The live
baseline uses repository secrets and does not validate pull request code.
Package workflows inspect the macOS, Debian, and Windows artifacts for the
bundled Daemon and Worker runtime.

## Tool-platform checks

Rust tests in `daemon/crates/nook-core/tests/tool_authorization_tests.rs` use
the real policy files and a fake Worker. They cover startup rejection of
malformed policy, Cedar decisions, grant scopes, denial, cancellation, and chat
deletion. `src/policy_assets.rs` and `src/authorization.rs` hold unit tests for
policy loading and for host and digest normalization.

Worker tests in `worker/tests/test_authorization.py` cover the wait for a
Daemon result, mismatched ids, timeout, and cancel.

Sandbox tests in `daemon/crates/nook-core/tests/sandbox_tests.rs` prove the
boundary on the real OS sandbox: reads and writes outside the roots, network
access, environment leaks, working directory, timeout, output limit, and process
group cleanup. They skip with a message when the sandbox is missing, unless
`NOOK_REQUIRE_SANDBOX` is set. Run them on Linux from the repository root:

```sh
scripts/test-linux.sh --test sandbox_tests
```

The script starts a `rust:1-bookworm` container with bubblewrap and relaxed
container security options, because bubblewrap needs namespaces and `/proc`.
Without arguments it runs clippy and the whole Daemon test suite.

`filesystem_tool_tests.rs`, `terminal_tool_tests.rs`, and `mcp_tool_tests.rs` run
the real policy files, a real provider, and the fake Worker through the whole
approval flow. The MCP tests start `examples/fake_mcp_server.rs` inside the
sandbox. Cargo builds that example with `cargo test`.

`permissions_tests.rs` drives the permission messages through a real socket and
checks permanent grants, revoking, and tool preferences with the real policy.
`permissions_db_tests.rs` stores and restores them in a temporary PostgreSQL
database and skips when PostgreSQL is not reachable.

`mcp_tool_tests.rs` also kills a server in the middle of a call. It checks that
the next call starts the server again and that a server which dies soon after a
start stays stopped. `permissions_db_tests.rs` covers the late database
connection: it keeps trying while PostgreSQL is down, then applies the
migrations and loads the stored grants.

`provider_execution_tests.rs` runs the Daemon execute path with a test provider
and the fake Worker. It covers approval, denial, grants, errors, the output
limit, invalid arguments, the Worker and Daemon tool mix-ups, expiry, and
cancel.

`panel/e2e/specs/tools.spec.mjs` opens Settings, Tools. It checks that the file
tools, the command tool, and a working MCP server show as ready, that a server
with a missing program shows as off with a reason, and that Refresh reloads the
list. The shared `before` hook in `panel/e2e/wdio.conf.mjs` waits until the Panel
enables its input, because the Panel keeps its input disabled until its own
Daemon check passes.

`folders_tests.rs` runs the Settings, Folders requests against the real tools. It
checks that an added folder works at once and reaches `config.toml`, that the
tool descriptions and the Worker catalog follow a change (also an empty one), that
a read-only change and a removal refuse new calls, and that a waiting approval in
a removed folder is denied while one in another folder stays. `folders.rs` holds
unit tests for the path checks. `panel/e2e/specs/folders.spec.mjs` opens the tab
through the `open_settings` event that the macOS menu sends, adds a folder (the
test build reads it from `NOOK_E2E_PICK_FOLDER`), and writes there through the
approval flow. WebDriver cannot click the macOS menu or the system folder picker.
Check the menu by hand.

`panel/e2e/specs/links.spec.mjs` clicks a link in a reply. It checks that the Panel
window stays put, that the link reaches the `open_external_link` command, and that
a `javascript:` or `file:` address is not a link. The test build never starts a
browser. It appends the link to `opened-links.log` in the run folder.

The deterministic native tests use `panel/e2e/fake_worker.py`. It sends
`authorize_tool`, waits for `authorization_result`, and sends tool events only
after `allow`. `panel/e2e/test_fake_worker.py` checks every message it sends
against the schema. Start a native run from `panel/`:

```sh
export NOOK_E2E_DATABASE_URL=postgresql://USER@localhost:5432/postgres
rtk npm run test:e2e:deterministic
```

The runner stops the test Daemon when the run ends. It finds the process
through the `daemon.sock` file in the run's own app directory.

A local Postgres that trusts local logins needs no password in the URL. Leave it
out, because a password with `@` or `/` must be percent-encoded and can end up in
an error message. For a server that needs one, write `USER:PASSWORD@`. The role
in the URL must be able to create databases. Each run creates and drops one
database named `nook_e2e_` followed by an id. A failed run can leave one
behind. Drop it by hand.

The live native test calls your model endpoint. It reads `NOOK_BASE_URL`,
`NOOK_MODEL` and `NOOK_API_KEY` from the shell. Load them from `.env` without
printing them, then run `rtk npm run test:e2e:live`. It covers a news query and
the fetch flow: deny, allow once, a chat and host grant, and cancel.

`rtk npm run test:e2e:live-tools` runs the live file and command scenarios with
your model. The test config lists one writable folder in a temporary directory.
The prompts name no path, so the model must take the path from the tool
descriptions. The scenarios create a file, replace it, try a write outside the
folder, and run `ls`. Set `NOOK_MODEL` to the model you use, because a model
that finds the path on its own does not show the fault.

`scripts/package-approval-check.py` runs the staged release Daemon, the staged
Worker (the PyInstaller build), and the staged policy through two tool calls. It
plays the Panel and a fake model endpoint. The fake model asks for `web_fetch`
in each chat. The script allows the first call and denies the second. It checks
that the allowed reply resumes with the model's answer, that the model received
the tool result, and that the denied call never starts and never reaches the
model. It needs an empty PostgreSQL database that the caller creates and drops.
The package workflow runs
it on macOS and Ubuntu. It does not run on Windows, because it uses a Unix
socket. Run it by hand from a staged layout:

```sh
createdb nook_pkg_check
python3 scripts/package-approval-check.py \
  --resources panel/src-tauri/resources \
  --database-url postgresql://USER@localhost:5432/nook_pkg_check
dropdb nook_pkg_check
```

The package workflow runs `nookd --check-policy` on the staged Daemon. It runs
once with `NOOK_POLICY_DIR` set and once from a copy of the resource layout
without it. `scripts/bundle.sh` runs the same check on the macOS bundle. These
steps passed on macOS, Ubuntu, and Windows in package workflow run 38966546745. Fetcher tests in
`worker/tests/test_fetch_redirects_and_guards.py` cover redirects and the
address guards. The next tool platform must test
sandbox boundaries before it exposes terminal, filesystem, browser, or
native-system access.
