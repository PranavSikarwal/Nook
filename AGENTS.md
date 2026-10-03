# Nook

Nook is a macOS overlay app. It has a SwiftUI panel, a Rust daemon, and a Python Deep Agents worker. The worker talks to a self-hosted OpenAI-compatible model.

## Where things are

- `docs/spec/` holds the v1 specs, starting at `00-overview.md`.
- `docs/plan.md` holds the tasks, and `CHECKLIST.md` tracks them.
- `docs/adr/` holds the decisions that are hard to reverse.
- `model-check/` holds the script that tests the model endpoint with Deep Agents.

## Agent skills

### Issue tracker

Issues and specs live as local markdown under `.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context. See `docs/agents/domain.md`.
