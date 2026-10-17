# Nook

Nook is a cross-platform desktop overlay app. It has a Tauri Panel, a Rust Daemon, and a Python Deep Agents Worker. The Worker talks to a self-hosted OpenAI-compatible model.

## Where things are

- `docs/architecture.md`, `docs/contracts.md`, `docs/tools-and-authorization.md`, `docs/release.md`, and `docs/testing.md` describe current behavior.
- `docs/plan.md` holds the active work, and `CHECKLIST.md` tracks it.
- `docs/plans/` holds active implementation plans. `docs/archive/plans/` holds completed plans.
- `docs/design/` holds proposed designs that wait for approval.
- `docs/adr/` holds decisions that are hard to reverse. `docs/archive/` holds retired specifications and historical records.
- `model-check/` holds the script that tests the model endpoint with Deep Agents.

## Agent skills

### Issue tracker

Issues and specs live as local markdown under `.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. See `docs/agents/domain.md`.

## Planning rules

Always write plans inside the `docs/plans/` directory for all work. Every new phase, feature, or refactoring task must have a plan file recorded there before execution starts.

## GitHub CLI and commit rules

This repository uses the personal GitHub profile `PranavSikarwal`.

1. Never run `gh auth switch`. It modifies global state in `~/.config/gh/hosts.yml` and disrupts other workspaces.
2. Run all GitHub CLI commands through `scripts/gh`:
   ```sh
   scripts/gh <subcommand>
   ```
   Do not run `gh` directly and do not run `gh auth token`. The wrapper manages authentication automatically.
3. Commits in this repository must use local repository identity rather than the global work identity.
4. Git pushes authenticate using the repository-local credential helper that reads `gh auth token --user PranavSikarwal`.
