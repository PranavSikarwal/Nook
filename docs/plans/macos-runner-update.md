# Move the macOS CI jobs to macos-15: implementation plan

**Goal:** The jobs that need a macOS runner start again. GitHub retired the `macos-14` image on 2026-11-02, so every `macos-14` job is cancelled with "The configuration 'macos-14-arm64-us-default' is not supported".

**Architecture:** No code change. Only the `runs-on` label of three jobs changes. `macos-15` is also Apple Silicon (arm64), so the build output stays the same architecture.

**Spec:** `.github/workflows/native-e2e-gate.yml` (lines 72 and 133) and `.github/workflows/package.yml` (line 17).

## Global constraints

- Work on the branch `feat/tp7-provider-and-sandbox` (already checked out). Commit locally. Do not push.
- Change only the files this plan lists. Do not run any build or test. A workflow file cannot run locally.
- Commit messages use `<type>(<scope>): <subject>` and contain no co-author or generator line.
- Prefix shell commands with `rtk`.
- Time box: 15 minutes.

## Tasks

### Task 1: Change the runner label

**Files:** `.github/workflows/native-e2e-gate.yml`, `.github/workflows/package.yml`, this file.

- [x] In `native-e2e-gate.yml`, change both `runs-on: macos-14` to `runs-on: macos-15`.
- [x] In `package.yml`, change `os: macos-14` to `os: macos-15`.
- [x] Run `rtk grep -rn "macos-14" .github docs` and make sure that no hit remains.
- [x] Validate the YAML: `python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/native-e2e-gate.yml .github/workflows/package.yml`.
- [x] Tick the boxes of this task. Commit all three files together: `ci: move the macOS jobs to macos-15`
