# Tool platform open gaps

This file lists what is not done after Stage 2 (TP7 to TP13) on PR #32. Each
gap names the risk and who decides. Update it when a gap closes.

Recorded on October 17, 2026.

## Needs a decision from the owner

1. **TP11 designs.** `docs/design/browser-automation.md` and
   `docs/design/desktop-app-tools.md` wait for approval. Each ends with open
   questions. The owner removed the clipboard, notification, and open-link tools on
   October 17, 2026, because the user can do each without the assistant. The
   desktop design is now one Panel-run Notes tool. Risk: no browser
   or desktop-app tool can start until the designs are approved or changed.
2. **Image search.** The owner chose to leave it out on October 17, 2026. It
   was removed because the DuckDuckGo image backend answers with a bot challenge
   (HTTP 202) and `ddgs` 9.16.0 cannot pass it. The `auto`, `bing`, `brave`,
   `google`, `yahoo` and `wikipedia` backends of `ddgs` returned images on the
   same day with no key. Risk: the model cannot find images. To bring the tool
   back, restore it from commit `ff7f57c`, pick one backend, and change the docs
   that say a search asks only DuckDuckGo.
3. **macOS sandbox successor.** Seatbelt is deprecated and Apple has published no
   replacement (apple/containerization#737, open). Risk: if a later macOS removes
   `sandbox-exec`, the start-up self-test fails and terminal and MCP tools turn
   off on macOS. They never run unsandboxed. Options: wait for Apple, or build a
   Linux virtual machine backend that runs Linux commands only. ADR 0007
   records the finding.
4. **MCP restart gap.** The Daemon starts a stopped MCP server on the next call
   and waits at least 5 seconds between two starts. Risk: calls in that window
   fail. The owner can ask for another gap.

## Work not done

0. **Notes and Google Docs.** `docs/plans/app-integrations.md` holds the trial
   result: a Notes MCP server cannot run in the macOS sandbox, because macOS
   refuses Apple events from sandboxed processes. The owner must pick a route.

5. **TP6 on Windows.** Closed for macOS and Ubuntu on October 17, 2026 (package
   workflow run 39016561692). The approval check uses a Unix socket, so Windows
   keeps the file and policy check only. Risk: a fault in the approval path that
   only the Windows build shows would reach a release.
6. **Live checks.** The live native test passed on October 17, 2026 for the
   news query and the web fetch flow, on the development build at `43d2cdb`. The
   installed `v0.1.4` app and the Stage 2 tools (file, terminal, MCP, news,
   video, book) were not run against the live model. Risk: a fault that only the live model shows stays
   hidden. The Hugging Face hang does not reproduce on the current code: the
   fetch takes about 1 second. The installed `v0.1.4` app has older code.
7. **News, video and book search.** They were checked with mocks, and news and
   video also with a live call on October 17, 2026. DuckDuckGo throttled them
   earlier. Risk: results can be empty when DuckDuckGo throttles.
8. **Windows has no sandbox.** Terminal and MCP tools are off on Windows. File
   tools work. Risk: Windows users have fewer tools.
9. **Linux container run.** The Docker disk on the development Mac is full, so
   `scripts/test-linux.sh` did not run for the last features. CI ran the Linux
   tests with `NOOK_REQUIRE_SANDBOX` set, and they passed.
10. **No local Windows build check.** `cargo check --target
    x86_64-pc-windows-msvc` fails on the development Mac because the C build of
    `ring` has no Windows headers. The Windows CI job covers it.
11. **URL rules.** URLs with a username and non-decimal IP forms are rejected.
    Risk: a rare valid URL is refused.

## Clean-up for the owner

12. **Stale Daemon.** Closed on October 17, 2026. The owner stopped process `51038`.
13. **Test rows in the `nook` database.** The owner chose to ignore them on
    October 17, 2026. Thread
    `59103bac-6fc6-4425-bf97-673264f2ad8f` sits in `worker.checkpoint_writes`,
    `worker.checkpoint_blobs` and `worker.checkpoints`. It is not a chat. Delete
    the rows with that `thread_id` from the three tables.
14. **Merge order.** PR #31 must merge before PR #32, because #32 contains its
    commits.
