# Nook

Nook is a macOS overlay app. It has a SwiftUI panel, a Rust daemon, and a Python Deep Agents worker. The worker talks to a self-hosted OpenAI-compatible model.

## Where things are

- `docs/spec/` holds the v1 specs, starting at `00-overview.md`.
- `docs/plan.md` holds the tasks, and `CHECKLIST.md` tracks them.
- `docs/plans/` holds implementation and execution plans for all work.
- `docs/adr/` holds the decisions that are hard to reverse.
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
