#!/usr/bin/env python3
"""Autonomous pull request reviewer using an agent tool loop with a hosted model."""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, cast

from openai import APIError, APIStatusError, OpenAI

MAX_AGENT_STEPS = 500
MAX_DIFF_CHARS = 15000
ALLOWED_COMMANDS = {"cargo", "pytest", "ruff", "pyright", "git"}
ALLOWED_GIT_SUBCOMMANDS = {
    "log",
    "diff",
    "show",
    "status",
    "branch",
    "blame",
    "rev-parse",
}
DISALLOWED_GIT_FLAGS = {"-c", "--exec-path", "--config-env", "--paginate", "-p"}
ERR_PREFIX = "Error:"

CODE_REVIEW_LEVELS = {
    "max": (
        "- Intensity: MAX (Exhaustive audit)\n"
        "- Confidence threshold: 50/100 (investigate subtle bugs, edge cases, and boundary conditions).\n"
        "- Use tools actively: read full files, check git blame, and run test suites before finalizing."
    ),
    "low": (
        "- Intensity: LOW (Quick scan)\n"
        "- Confidence threshold: 30/100.\n"
        "- Focus on quick wins, minor readability, naming, and low-impact suggestions."
    ),
    "high": (
        "- Intensity: HIGH (Strict filter, default)\n"
        "- Confidence threshold: 80/100.\n"
        "- Report only verified, high-confidence bugs and repository instruction violations."
    ),
}

EXPERT_REVIEW_LEVELS = {
    "max": (
        "- Intensity: MAX (Exhaustive architecture and security audit)\n"
        "- Report all P0, P1, P2, and P3 findings in detail.\n"
        "- Use tools to inspect callers across the repository and run cargo clippy / test suites."
    ),
    "low": (
        "- Intensity: LOW (High-level architecture scan)\n"
        "- Focus only on immediate structural concerns and critical code smells."
    ),
    "high": (
        "- Intensity: HIGH (Standard expert review, default)\n"
        "- Classify all issues into P0, P1, P2, and P3 severity categories with actionable solutions."
    ),
}

AGENT_TOOLS: list[dict[str, Any]] = [
    {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read contents of a file from the repository, optionally with line bounds.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative file path."},
                    "start_line": {
                        "type": "integer",
                        "description": "Start line (1-indexed).",
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "End line (inclusive).",
                    },
                },
                "required": ["path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "search_code",
            "description": "Search for a string or regex pattern across the repository.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Pattern to search for.",
                    },
                    "path": {
                        "type": "string",
                        "description": "Subdirectory to restrict search.",
                    },
                },
                "required": ["query"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "git_blame",
            "description": "Run git blame on specific lines to see previous commit history and author.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Relative file path."},
                    "start_line": {
                        "type": "integer",
                        "description": "Start line number.",
                    },
                    "end_line": {"type": "integer", "description": "End line number."},
                },
                "required": ["path", "start_line", "end_line"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_command",
            "description": "Run a read-only verification command such as 'cargo test', 'pytest', or 'git log -n 5'.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute.",
                    },
                },
                "required": ["command"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "submit_review",
            "description": "Submit the completed review once all checks, file reads, and tests are finished.",
            "parameters": {
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Overall review summary in Markdown.",
                    },
                    "inline_comments": {
                        "type": "array",
                        "description": "List of inline comments anchored to specific modified lines.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "File path."},
                                "line": {
                                    "type": "integer",
                                    "description": "Line number in the new file.",
                                },
                                "body": {
                                    "type": "string",
                                    "description": "Inline comment body.",
                                },
                            },
                            "required": ["path", "line", "body"],
                        },
                    },
                },
                "required": ["summary"],
            },
        },
    },
]


def get_gh_executable(repo_root: Path) -> str:
    """Return gh in GitHub Actions CI, or scripts/gh locally if available."""
    if os.environ.get("GITHUB_ACTIONS") == "true":
        return "gh"
    script_gh = repo_root / "scripts" / "gh"
    if script_gh.is_file() and os.access(script_gh, os.X_OK):
        return str(script_gh)
    return "gh"


def run_cli_command(cmd: list[str], timeout: int = 30) -> str:
    """Run a CLI command and return its stdout."""
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        if result.returncode != 0:
            return f"{ERR_PREFIX} {result.stderr.strip()}"
        return result.stdout.strip()
    except subprocess.TimeoutExpired:
        return f"{ERR_PREFIX} Command timed out after {timeout} seconds."


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
        print(
            f"[Reviewer Notice]: Missing required environment variables: {', '.join(missing)}. "
            "If this is a fork PR, secrets are not accessible by default.",
            file=sys.stderr,
        )
        sys.exit(0)

    return base_url, api_key, model


def get_pr_metadata(gh: str, pr_number: int) -> dict:
    """Fetch pull request metadata via gh CLI."""
    stdout = run_cli_command(
        [
            gh,
            "pr",
            "view",
            str(pr_number),
            "--json",
            "number,title,body,baseRefName,headRefName,headRefOid,commits",
        ]
    )
    if not stdout or stdout.startswith(ERR_PREFIX):
        sys.exit(f"Failed to fetch metadata for pull request #{pr_number}: {stdout}")
    return json.loads(stdout)


def get_pr_diff(gh: str, pr_number: int) -> str:
    """Fetch pull request diff via gh CLI."""
    diff = run_cli_command([gh, "pr", "diff", str(pr_number)])
    if not diff or diff.startswith(ERR_PREFIX):
        sys.exit(f"Failed to fetch diff for pull request #{pr_number}: {diff}")
    return diff


def load_file_content(path: Path) -> str:
    """Read a file if it exists."""
    if path.is_file():
        return path.read_text(encoding="utf-8")
    return ""


def _find_command_line(lines: list[str]) -> tuple[str, list[str]]:
    for idx, line in enumerate(lines):
        if line.startswith("/"):
            return line, lines[idx + 1 :]
    return lines[0], lines[1:]


def _extract_level_and_instructions(
    parts: list[str], extra_lines: list[str]
) -> tuple[str, list[str], list[str]]:
    level = "high"
    instructions_parts = []

    if parts:
        candidate = parts[0].lower()
        if candidate in ("max", "high", "low"):
            level = candidate
            instructions_parts = parts[1:]
        else:
            instructions_parts = parts
    elif extra_lines:
        first_extra = extra_lines[0].split()
        if first_extra and first_extra[0].lower() in ("max", "high", "low"):
            level = first_extra[0].lower()
            extra_lines = [" ".join(first_extra[1:])] + extra_lines[1:]

    return level, instructions_parts, extra_lines


def parse_comment(comment_body: str) -> tuple[str, str]:
    """Parse review intensity level (max, high, low) and custom instructions from comment."""
    if not comment_body.strip():
        return "high", ""

    lines = [line.strip() for line in comment_body.splitlines() if line.strip()]
    first_line, extra_lines = _find_command_line(lines)

    parts = first_line.split()
    if parts and parts[0].startswith("/"):
        parts = parts[1:]

    level, inst_parts, extra_lines = _extract_level_and_instructions(parts, extra_lines)
    instructions = " ".join(inst_parts).strip()
    if extra_lines:
        additional = "\n".join(extra_lines).strip()
        instructions = (
            f"{instructions}\n{additional}".strip() if instructions else additional
        )

    return level, instructions


def tool_read_file(
    repo_root: Path, path: str, start_line: int = 1, end_line: int = 500
) -> str:
    target = (repo_root / path).resolve()
    if not target.is_relative_to(repo_root.resolve()):
        return f"{ERR_PREFIX} Path traversal outside repository root is blocked."
    if not target.is_file():
        return f"{ERR_PREFIX} File '{path}' does not exist."

    try:
        lines = target.read_text(encoding="utf-8", errors="replace").splitlines()
        total_lines = len(lines)
        start = max(1, start_line)
        if end_line <= 0 or end_line >= total_lines:
            end = total_lines
        else:
            end = min(total_lines, max(start, end_line))
        subset = lines[start - 1 : end]
        numbered = [f"{start + i:4d} | {line}" for i, line in enumerate(subset)]
        return f"File: {path} (lines {start}-{end} of {total_lines}):\n" + "\n".join(
            numbered
        )
    except (OSError, UnicodeDecodeError) as exc:
        return f"{ERR_PREFIX} reading '{path}': {exc}"


def tool_search_code(repo_root: Path, query: str, path: str = ".") -> str:
    """Search for a pattern across the repository using git grep."""
    cmd = ["git", "grep", "-n", "-I", "-e", query, "--", path]
    try:
        res = subprocess.run(
            cmd,
            cwd=repo_root,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        output = res.stdout.strip()
        if not output:
            return "No matches found."
        lines = output.splitlines()
        if len(lines) > 50:
            lines = lines[:50]
            lines.append("... [Results truncated to first 50 matches]")
        return "\n".join(lines)
    except subprocess.TimeoutExpired:
        return f"{ERR_PREFIX} Search timed out."


def tool_git_blame(repo_root: Path, path: str, start_line: int, end_line: int) -> str:
    """Run git blame on specific lines."""
    cmd = ["git", "blame", "-L", f"{start_line},{end_line}", "--", path]
    try:
        res = subprocess.run(
            cmd,
            cwd=repo_root,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        if res.returncode != 0:
            return f"{ERR_PREFIX} running git blame: {res.stderr.strip()}"
        return res.stdout.strip() or "No blame data available."
    except subprocess.TimeoutExpired:
        return f"{ERR_PREFIX} Git blame timed out."


def tool_run_command(repo_root: Path, command: str) -> str:
    parts = shlex.split(command)
    if not parts:
        return f"{ERR_PREFIX} Empty command."
    base_cmd = Path(parts[0]).name
    if base_cmd not in ALLOWED_COMMANDS:
        return f"{ERR_PREFIX} Command '{base_cmd}' is not allowed. Permitted commands: {', '.join(sorted(ALLOWED_COMMANDS))}"

    if base_cmd == "git":
        subcommands = [arg for arg in parts[1:] if not arg.startswith("-")]
        if not subcommands or subcommands[0] not in ALLOWED_GIT_SUBCOMMANDS:
            allowed_sub = ", ".join(sorted(ALLOWED_GIT_SUBCOMMANDS))
            return f"{ERR_PREFIX} Disallowed git subcommand. Permitted: {allowed_sub}"
        for arg in parts[1:]:
            if any(
                arg == flag or arg.startswith(f"{flag}=")
                for flag in DISALLOWED_GIT_FLAGS
            ):
                return f"{ERR_PREFIX} Disallowed git flag '{arg}'."

    clean_env = {
        k: v
        for k, v in os.environ.items()
        if not any(
            secret_term in k
            for secret_term in ("TOKEN", "API_KEY", "SECRET", "BASE_URL")
        )
    }

    try:
        res = subprocess.run(
            parts,
            cwd=repo_root,
            env=clean_env,
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        combined = f"Exit code: {res.returncode}\n--- stdout ---\n{res.stdout.strip()}\n--- stderr ---\n{res.stderr.strip()}"
        if len(combined) > 6000:
            combined = combined[:6000] + "\n... [Command output truncated]"
        return combined
    except subprocess.TimeoutExpired:
        return f"{ERR_PREFIX} Command timed out after 60 seconds."
    except OSError as exc:
        return f"{ERR_PREFIX} Failed to execute '{parts[0]}': {exc}"


def execute_tool_call(
    repo_root: Path, tool_name: str, tool_args: dict[str, Any]
) -> tuple[str, bool, dict[str, Any] | None]:
    """Execute tool and return (result_string, is_final_submission, review_data)."""
    if tool_name == "read_file":
        result = tool_read_file(
            repo_root,
            tool_args.get("path", ""),
            tool_args.get("start_line", 1),
            tool_args.get("end_line", 200),
        )
        return result, False, None
    if tool_name == "search_code":
        result = tool_search_code(
            repo_root, tool_args.get("query", ""), tool_args.get("path", ".")
        )
        return result, False, None
    if tool_name == "git_blame":
        result = tool_git_blame(
            repo_root,
            tool_args.get("path", ""),
            tool_args.get("start_line", 1),
            tool_args.get("end_line", 50),
        )
        return result, False, None
    if tool_name == "run_command":
        result = tool_run_command(repo_root, tool_args.get("command", ""))
        return result, False, None
    if tool_name == "submit_review":
        return "Review accepted for submission.", True, tool_args

    return f"{ERR_PREFIX} Unknown tool '{tool_name}'", False, None


def _build_code_review_prompt(
    skill_content: str,
    level_instructions: str,
    user_focus: str,
    repo_guidelines: str,
    pr_meta: dict,
    diff_text: str,
) -> tuple[str, str]:
    system_prompt = (
        "You are an autonomous AI code reviewer operating with an interactive tool loop. "
        "You actively read files, check git history, and run test suites before submitting your findings."
    )
    user_prompt = f"""Follow these instructions to audit this pull request:

## Instructions
{skill_content}

## Active Review Settings
{level_instructions}
{user_focus}
## Repository Context
{repo_guidelines}

## Pull Request Details
- Title: {pr_meta.get("title")}
- Number: #{pr_meta.get("number")}
- Base branch: {pr_meta.get("baseRefName")}
- Head branch: {pr_meta.get("headRefName")}

### Pull Request Description
{pr_meta.get("body") or "(No description provided)"}

### Initial Pull Request Diff
```diff
{diff_text}
```

Use your available tools (`read_file`, `search_code`, `git_blame`, `run_command`) to inspect context.
When your audit is complete, call `submit_review(summary=..., inline_comments=...)` with your report.
"""
    return system_prompt, user_prompt


def _build_expert_review_prompt(
    skill_content: str,
    level_instructions: str,
    user_focus: str,
    repo_guidelines: str,
    pr_meta: dict,
    diff_text: str,
) -> tuple[str, str]:
    system_prompt = (
        "You are a senior software architect and security auditor operating with an interactive tool loop. "
        "You investigate SOLID design principles, modular boundaries, security vulnerabilities, and code quality."
    )
    user_prompt = f"""Audit this pull request against SOLID design and security standards:

## Instructions
{skill_content}

## Active Review Settings
{level_instructions}
{user_focus}
## Repository Context
{repo_guidelines}

## Pull Request Details
- Title: {pr_meta.get("title")}
- Number: #{pr_meta.get("number")}

### Pull Request Description
{pr_meta.get("body") or "(No description provided)"}

### Initial Pull Request Diff
```diff
{diff_text}
```

Use your available tools to check code and run tests.
When complete, call `submit_review(summary=..., inline_comments=...)` with your structured report.
"""
    return system_prompt, user_prompt


def _build_resolution_prompt(
    gh: str,
    pr_number: int,
    pr_meta: dict,
    diff_text: str,
    user_focus: str,
    level_instructions: str,
) -> tuple[str, str]:
    comments_json = run_cli_command(
        [
            gh,
            "api",
            f"repos/:owner/:repo/pulls/{pr_number}/comments",
            "--jq",
            "[.[] | {path: .path, line: .line, body: .body, user: .user.login}]",
        ]
    )
    reviews_json = run_cli_command(
        [
            gh,
            "api",
            f"repos/:owner/:repo/pulls/{pr_number}/reviews",
            "--jq",
            '[.[] | select(.body != "") | {body: .body, state: .state, user: .user.login}]',
        ]
    )
    commits = pr_meta.get("commits", [])
    commit_messages = "\n".join(
        f"- {c.get('messageHeadline', '')}" for c in commits[-10:]
    )
    system_prompt = (
        "You are an automated pull request reviewer verifying whether issues from previous "
        "reviews have been resolved by recent commits."
    )
    user_prompt = f"""Verify whether the author has addressed previous review feedback.

## Active Settings
{level_instructions}
{user_focus}

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
{diff_text}
```

Use your tools to check files and verify fixes.
When complete, call `submit_review(summary=..., inline_comments=...)` classifying each previous item as [RESOLVED], [UNRESOLVED], or [NEW ISSUE].
"""
    return system_prompt, user_prompt


def build_agent_prompts(
    gh: str,
    mode: str,
    pr_number: int,
    repo_root: Path,
    level: str,
    custom_instructions: str,
) -> tuple[str, str]:
    """Construct system prompt and user prompt based on mode, level, and instructions."""
    pr_meta = get_pr_metadata(gh, pr_number)
    raw_diff = get_pr_diff(gh, pr_number)

    if len(raw_diff) > MAX_DIFF_CHARS:
        diff_text = (
            raw_diff[:MAX_DIFF_CHARS]
            + "\n\n... [Diff truncated due to size. Use read_file to inspect full files.]"
        )
    else:
        diff_text = raw_diff

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

    user_focus = (
        f"\n## User-Requested Focus and Constraints\n{custom_instructions}\n"
        if custom_instructions
        else ""
    )

    if mode == "code-review":
        skill_content = load_file_content(
            repo_root / ".github" / "skills" / "code-review" / "SKILL.md"
        )
        level_instructions = CODE_REVIEW_LEVELS.get(level, CODE_REVIEW_LEVELS["high"])
        return _build_code_review_prompt(
            skill_content,
            level_instructions,
            user_focus,
            repo_guidelines,
            pr_meta,
            diff_text,
        )
    if mode == "code-review-expert":
        skill_content = load_file_content(
            repo_root / ".github" / "skills" / "code-review-expert" / "SKILL.md"
        )
        level_instructions = EXPERT_REVIEW_LEVELS.get(
            level, EXPERT_REVIEW_LEVELS["high"]
        )
        return _build_expert_review_prompt(
            skill_content,
            level_instructions,
            user_focus,
            repo_guidelines,
            pr_meta,
            diff_text,
        )
    if mode == "feedback-on-resolution":
        level_instructions = f"- Review level: {level.upper()}"
        return _build_resolution_prompt(
            gh, pr_number, pr_meta, diff_text, user_focus, level_instructions
        )

    sys.exit(f"Unknown review mode: {mode}")


def create_progress_comment(gh: str, pr_number: int, mode: str, level: str) -> str:
    """Post an initial progress comment to the PR and return its comment ID."""
    body = (
        f"**Review in progress** (`{mode}`, level: `{level.upper()}`)...\n\n"
        "Autonomous agent is inspecting files, checking git blame, and running checks. Findings will appear here shortly."
    )
    cmd = [
        gh,
        "api",
        f"repos/:owner/:repo/issues/{pr_number}/comments",
        "--input",
        "-",
        "--jq",
        ".id",
    ]
    payload = json.dumps({"body": body})
    result = subprocess.run(
        cmd, input=payload, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        return ""
    return result.stdout.strip()


def update_progress_comment(gh: str, comment_id: str, content: str) -> bool:
    """Update existing progress comment."""
    if not comment_id:
        return False
    cmd = [
        gh,
        "api",
        f"repos/:owner/:repo/issues/comments/{comment_id}",
        "-X",
        "PATCH",
        "--input",
        "-",
    ]
    payload = json.dumps({"body": content})
    result = subprocess.run(
        cmd, input=payload, capture_output=True, text=True, check=False
    )
    return result.returncode == 0


def _submit_batch_review(
    gh: str, pr_number: int, head_sha: str, summary: str, inline_comments: list[dict]
) -> bool:
    review_payload = {
        "commit_id": head_sha,
        "body": summary,
        "event": "COMMENT",
        "comments": inline_comments,
    }
    cmd = [
        gh,
        "api",
        f"repos/:owner/:repo/pulls/{pr_number}/reviews",
        "--input",
        "-",
    ]
    result = subprocess.run(
        cmd,
        input=json.dumps(review_payload),
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        print(
            f"Successfully posted review with {len(inline_comments)} inline comments on pull request #{pr_number}"
        )
        return True
    print(
        f"Batch review submission failed ({result.stderr.strip()}). Attempting individual comment posting..."
    )
    return False


def _submit_individual_inline_comments(
    gh: str, pr_number: int, head_sha: str, inline_comments: list[dict]
) -> int:
    posted_count = 0
    for item in inline_comments:
        single_cmd = [
            gh,
            "api",
            f"repos/:owner/:repo/pulls/{pr_number}/comments",
            "-f",
            f"body={item.get('body')}",
            "-f",
            f"commit_id={head_sha}",
            "-f",
            f"path={item.get('path')}",
            "-F",
            f"line={item.get('line')}",
            "-f",
            "side=RIGHT",
        ]
        single_res = subprocess.run(
            single_cmd, capture_output=True, text=True, check=False
        )
        if single_res.returncode == 0:
            posted_count += 1
    return posted_count


def _submit_inline_or_fallback(
    gh: str,
    pr_number: int,
    head_sha: str,
    summary: str,
    inline_comments: list[dict],
    progress_comment_id: str,
) -> bool:
    if _submit_batch_review(gh, pr_number, head_sha, summary, inline_comments):
        if progress_comment_id:
            update_progress_comment(
                gh,
                progress_comment_id,
                "Review completed. Detailed findings posted in review above.",
            )
        return True

    posted = _submit_individual_inline_comments(
        gh, pr_number, head_sha, inline_comments
    )
    if posted > 0:
        print(
            f"Successfully posted {posted}/{len(inline_comments)} inline comments individually."
        )
        if progress_comment_id:
            update_progress_comment(gh, progress_comment_id, summary)
        return True

    return False


def post_final_review(
    gh: str,
    pr_number: int,
    summary: str,
    inline_comments: list[dict],
    progress_comment_id: str = "",
) -> None:
    """Submit formal review with summary and inline comments."""
    head_sha = run_cli_command(
        [
            gh,
            "pr",
            "view",
            str(pr_number),
            "--json",
            "headRefOid",
            "--jq",
            ".headRefOid",
        ]
    )

    if inline_comments and head_sha and not head_sha.startswith(ERR_PREFIX):
        if _submit_inline_or_fallback(
            gh, pr_number, head_sha, summary, inline_comments, progress_comment_id
        ):
            return
        formatted_inline = "\n\n### Line Findings\n" + "\n".join(
            f"- `{c['path']}:{c['line']}`: {c['body']}" for c in inline_comments
        )
        summary += formatted_inline

    if progress_comment_id and update_progress_comment(
        gh, progress_comment_id, summary
    ):
        print(
            f"Successfully updated review comment #{progress_comment_id} on pull request #{pr_number}"
        )
        return

    fallback = subprocess.run(
        [gh, "pr", "comment", str(pr_number), "--body-file", "-"],
        input=summary,
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


def _execute_with_loop_guard(
    repo_root: Path,
    tool_name: str,
    tool_args: dict[str, Any],
    recent_calls: list[str],
) -> tuple[str, bool, dict[str, Any] | None]:
    sig = f"{tool_name}:{json.dumps(tool_args, sort_keys=True)}"
    if recent_calls.count(sig) >= 2 and tool_name != "submit_review":
        msg = (
            f"{ERR_PREFIX} Loop detected: you have called '{tool_name}' with these exact arguments "
            "multiple times. Do not re-call this tool with identical arguments. Proceed to the next step or call submit_review."
        )
        return msg, False, None

    recent_calls.append(sig)
    if len(recent_calls) > 10:
        recent_calls.pop(0)

    try:
        return execute_tool_call(repo_root, tool_name, tool_args)
    except Exception as exc:
        return (
            f"{ERR_PREFIX} Tool '{tool_name}' failed with unexpected error: {exc}",
            False,
            None,
        )


def _process_single_tool_call(
    repo_root: Path, tool_call: Any, recent_calls: list[str]
) -> tuple[dict[str, Any], bool, dict[str, Any] | None]:
    tool_name = tool_call.function.name
    try:
        tool_args = json.loads(tool_call.function.arguments or "{}")
    except json.JSONDecodeError:
        tool_args = {}

    print(f"  -> Tool Call: {tool_name}({tool_args})")
    tool_output, is_final, review_data = _execute_with_loop_guard(
        repo_root, tool_name, tool_args, recent_calls
    )

    if not is_final:
        snippet = tool_output[:120] + "..." if len(tool_output) > 120 else tool_output
        print(f"     Observation: {snippet}")

    tool_msg = {
        "role": "tool",
        "tool_call_id": tool_call.id,
        "content": tool_output,
    }
    return tool_msg, is_final, review_data


def query_model_with_retry(
    client: OpenAI,
    model: str,
    messages: list[dict[str, Any]],
    tools: list[dict[str, Any]],
    retry_interval_seconds: int = 60,
    max_total_wait_seconds: int = 1200,
) -> Any:
    """Query model API with automated retry loop when service is down (waiting up to 20 mins)."""
    start_wait = time.time()
    attempt = 0

    while True:
        attempt += 1
        try:
            return client.chat.completions.create(
                model=model,
                messages=cast(Any, messages),
                tools=cast(Any, tools),
                tool_choice="auto",
                temperature=0.2,
            )
        except (APIError, OSError) as exc:
            if (
                isinstance(exc, APIStatusError)
                and 400 <= exc.status_code < 500
                and exc.status_code != 429
            ):
                print(
                    f"[Model Service]: Non-transient client error {exc.status_code}: {exc}. Halting immediately without retry.",
                    file=sys.stderr,
                )
                raise

            elapsed = time.time() - start_wait
            if elapsed >= max_total_wait_seconds:
                print(
                    f"[Model Service]: Exhausted max retry time ({elapsed:.0f}s). Final error: {exc}",
                    file=sys.stderr,
                )
                raise

            print(
                f"[Model Service Notice]: Service down or unreachable ({exc}). "
                f"Waiting {retry_interval_seconds}s before retry (attempt {attempt}, elapsed: {elapsed:.0f}s / {max_total_wait_seconds}s)..."
            )
            time.sleep(retry_interval_seconds)


def _prune_message_history(
    messages: list[dict[str, Any]], max_chars: int = 50000
) -> None:
    """Compact older tool observation messages if total length exceeds limit."""
    total_len = sum(len(str(m.get("content") or "")) for m in messages)
    if total_len <= max_chars:
        return

    # Keep system and initial user prompt intact. Shorten tool observations older than the most recent 2 messages.
    cutoff = max(2, len(messages) - 2)
    for i in range(2, cutoff):
        msg = messages[i]
        if msg.get("role") == "tool" and len(str(msg.get("content") or "")) > 300:
            msg["content"] = (
                str(msg["content"])[:250]
                + "\n... [Observation compacted to save tokens]"
            )


def _check_step_milestones(
    step: int, max_steps: int, notice_steps: tuple[int, int]
) -> str | None:
    if step == notice_steps[0]:
        return (
            f"Notice: You have reached step {step} of {max_steps}. "
            "Please wrap up your file inspection, formulate your findings, and prepare to call submit_review."
        )
    if step == notice_steps[1]:
        remaining = max_steps - step
        return (
            f"Urgent notice: You have reached step {step} of {max_steps}. "
            f"You have only {remaining} steps remaining before the hard limit. Call submit_review now."
        )
    return None


def run_agent_loop(
    client: OpenAI,
    model: str,
    system_prompt: str,
    user_prompt: str,
    repo_root: Path,
    max_steps: int = 200,
    notice_steps: tuple[int, int] = (150, 175),
) -> tuple[str, list[dict], int]:
    """Execute autonomous agent loop with tools until review submission or max steps."""
    messages: list[dict[str, Any]] = [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": user_prompt},
    ]

    total_model_calls = 0
    start_time = time.time()
    recent_calls: list[str] = []

    for step in range(1, max_steps + 1):
        _prune_message_history(messages)
        total_model_calls += 1
        print(f"\n[Agent Step {step}/{max_steps}] Querying model '{model}'...")

        response = query_model_with_retry(
            client=client,
            model=model,
            messages=messages,
            tools=AGENT_TOOLS,
        )

        message = response.choices[0].message
        messages.append(message.model_dump(exclude_none=True))

        if not message.tool_calls:
            content = message.content or ""
            print(f"[Agent]: Direct response generated ({len(content)} chars).")
            elapsed = time.time() - start_time
            print(
                f"[Telemetry]: Completed in {elapsed:.1f}s across {total_model_calls} model calls."
            )
            return content, [], total_model_calls

        for tool_call in message.tool_calls:
            tool_msg, is_final, review_data = _process_single_tool_call(
                repo_root, tool_call, recent_calls
            )
            if is_final and review_data:
                elapsed = time.time() - start_time
                print(
                    f"\n[Agent]: Final review submitted at step {step} ({elapsed:.1f}s, {total_model_calls} calls)."
                )
                summary = str(review_data.get("summary", "")).strip()
                inline_comments = review_data.get("inline_comments") or []
                return summary, inline_comments, total_model_calls

            messages.append(tool_msg)

        milestone_reminder = _check_step_milestones(step, max_steps, notice_steps)
        if milestone_reminder:
            print(f"  [Milestone Alert]: {milestone_reminder}")
            messages.append({"role": "user", "content": milestone_reminder})

    elapsed = time.time() - start_time
    print(
        f"\n[Agent]: Reached maximum step limit ({max_steps}). Finalizing review ({elapsed:.1f}s)."
    )
    last_content = (
        messages[-1].get("content", "") if isinstance(messages[-1], dict) else ""
    )
    return (
        str(last_content) or "Review completed with maximum step limit.",
        [],
        total_model_calls,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Autonomous PR Reviewer")
    parser.add_argument(
        "--mode",
        required=True,
        choices=["code-review", "code-review-expert", "feedback-on-resolution"],
    )
    parser.add_argument("--pr", type=int, default=int(os.environ.get("PR_NUMBER") or 0))
    parser.add_argument("--comment", default=os.environ.get("COMMENT_BODY", ""))
    parser.add_argument("--level", choices=["max", "high", "low"], default=None)
    args = parser.parse_args()

    if args.pr <= 0:
        sys.exit(
            "A valid pull request number must be specified via --pr or PR_NUMBER environment variable."
        )

    base_url, api_key, model = load_config()
    repo_root = Path(__file__).resolve().parent.parent.parent
    gh = get_gh_executable(repo_root)

    parsed_level, custom_instructions = parse_comment(args.comment)
    final_level = args.level or parsed_level

    print(
        f"Starting autonomous agent reviewer in mode '{args.mode}' (level: {final_level}) on PR #{args.pr}..."
    )
    if custom_instructions:
        print(f"Custom user focus: {custom_instructions}")

    progress_comment_id = create_progress_comment(gh, args.pr, args.mode, final_level)

    system_prompt, user_prompt = build_agent_prompts(
        gh, args.mode, args.pr, repo_root, final_level, custom_instructions
    )

    if args.mode == "feedback-on-resolution":
        max_steps = 80
        notice_steps = (60, 70)
    else:
        max_steps = 200
        notice_steps = (150, 175)

    client = OpenAI(base_url=base_url, api_key=api_key)
    summary, inline_comments, total_calls = run_agent_loop(
        client,
        model,
        system_prompt,
        user_prompt,
        repo_root,
        max_steps=max_steps,
        notice_steps=notice_steps,
    )
    print(f"Review session completed with {total_calls} model interactions.")

    if not summary.strip():
        sys.exit("Agent terminated without producing a review summary.")

    post_final_review(gh, args.pr, summary, inline_comments, progress_comment_id)


if __name__ == "__main__":
    main()
