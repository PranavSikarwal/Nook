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

The package workflow runs `nookd --check-policy` on the staged Daemon. It runs
once with `NOOK_POLICY_DIR` set and once from a copy of the resource layout
without it. `scripts/bundle.sh` runs the same check on the macOS bundle. These
steps passed on macOS, Ubuntu, and Windows in package workflow run 38966546745. Fetcher tests in
`worker/tests/test_fetch_redirects_and_guards.py` cover redirects and the
address guards. The next tool platform must test
sandbox boundaries before it exposes terminal, filesystem, browser, or
native-system access.
