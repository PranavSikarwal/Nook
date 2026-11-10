# Node.js 24 upgrade: implementation plan

**Goal:** Run every CI, native gate and package job on Node.js 24 (LTS, codename Krypton) instead of Node.js 22 (maintenance, end of life 2027-04-30).

**Why 24 and not 26:** Node.js 26 became LTS on 2026-10-28, two weeks ago. Node.js 24 is in Maintenance LTS since 2026-10-20, with support until 2028-04. We pick it as the smallest step. `panel/package.json` already uses `@types/node` `^24`, so the types match. A later pull request can move to 26.

**Architecture:** No code change. Only the `node-version` input of `actions/setup-node` changes. The Panel dependencies stay as they are. `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` in `package.yml` sets the runtime of the GitHub actions themselves. It is separate from `node-version`, and it is now redundant, because runners use Node 24 for actions by default since 2026-06-16. It stays only to keep this pull request small. Remove it in a later pull request.

**Rollback:** revert the commit, or set `node-version` back to `22`. No lockfile or code change is involved.

## Global constraints

- Work on the branch `chore/node-24` (already checked out, from `origin/main`). Commit locally.
- Change only the files a task lists. Add no dependency. Do not change `panel/package.json` or `panel/package-lock.json`.
- Commit messages use `<type>: <subject>` or `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Do not start any command in the background. Run each check in the foreground with a time limit.
- Do not run a build or `npm ci`: CI is the test for this change.

## Tasks

### Task 1: Move the workflows to Node.js 24

**Files:** `.github/workflows/ci.yml` (2 places), `.github/workflows/native-e2e-gate.yml` (2 places), `.github/workflows/package.yml` (1 place).

- [x] Change `node-version: 22` to `node-version: 24` in all five places. Change nothing else in the workflow files.
- [x] Check: `grep -rn "node-version" .github` shows five lines, all with `24`. `python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/ci.yml .github/workflows/native-e2e-gate.yml .github/workflows/package.yml` exits with code 0.
- [x] Commit: `ci: run the workflows on Node.js 24`

### Task 2: Tick the plan

- [x] Tick every box above. Commit: `docs: tick the Node.js 24 plan`

## Verification (done by the reviewer, not by the implementer)

1. CI (`Code Quality and Tests`: `npm ci`, lint, `test:unit`, build) passes on Node 24.
2. Run the Native Tauri E2E Gate with `scripts/gh workflow run native-e2e-gate.yml --ref chore/node-24 -f pr_number=36`. A dispatch uses the workflow file of the ref it runs on, so `--ref chore/node-24` is required. The log says `node: v24.x` and the gate passes.
3. Run Package and Release with `scripts/gh workflow run package.yml --ref chore/node-24`. It passes on all three platforms and its log says `node: v24.x`.
4. The Node version line in each job log says `v24.x`.
