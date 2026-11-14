```
Stage: 3 spec review
Gate 1: pending
Gate 2: pending
```

# Google Docs through an MCP server

## Why

The owner wants the assistant to read and write Google Docs.
`docs/plans/app-integrations.md` already decides the route: a Google Docs MCP
server (a program that offers tools over standard input and output) attached
with the existing `[[tools.mcp_servers]]` setting. That route needs no new
Nook tool code. Two things are missing:

1. No page tells the owner which server to attach, which settings to use, and
   how to create the Google sign-in app (the OAuth client).
2. No test proves that a Docs-style tool is listed, authorized and called
   through the MCP path with the settings that a Docs server needs
   (`network = true`, a token folder, a credentials file).

A real Google Docs server cannot run in CI, because it needs a Google account
and the network. A fake MCP server stands in for it.

## Scope

In scope:

1. A sample `config.toml` block and the owner's setup steps, in
   `docs/mcp-servers.md`.
2. A `docs` mode in the fake MCP server, and tests in
   `daemon/crates/nook-core/tests/mcp_tool_tests.rs`.
3. A test that the sample block in `docs/mcp-servers.md` loads as a valid
   server with the real config loader, so the sample cannot drift.
4. One paragraph in `docs/testing.md` and one line in `CHANGELOG.md`.

Out of scope:

1. New Nook tool code, a new setting, or a change to `contracts/` or `policy/`.
2. A per-server tool filter in Nook (see gap G5).
3. A live call to Google in CI.
4. Notes, Windows support, and every other part of `app-integrations.md`.
5. `CHECKLIST.md` and `docs/plans/`. The housekeeping task owns them. The
   status line of `docs/plans/app-integrations.md` stays as it is.

## Design

### The server

The spec names `@piotr-agier/google-drive-mcp`, pinned to version `2.13.0`
(gap G1). It is an MIT-licensed server for Google Drive, Docs, Sheets, Slides
and Calendar. The orchestrator checked it on 2026-11-14:

1. `npm install --prefix <folder> @piotr-agier/google-drive-mcp@2.13.0`
   installs it. Its entry file is `dist/index.js`, and it needs Node 18 or newer.
2. Started with no token and no credentials, in stdio mode, it answers
   `initialize` and `tools/list`. It lists 130 tools. It opens no browser at
   that point.
3. `GOOGLE_DRIVE_MCP_SCOPES` narrows the scopes that it asks Google for.
   Without it, the server asks for eight scopes, including all of Drive and
   Calendar.
4. `GOOGLE_DRIVE_OAUTH_CREDENTIALS` is the path of the OAuth client file.
   `GOOGLE_DRIVE_MCP_TOKEN_PATH` is the path of the token file.
5. The one-time sign-in is the `auth` command of the same program. It opens a
   browser, so it runs outside Nook.

The sign-in, a token refresh and a real Docs call were not run. Nothing in CI
runs them. The docs say so, and the owner's live check (gap G6) closes that.

### Sample block

The page gives this block, with the paths of a macOS user. The values that the
owner must change are named in the text around it.

```toml
[[tools.mcp_servers]]
name = "google-docs"
command = "/usr/local/bin/node"
args = ["/Users/me/tools/google-docs-mcp/node_modules/@piotr-agier/google-drive-mcp/dist/index.js"]
env = { GOOGLE_DRIVE_OAUTH_CREDENTIALS = "/Users/me/.config/nook-google-docs/gcp-oauth.keys.json", GOOGLE_DRIVE_MCP_TOKEN_PATH = "/Users/me/.config/nook-google-docs/tokens.json", GOOGLE_DRIVE_MCP_SCOPES = "documents" }
network = true
read_paths = ["/Users/me/tools/google-docs-mcp"]
write_paths = ["/Users/me/.config/nook-google-docs"]
timeout_seconds = 120
```

Why each value:

1. `command` is the absolute path of `node`, and `args` holds the script. The
   sandbox `PATH` does not hold a version manager, so a script with
   `#!/usr/bin/env node` can fail. If `node` lies outside the system folders
   (for example under `nvm`), its folder goes into `read_paths`.
2. `network = true` is required, because the server calls Google. Nook has no
   per-host network rule, so the server can reach any host. The page says so.
3. `write_paths` holds one folder with the credentials file and the token file.
   The server rewrites the token file when it refreshes the token.
4. `env` holds paths and scope names, never a secret. The client secret lives in
   the credentials file, in a folder that only the owner can read.
5. `timeout_seconds = 120`, because a Google call can retry (the server retries
   up to 3 times).

### What the model sees

The model sees the tools as `mcp__google-docs__<tool>`, for example
`mcp__google-docs__readGoogleDoc`. Each call asks the owner first. The
approval card names the server, the tool and the first 300 characters of the
arguments. The page tells the owner to use **Always allow this tool** only for
tools that read.

The server lists 130 tools, and Nook offers the first 64 (gap G5). In
version 2.13.0, these 64 hold most of the Docs tools and many Drive tools.
With the scope `documents` only, a Drive tool fails at Google, and the failure
returns to the model as an error result. The page lists which tools work with
the scope and which do not, from the checked server list.

### The steps for the owner

The page gives these steps. The Google Cloud Console changes its menus, so the
page names the date that it was checked and says what to look for when a name
differs. The owner does this once. The owner pastes no secret into chat, a
commit or a pull request.

1. Open `https://console.cloud.google.com` and sign in with the Google account
   that owns the documents.
2. Open the project picker, choose **New project**, name it `Nook Google Docs`,
   and choose **Create**.
3. Open **APIs & Services**, then **Library**. Search for **Google Docs API**
   and choose **Enable**. If gap G2 adds Drive scopes, also enable **Google
   Drive API**.
4. Open **Google Auth Platform** (or **APIs & Services**, then **OAuth consent
   screen**). Choose **Get started**. Set the app name `Nook`, a support email
   and a contact email. Choose the audience that gap G3 decides.
5. Open **Data Access**. Choose **Add or remove scopes** and add the scope that
   gap G2 decides (`https://www.googleapis.com/auth/documents` in the
   recommendation). Save.
6. If the audience is External and in Testing status, open **Audience**, then
   **Test users**, then **Add users**, and add the Google account of step 1.
7. Open **Clients**. Choose **Create client**, application type **Desktop app**,
   name `Nook`, and **Create**. Choose **Download JSON**.
8. Make the folder `~/.config/nook-google-docs`, keep it private
   (`chmod 700`), and move the downloaded file there as `gcp-oauth.keys.json`
   (`chmod 600`). This file holds the client secret. Keep it out of the
   repository and out of `config.toml`.
9. Install the server in a folder you control:
   `npm install --prefix ~/tools/google-docs-mcp @piotr-agier/google-drive-mcp@2.13.0`.
10. Sign in once, in a terminal outside Nook, with the same three environment
    values that the sample block holds, then run the server with `auth`:
    `node <install folder>/node_modules/@piotr-agier/google-drive-mcp/dist/index.js auth`.
    The browser opens. Approve the access. The token file appears in the folder
    of step 8.
11. Add the block to `config.toml`, restart Nook, and look for
    `MCP server is ready` in `daemon.log`.
12. Ask the model to read a document. Give it the document address. Approve the
    call on the card.

The page adds one section, "When Google Docs stops working", with these causes:

1. The token expired. An External app in Testing status expires its refresh
   token after 7 days, so run step 10 again.
2. The scope changed. Changing `GOOGLE_DRIVE_MCP_SCOPES` needs a new sign-in
   (step 10).
3. The token file is missing. Without a token, a call fails, because the sandbox
   has no browser. Run step 10.
4. The call timed out. Raise `timeout_seconds`, up to 600.

### The fake server and the tests

The fake server gets a `docs` mode. In this mode it lists only Docs-style tools,
named like the real server: `readGoogleDoc` (arguments `documentId`) and
`insertText` (arguments `documentId`, `text`, `index`), and a Drive-style tool
`listFolder` that returns an error result, as a Drive call does when the scope
is missing. The `normal` mode stays as it is, so the existing exact-list test
does not change.

New tests in `mcp_tool_tests.rs`. Each test checks one behavior, and each uses
a server named `google-docs`, built like the sample block (with
`network = true` and a temporary token folder in `write_paths`):

1. `a_docs_style_tool_is_listed_under_the_server_name`: the listed names are
   `mcp__google-docs__readGoogleDoc`, `mcp__google-docs__insertText` and
   `mcp__google-docs__listFolder`. The description ends with
   `(MCP server: google-docs)`.
2. `a_docs_style_call_asks_first_and_shows_the_document_id`: the approval card
   shows the server, the tool and the arguments, with the four actions.
3. `an_approved_docs_style_call_returns_the_document_text`: after **Allow
   once**, the result is the text of the fake document.
4. `a_denied_docs_write_never_reaches_the_server`: after **Deny** on
   `insertText`, the fake server records no call.
5. `a_docs_style_scope_error_comes_back_as_an_error_result`: the `listFolder`
   error returns with `status` `error`.

One more test in `daemon/crates/nook-core/src/config.rs` or in a test file
that reads `docs/mcp-servers.md` (the plan names the place):
`the_google_docs_sample_block_loads_as_a_valid_server`. It takes the first
`toml` block under the heading "Example: Google Docs", loads it with the real
config loader, and checks the name, `network`, the three `env` keys, and that
every path is absolute.

The tests use the real policy files, the real sandbox and the fake Worker, as
the suite does. They skip with a message when the sandbox is missing, as the
other MCP tests do.

## Central docs that change

1. `docs/mcp-servers.md`: the section "Example: Google Docs" (sample block, the
   owner's steps, the limits) and the section "When Google Docs stops working".
2. `docs/testing.md`: one paragraph on the Docs-style MCP tests and the sample
   test.
3. `CHANGELOG.md`: one line under "Unreleased".

`docs/architecture.md`, `docs/contracts.md`, `docs/tools-and-authorization.md`
and `docs/release.md` do not change, because no code, message, policy or
release step changes. This task is not a release, so it changes no version.

## Rule changes

None. The task changes no rule and no binding document other than the
documents above.

## Risks

1. **The live path is untested.** The sign-in, the token refresh and a real Docs
   call do not run in CI. The docs say so. The owner's live check (G6)
   reduces the risk. The sample can be wrong in a detail that only a live run
   shows, for example a path the sandbox does not allow.
2. **A broad network.** `network = true` lets the server reach any host. A
   server with the network and a readable token can send the token out. The
   page tells the owner to pin the version, to use the narrow scope, and to
   keep the token folder small.
3. **The 64-tool cap.** Nook offers the first 64 of 130 tools. The model gets
   Drive tools that the narrow scope blocks. This costs context and a few
   failed calls. It does not cost safety, because each call asks first and the
   scope limits Google's answer.
4. **A moving package.** The server is a third-party program. The pin
   `2.13.0` and the "checked on" date are the guard. A newer version needs a
   new check.
5. **Google changes.** The Cloud Console menus and the rules for unverified apps
   change. The page names the date of the check and links the Google pages.
6. **Two licenses for one package family.** The `@a-bonus/google-docs-mcp`
   README says MIT and its npm record says ISC. This matters only if the owner
   picks that server (G1).
7. **Prompt injection.** A document can hold text that tries to steer the
   model. The page says that document text is untrusted, as it says for every
   tool result. The approval card still shows each write.
