# Plan: Manual pull request reviewers with hosted model

This plan sets up on-demand pull request reviewers powered by your hosted OpenAI-compatible model endpoint.

## Objectives

1. Support on-demand triggers: slash commands (`/code-review`, `/code-review-expert`, `/feedback-on-resolution`) and right-panel labels (`code-review`, `code-review-expert`, `feedback-on-resolution`).
2. Support review modifiers (`max`, `high`, `low`) and freeform focus instructions passed in the trigger comment.
3. Run against a hosted OpenAI-compatible model using repository secrets.
4. Keep workflows completely idle during normal push and PR open events.
5. Execute the specific checks from `code-review` and `code-review-expert`, plus a resolution verification flow.

## Architecture

```
GitHub Event (Comment or Label)
       |
       v
GitHub Actions (.github/workflows/reviewers.yml)
       |
       +--> Checkout repository (fetch-depth: 0)
       |
       +--> Setup uv and Python 3.13
       |
       +--> Execute scripts/pr_reviewer.py
               |
               +--> Read PR diff and git context
               +--> Load skill prompt (.github/skills/)
               +--> Query hosted OpenAI-compatible model
               +--> Post review using GitHub CLI (gh)
```

## Secrets required

| Secret name | Purpose | Example value |
|---|---|---|
| `REVIEWER_BASE_URL` | Base URL of your hosted OpenAI-compatible endpoint | `https://api.yourhost.com/v1` |
| `REVIEWER_API_KEY` | API key for authentication | `sk-...` |
| `REVIEWER_MODEL` | Target model name | `qwen-2.5-coder-32b` |

## File layout

```
.github/
  skills/
    code-review.md
    code-review-expert.md
  workflows/
    reviewers.yml
scripts/
  pr_reviewer.py
```

## Execution flow

1. A developer posts a comment (`/code-review`, `/code-review-expert`, `/feedback-on-resolution`) or applies a label on the pull request.
2. The GitHub Action passes the comment text in `COMMENT_BODY` and extracts the pull request number.
3. `scripts/pr_reviewer.py` parses the modifier level (`max`, `high`, or `low`) and any freeform focus instructions.
4. The runner checks out the repository with full git history (`fetch-depth: 0`).
5. `scripts/pr_reviewer.py` runs with the selected mode:
   - `code-review`: Scans changed files against repository guidelines and bugs with the chosen confidence threshold.
   - `code-review-expert`: Audits changes for SOLID principles, coupling, and P0-P3 risks.
   - `feedback-on-resolution`: Fetches previous PR comments via `gh api`, compares against recent commits, and reports which findings are resolved.
6. The script submits the review back to the pull request via `gh pr review` or `gh pr comment`.
