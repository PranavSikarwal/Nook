# Secrets live in the operating system secret store behind one abstraction

## Status

Accepted

## Date

November 14, 2026

## Context

The Google Docs server needs a client secret and a refresh token. Nook forbids secrets in configuration files and plain text files. `keychain.rs` works only on macOS and handles only the model API key. It does nothing on Linux and Windows. Nook needs one cross-platform abstraction for secrets that integrates with native system keyrings.

## Decision

1. PR 39 introduces the synchronous `SecretStore` trait in `daemon/crates/nook-core/src/secrets.rs`. The trait provides `get`, `set`, and `delete` methods.
2. Production code uses `OsSecretStore` built on the `keyring-core` crate. Three store crates run behind the Cargo feature `os-store`: `apple-native-keyring-store` on macOS, `windows-native-keyring-store` on Windows, and `zbus-secret-service-keyring-store` on Linux. `nookd` and `nookctl` enable this feature. The Panel crate does not link the store crates.
3. Secrets live under the service name `Nook MCP`. Every store call refuses an empty secret name and refuses the name `model-api-key`.
4. MCP server configuration adds the `env_secrets` table. It maps environment variable names to secret names in the store.
5. The CLI tool `nookctl` adds `secret set <name>` and `secret delete <name>`. The `set` command reads the secret value from standard input. It never accepts a value as a command-line argument.
6. The store returns fixed error texts through `SecretError` for missing, unavailable, and unreadable secrets. It never formats raw backend error objects because raw errors hold secret bytes.
7. The Daemon reads secrets through `spawn_blocking` with a 10 second read timeout. A timeout keeps the server off for the session.
8. The Daemon masks secret values in its log writer using a shared `SecretMask`. `McpProvider::call` scrubs resolved secret values of four or more characters from tool results and tool errors.
9. On Linux, the sandbox runner no longer passes the server environment on the `bwrap` command line. The Daemon passes variables directly to the `bwrap` process environment.
10. The Daemon refuses server environment names that start with the case-sensitive prefixes `LD_` or `DYLD_` in `env` and `env_secrets`.

## Consequences

- A headless Linux Daemon without a running Secret Service or D-Bus session cannot use `env_secrets`. The Daemon fails closed and leaves the affected server off.
- The model API key stays on `keychain.rs` until a follow-up task moves it.
- The crate introduces a new dependency tree for `keyring-core`, `zbus`, and system credential store bindings.
- Scrubbing resolved secrets from tool results is best effort. A server that encodes or fragments a secret bypasses the text match.
