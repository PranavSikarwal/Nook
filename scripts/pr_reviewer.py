#!/usr/bin/env python3
"""Automated pull request reviewer using a hosted OpenAI-compatible model."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from openai import OpenAI


def run_command(cmd: list[str]) -> str:
    """Run a CLI command and return its stdout."""
    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(
            f"Command failed ({' '.join(cmd)}): {result.stderr.strip()}",
            file=sys.stderr,
        )
        return ""
    return result.stdout.strip()


def load_config() -> tuple[str, str, str]:
    """Load model endpoint configuration from environment."""
    base_url = os.environ.get("REVIEWER_BASE_URL") or os.environ.get("NOOK_BASE_URL")
    api_key = os.environ.get("REVIEWER_API_KEY") or os.environ.get("NOOK_API_KEY")
    model = os.environ.get("REVIEWER_MODEL") or os.environ.get("NOOK_MODEL")

    missing = []
    if not base_url:
        missing.append("REVIEWER_BASE_URL (or NOOK_BASE_URL)")
    if not api_key:
        missing.append("REVIEWER_API_KEY (or NOOK_API_KEY)")
    if not model:
        missing.append("REVIEWER_MODEL (or NOOK_MODEL)")

    if not base_url or not api_key or not model:
        sys.exit(f"Missing required environment variables: {', '.join(missing)}")

    return base_url, api_key, model


def get_pr_metadata(pr_number: int) -> dict:
    """Fetch pull request metadata via gh CLI."""
    stdout = run_command(
        [
            "gh",
            "pr",
            "view",
            str(pr_number),
            "--json",
            "number,title,body,baseRefName,headRefName,headRefOid,commits",
        ]
    )
    if not stdout:
        sys.exit(f"Failed to fetch metadata for pull request #{pr_number}")
    return json.loads(stdout)


def get_pr_diff(pr_number: int) -> str:
    """Fetch pull request diff via gh CLI."""
    diff = run_command(["gh", "pr", "diff", str(pr_number)])
    if not diff:
        sys.exit(f"Failed to fetch diff for pull request #{pr_number}")
    return diff


def load_file_content(path: Path) -> str:
    """Read a file if it exists."""
    if path.is_file():
        return path.read_text(encoding="utf-8")
    return ""


def get_previous_feedback(pr_number: int) -> tuple[str, str]:
    """Fetch previous review comments and reviews for resolution verification."""
    comments_json = run_command(
        [
            "gh",
            "api",
            f"repos/:owner/:repo/pulls/{pr_number}/comments",
            "--jq",
            "[.[] | {path: .path, line: .line, body: .body, user: .user.login}]",
        ]
    )
    reviews_json = run_command(
        [
            "gh",
            "api",
            f"repos/:owner/:repo/pulls/{pr_number}/reviews",
            "--jq",
            '[.[] | select(.body != "") | {body: .body, state: .state, user: .user.login}]',
        ]
    )
    return comments_json, reviews_json


def parse_comment(comment_body: str) -> tuple[str, str]:
    """Parse review intensity level (max, high, low) and custom instructions from comment."""
    if not comment_body.strip():
        return "high", ""

    lines = [line.strip() for line in comment_body.splitlines() if line.strip()]
    first_line = ""
    extra_lines: list[str] = []
    for idx, line in enumerate(lines):
        if line.startswith("/"):
            first_line = line
            extra_lines = lines[idx + 1 :]
            break

    if not first_line:
        first_line = lines[0]
        extra_lines = lines[1:]

    parts = first_line.split()
    if parts and parts[0].startswith("/"):
        parts = parts[1:]

    level = "high"
    instructions_parts = []

    if parts:
        candidate = parts[0].lower()
        if candidate in ("max", "high", "low"):
            level = candidate
            instructions_parts = parts[1:]
        else:
            instructions_parts = parts

    instructions = " ".join(instructions_parts).strip()
    if extra_lines:
        additional = "\n".join(extra_lines).strip()
        instructions = (
            f"{instructions}\n{additional}".strip() if instructions else additional
        )

    return level, instructions


def build_prompt(
    mode: str,
    pr_number: int,
    repo_root: Path,
    level: str = "high",
    custom_instructions: str = "",
) -> tuple[str, str]:
    """Construct system prompt and user prompt based on mode, level, and instructions."""
    pr_meta = get_pr_metadata(pr_number)
    diff = get_pr_diff(pr_number)

    claude_md = load_file_content(repo_root / "CLAUDE.md")
    agents_md = load_file_content(repo_root / "AGENTS.md")

    repo_guidelines = ""
    if agents_md:
        repo_guidelines += (
            f"\n### Repository AGENTS.md\n```markdown\n{agents_md}\n```\n"
        )
    if claude_md:
        repo_guidelines += (
            f"\n### Repository CLAUDE.md\n```markdown\n{claude_md}\n```\n"
        )

    user_focus = ""
    if custom_instructions:
        user_focus = (
            f"\n## User-Requested Focus and Constraints\n{custom_instructions}\n"
        )

    if mode == "code-review":
        skill_content = load_file_content(
            repo_root / ".github" / "skills" / "code-review.md"
        )
        if level == "max":
            level_instructions = (
                "- Intensity: MAX (Exhaustive audit)\n"
                "- Confidence threshold: 50/100 (include subtle bugs, edge cases, and architectural friction).\n"
                "- Inspect every file thoroughly for boundary conditions and unintended side effects."
            )
        elif level == "low":
            level_instructions = (
                "- Intensity: LOW (Quick scan)\n"
                "- Confidence threshold: 30/100.\n"
                "- Focus on quick wins, minor readability, naming, and low-impact suggestions."
            )
        else:
            level_instructions = (
                "- Intensity: HIGH (Strict filter, default)\n"
                "- Confidence threshold: 80/100.\n"
                "- Report only verified, high-confidence bugs and repository instruction violations. Filter all minor nitpicks."
            )

        system_prompt = (
            f"You are an expert pull request reviewer running at intensity level '{level.upper()}'. "
            "You perform code audits based on repository instructions and bug scans."
        )
        user_prompt = f"""Follow the instructions below to review this pull request:

## Instructions
{skill_content}

## Active Review Settings
{level_instructions}
{user_focus}
## Repository Context
{repo_guidelines}

## Pull Request Details
- Title: {pr_meta.get("title")}
- Number: #{pr_number}
- Base branch: {pr_meta.get("baseRefName")}
- Head branch: {pr_meta.get("headRefName")}

### Pull Request Description
{pr_meta.get("body") or "(No description provided)"}

### Git Diff
```diff
{diff}
```
"""
    elif mode == "code-review-expert":
        skill_content = load_file_content(
            repo_root / ".github" / "skills" / "code-review-expert.md"
        )
        if level == "max":
            level_instructions = (
                "- Intensity: MAX (Exhaustive architecture and security audit)\n"
                "- Report all P0, P1, P2, and P3 findings in detail.\n"
                "- Provide in-depth analysis on modular boundaries, SOLID principles, and dead code removal plans."
            )
        elif level == "low":
            level_instructions = (
                "- Intensity: LOW (High-level architecture scan)\n"
                "- Focus only on immediate structural concerns and critical code smells."
            )
        else:
            level_instructions = (
                "- Intensity: HIGH (Standard expert review, default)\n"
                "- Classify all issues into P0, P1, P2, and P3 severity categories with actionable solutions."
            )

        system_prompt = (
            f"You are a senior software architect running at intensity level '{level.upper()}'. "
            "You audit pull requests for SOLID design, modular coupling, and security risks with P0-P3 classifications."
        )
        user_prompt = f"""Follow the instructions below to review this pull request:

## Instructions
{skill_content}

## Active Review Settings
{level_instructions}
{user_focus}
## Repository Context
{repo_guidelines}

## Pull Request Details
- Title: {pr_meta.get("title")}
- Number: #{pr_number}

### Pull Request Description
{pr_meta.get("body") or "(No description provided)"}

### Git Diff
```diff
{diff}
```
"""
    elif mode == "feedback-on-resolution":
        comments_json, reviews_json = get_previous_feedback(pr_number)
        commits = pr_meta.get("commits", [])
        commit_messages = "\n".join(
            f"- {c.get('messageHeadline', '')}" for c in commits[-10:]
        )
        system_prompt = (
            "You are an automated pull request reviewer verifying whether issues from previous "
            "reviews have been resolved by recent commits."
        )
        user_prompt = f"""Verify whether the author has addressed previous review feedback.

## Task
1. Inspect the previous review comments and reviews below.
2. Examine the recent commit messages and current git diff.
3. For each previous finding, classify it as:
   - [RESOLVED]: The issue was fixed correctly.
   - [UNRESOLVED]: The issue was not fixed or the fix is incomplete.
   - [NEW ISSUE]: A regression or new problem was introduced.
4. Output a concise markdown report with specific file citations.

## Previous Inline Comments
```json
{comments_json or "[]"}
```

## Previous Review Summaries
```json
{reviews_json or "[]"}
```

## Recent Commits on Branch
{commit_messages or "(No commit history)"}

## Current Git Diff
```diff
{diff}
```
"""
    else:
        sys.exit(f"Unknown review mode: {mode}")

    return system_prompt, user_prompt


def post_review(pr_number: int, content: str) -> None:
    """Post the review comment to the pull request via gh CLI."""
    # Attempt to post as formal review comment
    result = subprocess.run(
        ["gh", "pr", "review", str(pr_number), "--comment", "-b", content],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        print(f"Successfully posted review to pull request #{pr_number}")
        return

    # Fallback to standard PR comment if review submission is rejected (e.g. self-review restriction)
    fallback = subprocess.run(
        ["gh", "pr", "comment", str(pr_number), "-b", content],
        capture_output=True,
        text=True,
        check=False,
    )
    if fallback.returncode == 0:
        print(f"Successfully posted comment to pull request #{pr_number}")
    else:
        print(
            f"Failed to post comment to pull request #{pr_number}: {fallback.stderr.strip()}",
            file=sys.stderr,
        )
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Automated PR reviewer")
    parser.add_argument(
        "--mode",
        required=True,
        choices=["code-review", "code-review-expert", "feedback-on-resolution"],
        help="Review mode to run",
    )
    parser.add_argument(
        "--pr",
        type=int,
        default=int(os.environ.get("PR_NUMBER", "0")),
        help="Pull request number",
    )
    parser.add_argument(
        "--comment",
        default=os.environ.get("COMMENT_BODY", ""),
        help="Full comment text that triggered the review",
    )
    parser.add_argument(
        "--level",
        choices=["max", "high", "low"],
        default=None,
        help="Explicit review intensity level",
    )
    args = parser.parse_args()

    if args.pr <= 0:
        sys.exit(
            "A valid pull request number must be specified via --pr or PR_NUMBER environment variable."
        )

    base_url, api_key, model = load_config()
    repo_root = Path(__file__).resolve().parent.parent

    parsed_level, custom_instructions = parse_comment(args.comment)
    final_level = args.level or parsed_level

    print(
        f"Running reviewer in mode '{args.mode}' (level: {final_level}) on pull request #{args.pr} using model '{model}'..."
    )
    if custom_instructions:
        print(f"Applying custom instructions: {custom_instructions}")

    system_prompt, user_prompt = build_prompt(
        args.mode,
        args.pr,
        repo_root,
        level=final_level,
        custom_instructions=custom_instructions,
    )

    client = OpenAI(base_url=base_url, api_key=api_key)
    response = client.chat.completions.create(
        model=model,
        messages=[
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt},
        ],
        temperature=0.2,
    )

    review_content = response.choices[0].message.content or ""
    if not review_content.strip():
        sys.exit("Model returned an empty review response.")

    post_review(args.pr, review_content)


if __name__ == "__main__":
    main()
