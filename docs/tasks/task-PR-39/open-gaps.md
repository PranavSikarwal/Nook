# Open gaps

## G1. Which MCP server package the docs name

- Stage: 2 spec
- Question: Which Google Docs MCP server does `docs/mcp-servers.md` name in its sample?
- Options:
  1. `@piotr-agier/google-drive-mcp`, pinned to `2.13.0`. MIT license, last release 2026-10-15, Node 18 or newer. It covers Drive, Docs, Sheets, Slides and Calendar (130 tools). The scopes can be narrowed with `GOOGLE_DRIVE_MCP_SCOPES`. The credentials path and the token path are settings. Checked on 2026-11-14: it starts and lists tools with no token.
  2. `@a-bonus/google-docs-mcp`, version `1.11.3`. Its tool list is closer to Docs, and `MCP_TOOL_GROUPS` may filter tools (the README lists it only under remote deployment, so stdio is not confirmed). Its scopes are fixed in code and include `gmail.modify`, `drive`, `calendar.events` and `script.projects`. They cannot be narrowed. The README says MIT and the npm record says ISC. Its README starts the server with `npx`, which `docs/mcp-servers.md` forbids.
  3. Name no package. The page lists the requirements and the settings only. The owner chooses the server.
- Recommendation: Option 1. It is the only checked option whose scopes the owner can narrow to one Docs scope. Option 2 asks Google for Gmail access that Nook does not need. Option 3 leaves the owner with no tested sample.
- Decision:
- Decided by:
- Date:
- Status: open

## G2. OAuth scopes in the sample

- Stage: 2 spec
- Question: Which scopes does the sample ask Google for? The owner pastes the same value into the consent screen and into `GOOGLE_DRIVE_MCP_SCOPES`.
- Options:
  1. `documents` only (`https://www.googleapis.com/auth/documents`). Read, edit and create Google Docs. Drive tools (list, search, share, delete) fail at Google. The owner gives the model the document address. Google's pages that were read do not say whether this scope is marked sensitive (see G3).
  2. `documents.readonly` (full URL, not a short alias of the server). The assistant can read Docs and cannot write. This does not meet "write to online documents" and is the safest start.
  3. `documents` and `drive.file`. Adds Drive access to files that Nook created or opened through this app. It lets the assistant create a new document in Drive. It adds a second scope to approve.
  4. The server default: eight scopes, including all of Drive and Calendar. Nook does not need this.
- Recommendation: Option 1. It matches the goal (write to Docs) with the smallest scope. The page shows option 2 as a variant for a read-only start.
- Decision:
- Decided by:
- Date:
- Status: open

## G3. Consent screen audience and token life

- Stage: 2 spec
- Question: Which audience and publishing status does the owner choose for the OAuth client? It decides how long the sign-in lasts.
- Options:
  1. **External, Testing status.** Works for any Google account. Google expires the refresh token 7 days after consent for an app in Testing, so the owner signs in again every week. No Google review. The owner must list their own account as a test user.
  2. **Internal.** Only for a Google Workspace account, only for users of that organization. Google's pages that were read do not state a 7-day limit for it. Not available for a personal `@gmail.com` account.
  3. **External, In production.** The owner publishes the app. Google shows an "unverified app" warning at sign-in if the scope is sensitive, and caps unverified apps at 100 users over the life of the project. Google's pages that were read do not state the refresh-token expiry for this case, so the page cannot promise a lifetime.
- Recommendation: Option 1 first. It works for every account and the owner can publish later without a code change. Write the weekly sign-in into the page, and give the switch to option 3 as the way out. The owner may choose option 2 when the account is a Workspace account.
- Decision:
- Decided by:
- Date:
- Status: open

## G4. Where the client secret lives

- Stage: 2 spec
- Question: Where does the OAuth client secret go on the owner's machine?
- Options:
  1. In the downloaded JSON file in a private folder (`chmod 700`, file `chmod 600`) that is in `write_paths`. The sample `env` holds only the path (`GOOGLE_DRIVE_OAUTH_CREDENTIALS`).
  2. In `env` of `config.toml` as `GOOGLE_DRIVE_MCP_CLIENT_SECRET`. The server needs a refresh token in `env` for this mode, and `config.toml` stores `env` as plain text.
- Recommendation: Option 1. The secret stays out of `config.toml`, which the owner may share while asking for help. The repository holds no secret in either option.
- Decision:
- Decided by:
- Date:
- Status: open

## G5. The 64-tool cap and the 130 tools of the server

- Stage: 2 spec
- Question: Nook offers the first 64 tools of a server. The recommended server lists 130, and no setting of that server filters them. How does the sample deal with the extra Drive and Sheets tools?
- Options:
  1. Accept it. The narrow scope blocks the Drive calls at Google, every call asks first, and the page names the effect (context cost, a few failed calls).
  2. Add a per-server tool allowlist to Nook (`tools = [...]` in the server block). This is a code change in `config.rs`, `mcp.rs` and the docs. It is a separate task with its own spec, and it needs a decision on its own gate 1.
  3. Use a server that can filter its tool list (option 2 of G1). It gives up the narrow scope.
- Recommendation: Option 1 for this task. Record option 2 as a follow-up task if the extra tools cost too much in use.
- Decision:
- Decided by:
- Date:
- Status: open

## G6. Who checks the live path, and when

- Stage: 2 spec
- Question: CI cannot sign in to Google. The owner creates the OAuth client anyway. Does the owner follow the written steps once on the branch, and report the result, before merge?
- Options:
  1. The owner follows the steps before gate 2 and tells the orchestrator what failed. The orchestrator fixes the page in a round. The docs then say that the steps were run on a named date.
  2. The owner follows the steps after the merge. The docs say that the live path is untested until then.
  3. Nobody checks. The docs say that the live path is untested.
- Recommendation: Option 1. The owner does the work once, and the page is true when it merges. If the owner cannot do it before gate 2, use option 2 and keep the "untested" sentence.
- Decision:
- Decided by:
- Date:
- Status: open
