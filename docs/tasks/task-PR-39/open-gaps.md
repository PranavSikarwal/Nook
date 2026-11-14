# Open gaps

## G1. Which MCP server package the docs name

- Stage: 2 spec
- Question: Which Google Docs MCP server does `docs/mcp-servers.md` name in its sample?
- Options:
  1. `@piotr-agier/google-drive-mcp`, pinned to `2.13.0`. MIT license, last release 2026-10-15, Node 18 or newer. It covers Drive, Docs, Sheets, Slides and Calendar (130 tools). The scopes can be narrowed with `GOOGLE_DRIVE_MCP_SCOPES`. The credentials path and the token path are settings. Checked on 2026-11-14: it starts and lists tools with no token.
  2. `@a-bonus/google-docs-mcp`, version `1.11.3`. Its tool list is closer to Docs, and `MCP_TOOL_GROUPS` may filter tools (the README lists it only under remote deployment, so stdio is not confirmed). Its scopes are fixed in code and include `gmail.modify`, `drive`, `calendar.events` and `script.projects`. They cannot be narrowed. The README says MIT and the npm record says ISC. Its README starts the server with `npx`, which `docs/mcp-servers.md` forbids.
  3. Name no package. The page lists the requirements and the settings only. The owner chooses the server.
- Recommendation: Option 1. It is the only checked option whose scopes the owner can narrow to one Docs scope. Option 2 asks Google for Gmail access that Nook does not need. Option 3 leaves the owner with no tested sample. With any option, the owner also accepts `network = true` for a third-party server that can read the token and the client secret. No Nook setting limits the hosts.
- Decision: Option 1: `@piotr-agier/google-drive-mcp` pinned to `2.13.0`. The owner accepts `network = true` for this third-party server. "Follow recommendation."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G2. OAuth scopes in the sample

- Stage: 2 spec
- Question: Which scopes does the sample ask Google for? The owner pastes the same value into the consent screen and into `GOOGLE_DRIVE_MCP_SCOPES`.
- Options:
  1. `documents` only (`https://www.googleapis.com/auth/documents`). Read and edit an existing Google Doc. In version 2.13.0 (read from the source, not run), `readGoogleDoc` and `insertText` work, and `createGoogleDoc` fails, because it calls `drive.files.create`. The Drive tools (list, search, share, delete, upload) fail inside the server, which refuses a tool whose scope the env value does not cover (read from the source by the round 3 reviewer). The owner gives the model the document address. Google's pages that were read do not say whether this scope is marked sensitive (see G3).
  2. `documents.readonly` (full URL, not a short alias of the server). The assistant can read Docs and cannot write. This does not meet "write to online documents" and is the safest start.
  3. `documents` and `drive.file`. Adds Drive access to files that this app created or opened. It lets `createGoogleDoc` work. It adds a second scope to approve. `uploadFile` takes a local path, so with a Drive scope the server could upload any file that it can read, including the credentials file. The approval card shows the path. G5 must be decided again with this option.
  4. The server default: eight scopes, including all of Drive and Calendar. Nook does not need this.
- Recommendation: Option 1. It matches the goal (write to Docs) with the smallest scope, and it cannot create a new document. The owner who needs a new document chooses option 3. The page shows option 2 as a variant for a read-only start.
- Decision: Option 1: `documents` only. Add a follow-up task to the docs: if the `documents` scope works, widen the scope to Sheets, Calendar and the other Google Workspace tools of the server, so that Nook works as a full assistant. "Follow recommendation, and add it to next tasks in docs."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G3. Consent screen audience and token life

- Stage: 2 spec
- Question: Which audience and publishing status does the owner choose for the OAuth client? It decides how long the sign-in lasts.
- Options:
  1. **External, Testing status.** Works for any Google account. Google expires the refresh token 7 days after consent for an app in Testing, so the owner signs in again every week. No Google review. The owner must list their own account as a test user.
  2. **Internal.** Only for a Google Workspace account, only for users of that organization. Google's pages that were read do not state a 7-day limit for it. Not available for a personal `@gmail.com` account.
  3. **External, In production.** The owner publishes the app. Google shows an "unverified app" warning at sign-in if the scope is sensitive, and caps unverified apps at 100 users over the life of the project. Google's pages that were read do not state the refresh-token expiry for this case, so the page cannot promise a lifetime.
- Recommendation: Option 1 first. It works for every account and the owner can publish later without a code change. Write the weekly sign-in into the page, and give the switch to option 3 as the way out. The owner may choose option 2 when the account is a Workspace account.
- Decision: Option 1: External, Testing status. "External (follow recommendation)."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G4. Where the client secret lives

- Stage: 2 spec
- Question: Where does the OAuth client secret go on the owner's machine?
- Options:
  1. In the downloaded JSON file in a private folder (`chmod 700`, file `chmod 600`) that is in `write_paths`. The sample `env` holds only the path (`GOOGLE_DRIVE_OAUTH_CREDENTIALS`).
  2. In `env` of `config.toml` as `GOOGLE_DRIVE_MCP_CLIENT_SECRET`. The server needs a refresh token in `env` for this mode, and `config.toml` stores `env` as plain text.
- Recommendation: Option 1. The secret stays out of `config.toml`, which the owner may share while asking for help. On Linux, Nook passes each `env` value to the sandbox on a command line, so another process of the same user can read it with `ps`. The repository holds no secret in either option.
- Decision: Keep every client secret in the operating system secret store (macOS Keychain, Windows Credential Manager, Linux Secret Service) through one Nook abstraction, not one code path for each system. The secret does not go in a file or in `env` of `config.toml`. Fact found after this decision: Nook has no such abstraction today. `daemon/crates/nook-core/src/keychain.rs` talks to the macOS Keychain only, and on Linux and Windows it does nothing. G7 asks how to build the abstraction. "We should add all client secrets to the keychain ... I want abstraction rather than implementing mac windows and linux chains differently."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G5. The 64-tool cap and the 130 tools of the server

- Stage: 2 spec
- Question: Nook offers the first 64 tools of a server. The recommended server lists 130, and no setting of that server filters them. How does the sample deal with the extra Drive and Sheets tools?
- Options:
  1. Accept it. The narrow scope blocks the Drive calls (the server refuses them), every call asks first, and the page names the effect (context cost, a few failed calls).
  2. Add a per-server tool allowlist to Nook (`tools = [...]` in the server block). This is a code change in `config.rs`, `mcp.rs` and the docs. It is a separate task with its own spec, and it needs a decision on its own gate 1.
  3. Use a server that can filter its tool list (option 2 of G1). It gives up the narrow scope.
- Recommendation: Option 1 for this task, with the scope of G2 option 1. If G2 picks a Drive scope, decide this gap again, because the scope is then no longer a guard. Record option 2 as a follow-up task if the extra tools cost too much in use.
- Decision: Option 1: accept the 64-tool cap for this task. Add a follow-up task to the docs: remove the narrow scope and the cap so that all 130 tools work, when the first tools work well. "Accept it and add to the follow up."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G6. Who checks the live path, and when

- Stage: 2 spec
- Question: CI cannot sign in to Google. The owner creates the OAuth client anyway. Does the owner follow the written steps once on the branch, and report the result, before merge?
- Options:
  1. The owner follows the steps before gate 2 and tells the orchestrator what failed. The orchestrator fixes the page in a round. The docs then say that the steps were run on a named date.
  2. The owner follows the steps after the merge. The docs say that the live path is untested until then.
  3. Nobody checks. The docs say that the live path is untested.
- Recommendation: Option 1. The owner does the work once, and the page is true when it merges. If the owner cannot do it before gate 2, use option 2 and keep the "untested" sentence.
- Decision: Option 1: the owner follows the written steps once before gate 2. "Follow recommendations."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G7. How Nook builds the secret store abstraction

- Stage: 4 gate 1 (found after the owner decided G4)
- Question: G4 needs a secret store that works the same on macOS, Windows and Linux. Nook has none. Where and how does this task build it? The abstraction is new Daemon code, a new dependency and a new setting. It touches how secrets reach the sandbox, so it needs the owner.
- Facts:
  1. `daemon/crates/nook-core/src/keychain.rs` uses `security-framework` and stores one item (service `Nook`, account `model-api-key`). On other systems it returns nothing. It is not an abstraction.
  2. `docs/plans/open-items-fixes.md` item 8 already proposed a setting such as `env_keychain = { NAME = "account" }` and recommended waiting.
  3. The server accepts `GOOGLE_DRIVE_MCP_CLIENT_ID`, `GOOGLE_DRIVE_MCP_CLIENT_SECRET` and `GOOGLE_DRIVE_MCP_REFRESH_TOKEN` in `env` (external token mode). With these three values the server needs no credentials file and no token file. The three values can all come from the secret store. The owner gets the refresh token once with the `auth` command of the server, then puts it in the store.
  4. The `keyring` crate (MIT or Apache-2.0, version 4.2.0 on 2026-11-14) wraps the native secure stores of macOS, Windows and Linux behind one API. Read from its README only: the Linux backend and its need for a desktop session (D-Bus) are not verified. The plan must verify them.
  5. A Daemon with no store (for example a headless Linux server) must fail closed: the server does not start, and the error names the missing secret.
- Options:
  1. In this task, with the `keyring` crate. New module in `nook-core` with a `SecretStore` trait, the `keyring` store and an in-memory store for tests. New server setting `env_secrets = { ENV_NAME = "account" }`: the Daemon reads each value from the store when it starts the server and puts it in the sandbox `env`. New `nookctl secret set <account>` (reads the value from the terminal, never from an argument) and `nookctl secret delete <account>`. The model API key stays on its current code. The spec, the docs, the tests and an ADR change. This task grows from docs and tests to Daemon code.
  2. A separate task first: the secret store abstraction, `env_secrets` and `nookctl secret`, with its own spec, gate 1 and pull request. This task then waits, and changes the Google Docs sample to use the store after that task merges. Smaller reviews. The Google Docs page waits for a full second cycle.
  3. Without the `keyring` crate: write the abstraction by hand over `security-framework` and the Windows and Linux libraries. The owner declined per-system code in G4.
- Recommendation: Option 1, because the owner wants the Docs sample to use the store and the Docs sample is the first user. It needs a second spec review round and a new gate 1 for the changed spec. If the owner prefers smaller reviews, choose option 2. Moving the model API key onto the store is a separate follow-up.
- Decision: Option 1: build it in this task, with the `keyring` crate. "In this task, with keyring crate (Recommended)." The spec now holds the secret store, the `env_secrets` setting and `nookctl secret`. The model API key stays on its current code.
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G8. Pass the Linux sandbox environment without a command line

- Stage: 3 spec review (found by the round 3 reviewer, finding S10)
- Question: On Linux, `sandbox.rs` passes each `env` value of a server to `bwrap` as `--setenv <name> <value>`. Any user of the machine can read these arguments from `/proc/<pid>/cmdline` while the server runs. That includes the new secrets. Does this task fix it? The fix changes the sandbox code for every MCP server and terminal command on Linux.
- Options:
  1. Fix it in this task. Set the environment with `command.envs(...)` on the `bwrap` process (it already starts with `env_clear()`), and drop `--clearenv` and `--setenv`. `/proc/<pid>/environ` is readable by the owner of the process only. The sandbox limits stay the same. `sandbox_tests.rs` changes, and a new Linux test checks that the command line holds no value. macOS does not change.
  2. Keep it and document it. The page and Risk 11 say that any user of a Linux machine can read the secrets while the server runs.
- Recommendation: Option 1. The task exists to keep these values private, and the change is small.
- Decision: Option 1: fix it in this task. "Fix it in this task (Recommended)."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G9. Library variables in the environment of a server

- Stage: 3 spec review (found by the round 4 reviewer, finding S21)
- Question: After G8, the server environment is set on the `bwrap` process itself on Linux. A name such as `LD_PRELOAD` or `LD_LIBRARY_PATH` then changes how `bwrap` loads libraries on the host, before the sandbox starts. How does Nook handle these names?
- Options:
  1. Refuse names that start with `LD_` or `DYLD_`. `start_reporting` skips the server with a fixed report. One test. A server that needs `LD_LIBRARY_PATH` cannot be attached until a later task.
  2. Keep `--clearenv` and pass the values through `bwrap --args <fd>` from a pipe that is closed after the start. No name is refused and the host `bwrap` is not affected. More code and a harder test in `sandbox.rs`.
- Recommendation: Option 1. It is small and safe, and the Google Docs server needs none of these names.
- Decision: Option 1: refuse `LD_*` and `DYLD_*` names. "Refuse LD_* and DYLD_* names (Recommended)."
- Decided by: owner
- Date: 2026-11-14
- Status: decided by owner

## G10. Extra error variant `SecretError::Empty`

- Stage: 7 code review (reported by the implementer of plan task R1-4)
- Question: Plan task R1-4 names the new variant `Refused`. `set_from_reader` also needs to report an empty value. Does it get its own variant?
- Options:
  1. Add `SecretError::Empty` with the fixed text "The value is empty." `nookctl` prints this text and its test stays as it is.
  2. Reuse `Unreadable` for an empty value. The user would see "Secret cannot be read", which is wrong.
- Recommendation: Option 1. It touches only `secrets.rs` and `nookctl`, changes no interface in `contracts/` or `policy/`, and adds no dependency.
- Decision: Option 1, closed by recommendation by the orchestrator in code round 1.
- Decided by: orchestrator (closed by recommendation)
- Date: 2026-11-14
- Status: closed by recommendation

## G11. Overlapping secrets can leave a part of a secret in masked text

- Stage: 7 code review (reported by the implementer of plan task R1-1)
- Question: The mask replaces secrets longest first. Two secrets that overlap without one containing the other (`xxabcd` and `abcdyy` in the text `xxabcdyy`) can leave a part (`yy`). Does the task mask by match ranges?
- Options:
  1. Accept. The spec and the page say that the scrub and the log mask are best effort, and the secrets of one server are unrelated random strings. Mention it in the ADR if the owner asks.
  2. Mask by merged match ranges in one pass. More code in `secrets.rs` and a new test.
- Recommendation: Option 1. The case needs two secrets that overlap by construction, and the Google client secret and refresh token do not.
- Decision: Option 1, closed by recommendation by the orchestrator in code round 1.
- Decided by: orchestrator (closed by recommendation)
- Date: 2026-11-14
- Status: closed by recommendation

