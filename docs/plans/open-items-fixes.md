# Plan: fixes for the open items

Status: proposed. Items 1 to 5 need no decision. Items 6 to 9 wait for the owner.

Written on October 17, 2026, after the Stage 2 work on PR #32. Each item names
the evidence, the cause, the fix, the test that fails first, and the check.

## Evidence gathered

1. **Package approval check failed on CI.** The Ubuntu and macOS jobs failed with
   `FileNotFoundError: panel/src-tauri/resources/daemon/nookd`. The workflow passes
   a relative path, and the script started the Daemon from another folder. The
   script now resolves the path (commit `b650995`). A run with a relative path
   passes on the development Mac. Run `39016280603` must confirm it on CI.
2. **Links in replies.** `MarkdownRenderer.tsx` has no `a` component, so
   `react-markdown` renders a plain link. The Tauri capability file has no
   permission to open a link, although `tauri-plugin-shell` is loaded. What a
   click does is not tested. The click may navigate the Panel window away from
   the app or may do nothing.
3. **Settings rewrite `config.toml`.** `Config::save_to_file` serializes the whole
   struct. Every save from Settings drops the comments and the layout of a
   hand-edited file. It writes the file with the default mode, which is
   readable by other users on a shared machine. The `env` values of an MCP server
   sit in that file.
4. **MCP status is invisible in the app.** The Panel keeps the Daemon's standard
   error only in test runs (`NOOK_E2E_RUN_DIR`). In a normal run the reason that
   a server is off goes nowhere. The Panel also has no message that lists the
   servers.
5. **Hugging Face hang.** The current Worker fetches
   `huggingface.co/docs/transformers/index` in 0.8 seconds and
   `huggingface.co/papers` in 1.2 seconds. The live native test fetches the same
   pages and passes. The hang does not reproduce on the current code. The
   installed `v0.1.4` app has older code without the fetch deadline.

## Fixes

### 1. Close the package check (TP6)

1. Wait for run `39016280603`. It must pass on macOS and Ubuntu.
2. If a step fails, read that job's log, find the cause, and fix it. Do not retry
   without a cause.
3. Tick TP6 in `CHECKLIST.md`, and update `docs/tools-and-authorization.md` line
   about TP6 and the gaps file.

### 2. Keep `config.toml` intact on save

Cause: the writer re-serializes the whole file.

1. Write a Rust test first. A file with comments, an `[[tools.mcp_servers]]`
   block with `env`, and a changed model must keep the comments and the block
   after the update. It fails today.
2. Add `toml_edit` to `nook-core`. Add `Config::update_model_settings(path,
   base_url, model)` that edits those two keys in place and creates the file if
   it is missing.
3. Create the file with mode `0600` on Unix. Test the mode on macOS and Linux.
   Windows keeps the default, and the gap is written in the guide.
4. Use the new function in the Settings handler. Remove `save_to_file` if nothing
   else uses it.
5. Add one sentence to `docs/mcp-servers.md` about the file mode.

Check: `cargo test`, `clippy -D warnings`, and the `history-settings` native spec,
which saves settings.

### 3. Make links in replies open in the browser

Cause (suspected, not yet shown): no link handler and no open permission.

1. Reproduce first. Add a native test that sends a reply with a Markdown link,
   clicks it, and asserts that the Panel window still shows the chat. If the
   window navigates away, the test fails and the bug is confirmed. If it does
   not fail, record the result and stop.
2. Add an `a` component in `MarkdownRenderer.tsx`. It accepts only `http` and
   `https` links, shows no `javascript:` or `file:` link as a link, and calls the
   Tauri open command with the address.
3. Add only the permission for opening links, limited to `http` and `https`, to the
   capability file.
4. The native test cannot see the system browser. It checks that the command is
   called with the right address, using a stub of the command in the E2E build.

Check: Panel lint, unit tests, build, and the new native test.

### 4. Show why an MCP server is off

Cause: nothing stores the Daemon log and nothing reports server state.

Part A, no contract change:

1. Write the Daemon log to `daemon.log` in the application folder, with mode
   `0600`, and keep the last two files of 1 MiB each.
2. Redact the log first. sqlx errors can contain parts of `database_url`, and the
   log holds the model and base address. Test the redaction with a URL that has
   a password and an `env` value.
3. Document the file in `docs/mcp-servers.md` instead of "quit Nook and run
   `nookd`".

Part B, needs the owner's yes because it changes the Panel contract:

1. Add a Daemon message `tools_status` with each MCP server, its state (`ready`,
   `off`, or `stopped`), the number of tools, and a short reason.
2. Show it in Settings, in a Tools tab.
3. Update the schemas, the examples, the contract tests, and the native tests.

### 5. Close the Hugging Face hang

1. Install the build of PR #32.
2. Run the manual live checklist below. It includes the Hugging Face fetch.
3. If it does not hang, record "not reproducible on the current code" and close
   the item. If it hangs, capture `daemon.log` (item 4) and start debugging from
   the first evidence.

## Manual live checklist

Run it on the installed build with your model. Add the config blocks first.

1. A news question. A video question. A book question.
2. A fetch of `https://huggingface.co/docs/transformers/index`: deny, allow once,
   and a chat and host grant.
3. With `[tools.filesystem]`: read, list, and write a file in the listed folder,
   and a write over an existing file.
4. With `[tools.terminal]`: `ls`, then a command that needs the network.
5. With one MCP server from `docs/mcp-servers.md`: list its tools, call one, then
   kill the server process and call again.
6. Settings, Permissions: revoke a grant and set a tool to Never allow.

## Decisions for the owner

6. **Notes route.** Options are in `app-integrations.md`. I recommend a Panel-run
   Notes tool. It needs its own design and ADR 0007 gets a note, because the
   Panel executes it.
7. **Native-system design.** I recommend deleting the clipboard, notification,
   and open-link design. If you choose a Panel-run Notes tool, rewrite the file
   for it instead.
8. **Secrets in `env`.** Keeping them in the Keychain needs a new setting form,
   such as `env_keychain = { NAME = "account" }`. I recommend waiting. Item 2 makes
   the file private, which covers the common risk.
9. **Image search.** It stays out. To bring it back, restore it from commit
   `ff7f57c` with one named backend.

## Not planned

- A local Windows build check. It needs a Windows toolchain on the Mac. CI covers it.
- A local Linux container run. The Docker disk is full. CI covers it.
- Computer use. It needs its own design.

## Order

1, 2, 3, 4A, 5. Then 4B, 6, 7, 8 as the owner decides. One commit for each item.
