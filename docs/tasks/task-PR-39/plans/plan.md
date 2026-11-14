# Plan: Google Docs through an MCP server

Spec: `docs/tasks/task-PR-39/specs/design.md`.

Rules for every task:

1. Write the failing test first, run it, and see it fail for the reason that the
   task names. Then write the code.
2. Check for each Daemon task, from `daemon/`: `rtk cargo test` and
   `rtk cargo clippy --workspace --all-targets -- -D warnings`. Both must print
   zero warnings and zero failures. The sandbox tests skip with a message on
   this machine (`bwrap` has no network namespace here). CI runs them with
   `NOOK_REQUIRE_SANDBOX=1`, so a sandbox test is proved on the pushed head.
3. Check for each Markdown-only task: `git diff --check origin/main HEAD` prints
   nothing.
4. Windows: `unix`-only code carries `#[cfg(unix)]`, so that
   `Windows named-pipe IPC` still builds.
5. A task adds no file outside its list. A needed extra file is a gap.

## Tasks

- [x] **1. Secret store module.** New `daemon/crates/nook-core/src/secrets.rs`,
  `daemon/crates/nook-core/src/lib.rs`. The module holds the trait
  `SecretStore` (`get`, `set`, `delete`, synchronous), the enum `SecretError`
  (missing, unavailable, unreadable) with a fixed text for each case and no raw
  error, `MemorySecretStore`, the refusal of the name `model-api-key` and of an
  empty name in every store call, the shared mask list type
  `SecretMask = Arc<RwLock<Vec<String>>>` with an `add` that skips values under
  4 characters, and `set_from_reader(store, name, reader)` (trims one trailing
  `\r\n` or `\n`, refuses an empty value). Unit tests (spec "Unit tests" 1, 3
  and 4): `the_memory_store_returns_what_was_set_and_nothing_after_delete`,
  `set_from_reader_trims_one_trailing_newline_or_crlf_and_refuses_an_empty_value`,
  `the_model_key_name_is_refused_by_every_store_call`. Each fails first because
  the module does not exist. Check: Daemon `rtk cargo test` and clippy.
- [x] **2. The `env_secrets` field and the store reach the MCP code.**
  `daemon/crates/nook-core/src/config.rs`,
  `daemon/crates/nook-core/src/providers/mcp.rs`,
  `daemon/crates/nook-core/src/providers/mod.rs`,
  `daemon/crates/nookd/src/main.rs`,
  `daemon/crates/nook-core/tests/common/mod.rs`,
  `daemon/crates/nook-core/tests/mcp_tool_tests.rs`, and every other test file
  or module that builds an `McpServerConfig` or calls `McpProvider::start`,
  `start_reporting` or `build_registry` (find them with `grep`; the task commit
  lists them). `McpServerConfig` gets `env_secrets: BTreeMap<String, String>`
  with `#[serde(default)]`. A new type (in `secrets.rs`) bundles the store
  (`Arc<dyn SecretStore>`) and the `SecretMask`. It reaches `build_registry`,
  `McpProvider::start`, `start_reporting`, `ServerSlot::live` and `start_server`
  as one argument. This task changes no behavior: `nookd` passes an empty
  `MemorySecretStore` until task 8, and the existing tests pass one too. This
  is the signature change that the spec asks for first (about 25 tests). Test:
  `an_env_secrets_field_in_config_toml_is_read_by_the_real_parser` (the struct
  has no `deny_unknown_fields`, so assert the parsed value). It fails first
  because the field does not exist. Check: Daemon `rtk cargo test` and clippy.
- [x] **3. The `docs` mode of the fake server and the tests that need no
  secret.** `daemon/crates/nook-core/examples/fake_mcp_server.rs`,
  `daemon/crates/nook-core/tests/mcp_tool_tests.rs`. The fake gets the mode
  `docs` with the tools `readGoogleDoc`, `insertText` (appends a line to
  `inserted.log` in `FAKE_DOCS_RECORD_DIR`), `checkEnv`, `echoEnv` and
  `connect` (uses `TcpStream::connect_timeout`). `normal` mode and its
  exact-list test stay as they are. Tests 1, 2, 3, 4 and 12 of the spec:
  `a_docs_style_tool_is_listed_under_the_hyphenated_server_name` (lists the five
  tool names of the spec), `a_docs_style_call_asks_first_and_shows_the_server_tool_and_arguments`,
  `an_approved_docs_write_reaches_the_server`,
  `a_denied_docs_write_never_reaches_the_server` (tests 3 and 4 share one helper
  for the path of `inserted.log`) and
  `a_docs_server_with_network_true_can_connect_out`. Each fails first because
  the `docs` mode does not exist. Check: Daemon `rtk cargo test` and clippy.
- [x] **4. Resolve and check `env_secrets` when a server starts.**
  `daemon/crates/nook-core/src/providers/mcp.rs`,
  `daemon/crates/nook-core/src/secrets.rs`,
  `daemon/crates/nook-core/tests/mcp_tool_tests.rs`. In `start_reporting`, a
  server is skipped with a fixed report for: a name in both `env` and
  `env_secrets`, an empty secret name, the name `model-api-key`, and a key (in
  `env` or `env_secrets`) with the exact, case-sensitive prefix `LD_` or
  `DYLD_` (the report names the server and the variable, never a value). In
  `start_server`, each secret is read through `spawn_blocking` with a timeout
  of 10 seconds (a timeout is a failure), put in the server `env` after `env`,
  and added to the `SecretMask` before the child starts. A missing secret
  reports `The secret "<name>" is not in the secret store. Run: nookctl secret
  set <name>`; the other `SecretError` cases report their fixed text and the
  name. A restart that fails to read a secret logs the fixed text and the call
  returns `Error: the MCP server stopped`. Tests (spec 5, 6, 8, 9 and 11; test 7 is task 8), with test
  helper stores that return a missing value, an error and no answer:
  `a_secret_from_the_store_reaches_the_server_as_an_env_value`,
  `a_missing_secret_keeps_the_server_off_and_names_the_secret`,
  `a_store_that_never_answers_keeps_the_server_off_after_the_timeout`
  (other servers start), `a_store_error_keeps_the_server_off_with_the_fixed_text`
  (a helper store returns each `SecretError`; the report holds the fixed text and
  the name, never the raw error), `a_restart_with_a_missing_secret_logs_a_fixed_text_and_the_call_fails`, and the
  skip tests `a_server_that_lists_one_name_in_env_and_env_secrets_is_skipped_with_a_report`,
  `an_env_secrets_entry_with_an_empty_name_is_skipped_with_a_report`,
  `an_env_secrets_entry_for_the_model_key_is_skipped_with_a_report`,
  `a_library_variable_name_in_env_is_skipped_with_a_report` (cases `LD_PRELOAD`
  and `DYLD_INSERT_LIBRARIES`),
  `a_library_variable_name_in_env_secrets_is_skipped_with_a_report` and
  `a_name_that_only_looks_like_a_library_variable_is_accepted` (`LDAP_URL`; the
  skip tests also assert that the report holds no value). The skip tests sit
  next to `servers_that_cannot_start_or_have_bad_settings_are_skipped`. Each
  fails first because the check does not exist. Check: Daemon `rtk cargo test`
  and clippy.
- [x] **5. Scrub resolved secrets from tool results.**
  `daemon/crates/nook-core/src/providers/mcp.rs`,
  `daemon/crates/nook-core/tests/mcp_tool_tests.rs`. `McpProvider::call`
  replaces each resolved value of 4 or more characters, from the `SecretMask`,
  with `***` in the result text and in the error text. The tool list is not
  scrubbed. Test:
  `a_secret_printed_by_a_server_is_scrubbed_from_the_result` (`echoEnv` for the
  client secret returns `***`). It fails first because the scrub does not
  exist. Check: Daemon `rtk cargo test` and clippy.
- [x] **6. Redacting log writer in the Daemon.**
  `daemon/crates/nook-core/src/daemon_log.rs`,
  `daemon/crates/nook-core/src/providers/mcp.rs`,
  `daemon/crates/nookd/src/main.rs`,
  `daemon/crates/nookd/src/shutdown.rs` (only if the writer wraps `SafeStderr`
  there). A writer type in `daemon_log.rs` wraps a writer, reads the
  `SecretMask`, and redacts each log event as one write (or buffers by line). In
  `nookd/src/main.rs`, the `tracing_subscriber` writer is wrapped by it, and the
  mask is seeded with `secrets_to_redact(config, api_key)` at start. The same
  mask object reaches task 2's bundle, so `start_server` fills it. The server
  stderr line that the Daemon logs at `debug!` goes through the same redaction
  function. The Panel keeps its own pass. Test (spec "Unit tests" 5):
  `the_redacting_log_writer_masks_a_value_that_was_added_after_it_started`,
  through the real writer, plus a test that the seeded list masks an `env`
  value and the database password. Each fails first because the writer does not
  exist. Check: Daemon `rtk cargo test` and clippy.
- [ ] **7. Environment of the sandbox off the `bwrap` command line (G8).**
  `daemon/crates/nook-core/src/sandbox.rs`,
  `daemon/crates/nook-core/tests/sandbox_tests.rs`. For the `Bubblewrap`
  backend, `command.envs(&spec.env)` replaces the `--clearenv` and `--setenv`
  arguments (the command already calls `env_clear()`), and `PATH` for `bwrap`
  itself stays the sandbox `PATH`. macOS does not change. Tests (Linux only):
  `the_bwrap_command_line_holds_no_environment_value` (a server with a secret
  runs, and the `bwrap` command line holds no value), and the change of
  `the_bubblewrap_arguments_unshare_the_network_unless_allowed` (the assertion
  at line 446 no longer expects `--clearenv`). Make sure that a command still
  sees its `env` inside the sandbox (an existing test or a new one with the
  value read from inside). The new test fails first because the arguments hold
  the value. Check: Daemon `rtk cargo test` and clippy. The Linux sandbox tests
  run in CI.
- [ ] **8. The operating system store.** `daemon/Cargo.toml`,
  `daemon/crates/nook-core/Cargo.toml`, `daemon/crates/nookd/Cargo.toml`,
  `daemon/crates/nookctl/Cargo.toml`, `daemon/Cargo.lock`,
  `daemon/crates/nook-core/src/secrets.rs`, `daemon/crates/nookd/src/main.rs`,
  `daemon/crates/nook-core/tests/mcp_tool_tests.rs`. `OsSecretStore` holds a
  `keyring-core` store handle and builds one entry for each name under the
  service `Nook MCP`. The crates are `keyring-core`,
  `apple-native-keyring-store` (feature `keychain`),
  `windows-native-keyring-store` and `zbus-secret-service-keyring-store`
  (feature `crypto-rust`), each in the `cfg(target_os)` table of its system and
  behind the Cargo feature `os-store` of `nook-core`. `nookd` and `nookctl`
  enable the feature, and the Panel does not. A store that cannot be built (no
  D-Bus session) answers `SecretError` "unavailable" and nothing crashes. `nookd`
  passes `OsSecretStore` instead of the empty store of task 2. Verify the
  versions on crates.io on the day (spec: `keyring` 4.2.0, `keyring-core` 1.0.1)
  and commit `Cargo.lock`. Tests: `the_os_store_round_trips_through_the_keyring_core_mock_store`
  and the real `a_store_error_that_holds_the_secret_bytes_never_reaches_a_report_or_the_log`
  (an `OsSecretStore` over `keyring_core::mock::Store` whose credential fails
  with `BadEncoding(<bytes>)`; neither the report nor the captured log holds the
  bytes or their text). Make sure that the Panel crate does not link the store
  crates (`cargo tree -p` for the Panel, or a feature check). Each test fails
  first because `OsSecretStore` does not exist. Check: Daemon `rtk cargo test`
  and clippy, and the `cargo tree` check in the report.
- [ ] **9. `nookctl secret set` and `delete`.**
  `daemon/crates/nookctl/src/main.rs`, `daemon/crates/nookctl/Cargo.toml`,
  `daemon/Cargo.toml`, `daemon/Cargo.lock`, and a new test file
  `daemon/crates/nookctl/tests/secret_tests.rs`. A `secret` subcommand with
  `set <name>` and `delete <name>`. Both run before the Daemon socket check, so
  they need no running Daemon. `set` reads the value from standard input and
  takes no value argument. On a terminal it turns echo off with `rpassword`
  (the report names its license and version). Errors print the fixed text of
  `SecretError`. Test: `nookctl_secret_set_reads_standard_input_and_takes_no_value_argument`
  (a binary-level test that runs the built `nookctl` with no Daemon socket: `secret
  set a b` is refused by the argument parser because a value argument does not
  exist, and `secret set a` with empty standard input fails with the fixed
  "value is empty" text and does not print the "Is nookd running?" text). No
  test writes to a real store. It fails first because the subcommand does not
  exist. Check: Daemon `rtk cargo test` and clippy.
- [ ] **10. Docs: `docs/mcp-servers.md`.** `docs/mcp-servers.md`. All the page
  changes of the spec ("Central docs that change" 1.1 to 1.11): the `env_secrets`
  row and a section on the secret store and `nookctl secret` (with where to get
  `nookctl`); the section "Example: Google Docs" with the sample block, the 15
  steps, the date of the check, the tool list for the narrow scope (from the
  source, not a live run), the scope behavior of the spec "The server" point 5,
  `network = true` and the document-text warning; the section "When Google Docs
  stops working" (6 causes); the section "Next steps for Google Workspace" with
  the three follow-up tasks; the fixes to "A login in a browser", "Put a secret
  in `env`", the "Environment" bullet, the `daemon.log` paragraph, the `env` row
  (the `LD_` and `DYLD_` rule), "What cannot work", and the "For developers"
  bullet that names the `docs` mode. Write the page in plain English (the
  `simple-english` skill). The sample block must be the first `toml` block under
  the heading "Example: Google Docs". Check: `git diff --check origin/main HEAD`
  prints nothing.
- [ ] **11. The sample test.** `daemon/crates/nook-core/src/providers/mcp.rs`,
  `daemon/crates/nook-core/tests/docs_sample_tests.rs`. `valid_server_name`
  becomes a `pub` helper that the loader and the test both call (note N1).
  The new file holds `the_google_docs_sample_block_loads_as_a_valid_server`,
  with `#[cfg(unix)]`: it finds `docs/mcp-servers.md` from `CARGO_MANIFEST_DIR`,
  takes the first `toml` block under "Example: Google Docs", wraps it with
  `base_url` and `model`, parses it with `toml::from_str::<Config>`, and asserts
  the name (through the `pub` helper), `network`, the `env` keys, the
  `env_secrets` keys, that no secret name appears in `env`, that no key has an
  `LD_` or `DYLD_` prefix, and that every path is absolute. It cannot fail first,
  because task 10 wrote the page. The implementer proves the test instead: run
  it against a copy of the page without the block and with a bad block (an
  `LD_PRELOAD` key), see it fail, and restore the page. Check: Daemon `rtk cargo test`
  and clippy.
- [ ] **12. Docs: testing, architecture, ADR, changelog.** `docs/testing.md`,
  `docs/architecture.md`, `docs/adr/<next number>-secrets-live-in-the-operating-system-secret-store.md`,
  `CHANGELOG.md`. `docs/testing.md`: one paragraph on the Docs-style MCP tests,
  the store tests, the sandbox environment test and the sample test.
  `docs/architecture.md`: one sentence next to the Keychain line (line 40) that
  names the secret store and its use for MCP servers. The ADR "Secrets live in
  the operating system secret store behind one abstraction": the context, the
  decision (`SecretStore`, `env_secrets`, service `Nook MCP`), and the effects
  (a headless Linux Daemon cannot use `env_secrets`; the model key stays on
  `keychain.rs` until a follow-up). Take the ADR number from the highest number
  on `origin/main` on the day (0008 on 2026-11-14; run `git fetch` first).
  `CHANGELOG.md` under "Unreleased": an "Added" line for Google Docs, the secret
  store, `env_secrets` and `nookctl secret`; a "Changed" line that says a server
  with an `env` name that starts with `LD_` or `DYLD_` no longer starts, and
  that on Linux the server environment no longer shows in the process command
  line. No version bump. Check: `git diff --check origin/main HEAD` prints
  nothing.

## Order and why

1. Tasks 1 and 2 come first, as the spec asks: the signature change reaches
   about 25 tests, so it lands alone with no behavior change.
2. Task 3 (fake `docs` mode) comes before task 4 because tests 5 to 9 use it.
3. Tasks 4 and 5 add behavior to `mcp.rs` and share the `SecretMask`. Task 6
   reads the same mask. Task 7 (sandbox) is independent of 4 to 6 and follows
   them so that one implementer writes to the branch at a time.
4. Task 8 comes after 4 so that the real error test can use the real store.
5. Docs tasks 10 and 12 come after the code they describe. Task 11 (sample
   test) comes after task 10 because it reads the page.
6. Stage 6 hands tasks 1 to 2, 3 to 5, 6 to 7 and 8 to 9 to implementers in
   that order, then 10 to 12.

## Map: spec and decisions to tasks

| Decision or spec part | Task |
| --- | --- |
| G1 package, G2 scope, G3 audience (sample, steps, causes) | 10 |
| G2 and G5 follow-ups, and the third follow-up (API key on the store) | 10 |
| G4 and G7: `SecretStore`, `SecretError`, `MemorySecretStore`, name rule, service `Nook MCP` | 1, 8 |
| G4 and G7: `env_secrets` field and resolution | 2, 4 |
| G7: `nookctl secret` | 9 |
| G6 live check | stage 7 and gate 2 (the owner), page text in 10 |
| G8 Linux environment off the command line | 7 |
| G9 `LD_*` and `DYLD_*` | 4 (check and tests), 10 (page), 12 (changelog) |
| Masking and the redacting writer (S9) | 6 |
| Scrub of results (S17) | 5 |
| Timeout and the restart report (S12) | 4 |
| Store error texts (S16) | 1, 4, 8 |
| Features and an unbuildable store (S24, S25) | 8, 4 |
| `docs` fake mode and tests 1 to 4, 12 | 3 |
| Sample test and N1 | 11 |
| N3 (`connect_timeout`), N4 (shared path helper) | 3 |
| Central docs, ADR, changelog | 10, 12 |
