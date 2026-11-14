```
Stage: 3 spec review
Gate 1: pending
Gate 2: pending
```

# Name the folder rule when a call is refused

Item 11 of `docs/plans/open-items-fixes.md`. Pull request: #41.

## Why

A tool call outside the allowed folders ends with the notice "Request denied. I
stopped here." The user denied nothing. The notice misleads the user, and it
does not say how to fix the problem.

Cause, in the Worker (`worker/src/nook_worker/`):

1. The Daemon refuses the call with a fixed reason, for example "Path is outside
   the allowed folders" (`OUTSIDE_ROOTS` in
   `daemon/crates/nook-core/src/providers/roots.rs`). The file tools run in the
   Daemon, so the reason arrives as the output of a tool result with the status
   `denied`. A tool that runs in the Worker gets it as `AuthorizationDecision.reason`.
2. `AuthorizationManager.authorize` and `AuthorizationManager.execute`
   (`authorization.py`) call `mark_denied(request_id)`. `_denied` is a set of
   request ids, so the reason is lost.
3. `RealAgentRunner.run` (`agent.py`) adds the constant `DENIAL_NOTICE` ("Request
   denied. I stopped here.") for every denied request, whatever the reason.

The text for a real Deny click is correct. The text is wrong for every refusal
that no user decision caused: a path outside the allowed folders, a write to a
read-only folder, a path in the wrong form, a policy denial and a timeout. This
task fixes the two folder rules, because the user can change them in Settings,
Folders. Gap G4 records why the other refusals stay as they are.

## Scope

In scope:

1. The Worker keeps the reason of the first denial of a request.
2. When that reason is a folder rule, the reply ends with a fixed message that
   names the rule and says how to allow the path.
3. Every other denial keeps the text "Request denied. I stopped here."
4. Tests, the central docs and one changelog line under "Unreleased", in the
   category "Fixed".

Out of scope:

1. The Daemon, `contracts/`, `policy/` and the Panel do not change.
2. A denial still ends the reply (see G3).
3. Policy denials, timeouts, a path that is not absolute or not valid, a write
   over the size limit, and the argument errors of the command tool keep their
   current text (gap G4).
4. `docs/plans/open-items-fixes.md` and `CHECKLIST.md` belong to the
   housekeeping task (PR #42, branch `chore/plans-housekeeping`). This task does
   not edit them. PR #42 does not change the row of item 11 in
   `docs/plans/open-items-fixes.md`, so the row is stale after this merge. Gap
   G5 says who changes it.

## Design

One table in `agent.py` maps a Daemon reason to its notice. It is the only
place that holds the folder messages. The notices below are the recommendation
of gap G1. The owner decides the final text at gate 1.

| Daemon reason | Notice |
| --- | --- |
| `Path is outside the allowed folders` | `I stopped here. That path is outside your allowed folders. To allow it, add its folder in Settings, Folders, and ask again.` |
| `The folder is read-only` | `I stopped here. That folder is read-only. To let me write there, change its access in Settings, Folders, and ask again.` |

Changes, all in `worker/`:

1. `authorization.py`: `_denied` becomes a dict from request id to the first
   reason (`str | None`). `mark_denied(request_id, reason=None)` keeps the
   first reason and ignores later ones. The default keeps the existing callers
   valid. A new `denial_reason(request_id)` returns it.
   `authorize` passes `decision.reason`. `execute` passes the output of a
   `denied` outcome. `clear_context` still forgets the request.
2. `agent.py`: a function `_denial_notice(reason)` returns the table entry for
   the reason, or `DENIAL_NOTICE` when the reason has no entry. `run` calls it
   with `denial_reason(request.request_id)`. The function must read the reason
   before `cancel_for_request` and before `clear_context`.
3. The Worker matches the reason text that the Daemon sends. This is gap G2.
   An unknown reason falls back to `DENIAL_NOTICE`, so a changed Daemon text
   never hides a denial. A Worker test reads the two texts from the Daemon
   source and checks them against the table keys, so a changed Daemon text
   fails a test.

Tests (written first, in `worker/tests/test_denial_ends_reply.py`), one behavior
each. Tests 1 to 5 drive `RealAgentRunner.run` through the real
`build_remote_tool` and `AuthorizationManager.resolve_tool_result`, as the Daemon
answers a Daemon-run tool. They do not call `mark_denied` directly.

1. A request denied with the outside-folders reason ends with the folder
   notice, and not with `DENIAL_NOTICE`.
2. A request denied with the read-only reason ends with the read-only notice.
3. A request denied with `Denied by the user` still ends with `DENIAL_NOTICE`.
4. A request denied with an unknown reason ends with `DENIAL_NOTICE`.
5. The first reason wins when two calls are denied in one request.
6. `AuthorizationManager.execute` keeps the reason of a `denied` outcome.
7. `AuthorizationManager.authorize` keeps the reason of a deny decision.
8. The keys of the notice table equal `OUTSIDE_ROOTS`
   (`daemon/crates/nook-core/src/providers/roots.rs`) and `READ_ONLY`
   (`daemon/crates/nook-core/src/providers/filesystem.rs`), read from the Daemon
   source.

## Central docs that change

1. `docs/tools-and-authorization.md`, section "Denial ends the reply" (about
   line 324): the notice depends on the reason.
2. `docs/tools-and-authorization.md`, the paragraph on refusals before any
   prompt (about line 119): it says which refusals end with a folder notice.
3. `docs/contracts.md`, the list of fixed reasons (about line 89): it adds the
   two folder reasons and says that the Worker reads them.
4. `docs/contracts.md`, Worker step 4 of the Daemon-run tool flow (about line
   160): "the reply ends as it does for a web tool" now names the notice.

`docs/architecture.md`, `docs/mcp-servers.md`, `docs/release.md` and
`docs/testing.md` do not change. This is not a release: no version bump.

## Rule changes

None.

## Risks

1. The Worker and the Daemon both hold the reason text. `docs/contracts.md`
   lists only seven other reasons today, and the schema types `reason` as a free
   string. The Daemon tests pin each text (`folders_tests.rs`,
   `filesystem_tool_tests.rs`), but they move together with a Daemon edit. Test 8
   ties the Worker keys to the Daemon constants, so a changed Daemon text fails a
   Worker test. The test reads Daemon source files, so it works only in a full
   checkout. Gap G2 offers a contract field that removes the duplicate.
2. With two denied calls in one request, the notice names the first reason.
3. A folder-rule denial still stops the model. The model cannot try another
   path in the same reply (gap G3).
5. A policy denial after an Allow click (a credential path, reason `Policy does
   not allow this request`) still ends with "Request denied". This is outside
   item 11.
4. `scripts/package-approval-check.py` and `panel/e2e/specs/live-fetch.spec.mjs`
   check the text of a real Deny. That text does not change.
