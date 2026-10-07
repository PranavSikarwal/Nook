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
            "description": (
                "Read contents of a file from the repository, optionally with line bounds. "
                "Note: Deleted files do not exist on disk in the current checkout; to view a deleted "
                "file, run 'git show origin/<base>:<path>' via run_command."
            ),
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
            "name": "get_changed_files",
            "description": "Get the list of all files changed in this pull request with line change stats.",
            "parameters": {
                "type": "object",
                "properties": {},
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "get_file_diff",
            "description": "Get the exact git diff for a specific file changed in this pull request.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative file path to inspect.",
                    },
                },
                "required": ["path"],
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
            "description": (
                "Run a read-only verification command such as 'cargo test', 'pytest', "
                "'git show origin/<base>:<path>' (to inspect deleted files), or 'git log -n 5'."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute.",
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Command timeout in seconds (default: 300, max: 600).",
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
    """Search for a pattern across the repository using git grep with extended regex."""
    cmd = ["git", "grep", "-E", "-n", "-I", "-e", query, "--", path]
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


def _run_single_command(
    repo_root: Path,
    parts: list[str],
    clean_env: dict[str, str],
    timeout: int = 300,
) -> tuple[int, str, str]:
    if not parts:
        return 1, "", f"{ERR_PREFIX} Empty command."
    base_cmd = Path(parts[0]).name
    if base_cmd not in ALLOWED_COMMANDS:
        return (
            1,
            "",
            f"{ERR_PREFIX} Command '{base_cmd}' is not allowed. Permitted commands: {', '.join(sorted(ALLOWED_COMMANDS))}",
        )

    if base_cmd == "git":
        subcommands = [arg for arg in parts[1:] if not arg.startswith("-")]
        if not subcommands or subcommands[0] not in ALLOWED_GIT_SUBCOMMANDS:
            allowed_sub = ", ".join(sorted(ALLOWED_GIT_SUBCOMMANDS))
            return (
                1,
                "",
                f"{ERR_PREFIX} Disallowed git subcommand. Permitted: {allowed_sub}",
            )
        for arg in parts[1:]:
            if any(
                arg == flag or arg.startswith(f"{flag}=")
                for flag in DISALLOWED_GIT_FLAGS
            ):
                return 1, "", f"{ERR_PREFIX} Disallowed git flag '{arg}'."

    res = subprocess.run(
        parts,
        cwd=repo_root,
        env=clean_env,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    return res.returncode, res.stdout.strip(), res.stderr.strip()


DEFAULT_BASE_COMPARE = "origin/main...HEAD"


DISALLOWED_SHELL_TOKENS = {"|", ">", ">>", "<", "$", "`", "||"}
MAX_SUBCOMMANDS = 5


def _group_tokens_into_pipelines(tokens: list[str]) -> list[list[str]]:
    sub_cmds: list[list[str]] = []
    current: list[str] = []
    for token in tokens:
        if token in ("&&", ";"):
            if current:
                sub_cmds.append(current)
                current = []
        else:
            current.append(token)
    if current:
        sub_cmds.append(current)
    return sub_cmds


def tool_run_command(repo_root: Path, command: str, timeout_seconds: int = 300) -> str:
    clean_env = {
        k: v
        for k, v in os.environ.items()
        if not any(
            secret_term in k
            for secret_term in ("TOKEN", "API_KEY", "SECRET", "BASE_URL")
        )
    }

    effective_timeout = min(max(timeout_seconds, 10), 600)

    try:
        tokens = shlex.split(command)
    except ValueError as exc:
        return f"{ERR_PREFIX} Failed to parse command: {exc}"

    if not tokens:
        return f"{ERR_PREFIX} Empty command."

    for token in tokens:
        if token in DISALLOWED_SHELL_TOKENS or any(op in token for op in ("`", "$(")):
            return f"{ERR_PREFIX} Shell operator '{token}' is not supported."

    sub_cmds = _group_tokens_into_pipelines(tokens)
    if len(sub_cmds) > MAX_SUBCOMMANDS:
        return f"{ERR_PREFIX} Too many chained commands (maximum {MAX_SUBCOMMANDS} allowed)."

    stdout_parts: list[str] = []
    stderr_parts: list[str] = []
    last_exit_code = 0
    start_time = time.monotonic()
    deadline = start_time + effective_timeout

    try:
        for cmd_parts in sub_cmds:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return (
                    f"{ERR_PREFIX} Command timed out after {effective_timeout} seconds."
                )

            code, out, err = _run_single_command(
                repo_root, cmd_parts, clean_env, timeout=max(1, int(remaining))
            )
            last_exit_code = code
            if out:
                stdout_parts.append(out)
            if err:
                stderr_parts.append(err)
            if code != 0:
                break

        combined = f"Exit code: {last_exit_code}\n--- stdout ---\n{'\n'.join(stdout_parts)}\n--- stderr ---\n{'\n'.join(stderr_parts)}"
        if len(combined) > 6000:
            combined = combined[:6000] + "\n... [Command output truncated]"
        return combined
    except subprocess.TimeoutExpired:
        return f"{ERR_PREFIX} Command timed out after {effective_timeout} seconds."
    except OSError as exc:
        return f"{ERR_PREFIX} Failed to execute command: {exc}"


def tool_get_changed_files(repo_root: Path, base_ref: str = "main") -> str:
    """Return stat summary and name status of all files changed against base."""
    cmd = ["git", "diff", "--stat", f"origin/{base_ref}...HEAD"]
    try:
        res = subprocess.run(
            cmd, cwd=repo_root, capture_output=True, text=True, timeout=30, check=False
        )
        return res.stdout.strip() or "No changed files."
    except Exception as exc:
        return f"{ERR_PREFIX} Failed to get changed files: {exc}"


def tool_get_file_diff(repo_root: Path, path: str, base_ref: str = "main") -> str:
    """Return git diff for a specific file against base."""
    cmd = ["git", "diff", f"origin/{base_ref}...HEAD", "--", path]
    try:
        res = subprocess.run(
            cmd, cwd=repo_root, capture_output=True, text=True, timeout=30, check=False
        )
        output = res.stdout.strip()
        if not output:
            return f"No diff for file '{path}' against origin/{base_ref}."
        if len(output) > 8000:
            output = output[:8000] + "\n... [Diff truncated to first 8000 characters]"
        return output
    except Exception as exc:
        return f"{ERR_PREFIX} Failed to get diff for '{path}': {exc}"


def execute_tool_call(
    repo_root: Path,
    tool_name: str,
    tool_args: dict[str, Any],
    base_ref: str = "main",
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
    if tool_name == "get_changed_files":
        result = tool_get_changed_files(repo_root, base_ref=base_ref)
        return result, False, None
    if tool_name == "get_file_diff":
        result = tool_get_file_diff(
            repo_root, tool_args.get("path", ""), base_ref=base_ref
        )
        return result, False, None
    if tool_name == "run_command":
        result = tool_run_command(
            repo_root,
            tool_args.get("command", ""),
            tool_args.get("timeout_seconds", 300),
        )
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
    base_ref = pr_meta.get("baseRefName") or "main"
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
- Base branch: {base_ref}
- Head branch: {pr_meta.get("headRefName")}

### Pull Request Description
{pr_meta.get("body") or "(No description provided)"}

### Initial Pull Request Diff
```diff
{diff_text}
```

### Working Tree and Diff Notes
- The working directory contains the PR checkout at its head commit. `git status` shows a clean tree because the commit is already checked out. To inspect changes against the base branch, compare against `origin/{base_ref}` (e.g. `git diff origin/{base_ref}...HEAD`).
- Do NOT use `HEAD^` or `HEAD~1` to inspect the PR, because a pull request contains multiple commits. Always use `origin/{base_ref}...HEAD` to see the full PR diff.
- Deleted files in this PR no longer exist on disk. `read_file` cannot read them directly. To inspect the contents of a deleted file before its removal, run `git show origin/{base_ref}:<path>` via `run_command`.

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
    base_ref = pr_meta.get("baseRefName") or "main"
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
- Base branch: {base_ref}
- Head branch: {pr_meta.get("headRefName")}

### Pull Request Description
{pr_meta.get("body") or "(No description provided)"}

### Initial Pull Request Diff
```diff
{diff_text}
```

### Working Tree and Diff Notes
- The working directory contains the PR checkout at its head commit. `git status` shows a clean tree because the commit is already checked out. To inspect changes against the base branch, compare against `origin/{base_ref}` (e.g. `git diff origin/{base_ref}...HEAD`).
- Do NOT use `HEAD^` or `HEAD~1` to inspect the PR, because a pull request contains multiple commits. Always use `origin/{base_ref}...HEAD` to see the full PR diff.
- Deleted files in this PR no longer exist on disk. `read_file` cannot read them directly. To inspect the contents of a deleted file before its removal, run `git show origin/{base_ref}:<path>` via `run_command`.

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

    if len(raw_diff) > 40000:
        stat_summary = run_cli_command(["git", "diff", "--stat", DEFAULT_BASE_COMPARE])
        diff_text = (
            f"The full pull request diff is {len(raw_diff):,} characters.\n\n"
            f"### Changed Files Summary\n```text\n{stat_summary}\n```\n\n"
            f"### Initial Diff Preview\n"
            + raw_diff[:30000]
            + "\n\n... [Preview truncated due to size. Use 'get_file_diff(path=...)' to inspect any file's exact diff in steps, 'read_file' to inspect complete files, and 'run_command' to execute tests.]"
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
    base_ref: str = "main",
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
        return execute_tool_call(repo_root, tool_name, tool_args, base_ref=base_ref)
    except Exception as exc:
        return (
            f"{ERR_PREFIX} Tool '{tool_name}' failed with unexpected error: {exc}",
            False,
            None,
        )


def _process_single_tool_call(
    repo_root: Path,
    tool_call: Any,
    recent_calls: list[str],
    base_ref: str = "main",
) -> tuple[dict[str, Any], bool, dict[str, Any] | None, str]:
    tool_name = tool_call.function.name
    try:
        tool_args = json.loads(tool_call.function.arguments)
    except (json.JSONDecodeError, TypeError) as exc:
        err_msg = f"{ERR_PREFIX} Invalid JSON arguments for tool '{tool_name}': {exc}"
        return (
            {"role": "tool", "tool_call_id": tool_call.id, "content": err_msg},
            False,
            None,
            f"Tool {tool_name} call failed: invalid JSON arguments",
        )

    tool_output, is_final, review_data = _execute_with_loop_guard(
        repo_root, tool_name, tool_args, recent_calls, base_ref=base_ref
    )
    action_desc = _format_tool_action(tool_name, tool_args)

    if not is_final:
        print(tool_output)
    print("##[endgroup]")

    tool_msg = {
        "role": "tool",
        "tool_call_id": tool_call.id,
        "content": tool_output,
    }
    return tool_msg, is_final, review_data, action_desc


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


def _format_tool_action(tool_name: str, args: dict[str, Any]) -> str:
    if tool_name == "read_file":
        path = args.get("path", "")
        start_line = args.get("start_line", 1)
        end_line = args.get("end_line", 500)
        return f"Read file `{path}` (lines {start_line}-{end_line})"
    if tool_name == "run_command":
        return f"Execute shell command `{args.get('command', '')}`"
    if tool_name == "search_code":
        return f"Search code for `{args.get('query', '')}` in `{args.get('path', '.')}`"
    if tool_name == "git_blame":
        return f"Git blame on `{args.get('path', '')}` (lines {args.get('start_line')}-{args.get('end_line')})"
    if tool_name == "get_changed_files":
        return "List all changed files in PR"
    if tool_name == "get_file_diff":
        return f"Get diff for `{args.get('path', '')}`"
    if tool_name == "submit_review":
        return "Submit completed review"
    return f"Invoke tool `{tool_name}`"


def _write_transcripts(
    repo_root: Path,
    pr_number: int,
    mode: str,
    level: str,
    model: str,
    elapsed: float,
    total_calls: int,
    trace: list[dict[str, Any]],
    summary: str,
    inline_comments: list[dict[str, Any]],
    messages: list[dict[str, Any]],
) -> None:
    """Generate both human-readable Markdown and structured JSON transcripts."""
    md_lines: list[str] = [
        "# Autonomous Code Review Transcript\n",
        f"- **Pull Request**: #{pr_number}",
        f"- **Review Mode**: `{mode}`",
        f"- **Intensity Level**: `{level.upper()}`",
        f"- **Model**: `{model}`",
        f"- **Total Model Interactions**: {total_calls}",
        f"- **Elapsed Execution Time**: {elapsed:.1f} seconds",
        f"- **Generated At**: {time.strftime('%Y-%m-%d %H:%M:%S UTC', time.gmtime())}\n",
        "---\n",
        "## Investigation Trace\n",
    ]

    for item in trace:
        step = item.get("step")
        thought = item.get("thought", "").strip()
        action = item.get("action", "")
        output = item.get("output", "").strip()
        notice = item.get("notice")

        md_lines.append(f"### Step {step}: {action}\n")
        if thought:
            md_lines.append(f"**Agent Reasoning**:\n> {thought}\n")
        if notice:
            md_lines.append(f"> ⚠️ **{notice}**\n")

        line_count = len(output.splitlines()) if output else 0
        md_lines.extend(
            [
                f"<details>\n<summary>View Observation Output ({line_count} lines)</summary>\n",
                f"```text\n{output}\n```\n</details>\n",
            ]
        )

    md_lines.extend(["---\n", "## Final Review Outcome\n", f"{summary}\n"])

    if inline_comments:
        md_lines.append(f"### Inline Diff Comments ({len(inline_comments)} total)\n")
        for comment in inline_comments:
            path = comment.get("path", "")
            line = comment.get("line", "")
            body = comment.get("body", "")
            md_lines.append(f"- **`{path}:{line}`**:\n  {body}\n")

    md_path = repo_root / "review-transcript.md"
    json_path = repo_root / "review-transcript.json"

    md_path.write_text("\n".join(md_lines), encoding="utf-8")

    json_payload = {
        "metadata": {
            "pr_number": pr_number,
            "mode": mode,
            "level": level,
            "model": model,
            "total_calls": total_calls,
            "elapsed_seconds": round(elapsed, 1),
            "timestamp": time.time(),
        },
        "trace": trace,
        "summary": summary,
        "inline_comments": inline_comments,
        "raw_messages": messages,
    }
    json_path.write_text(
        json.dumps(json_payload, indent=2, default=str), encoding="utf-8"
    )
    print(f"Saved review transcripts to {md_path.name} and {json_path.name}.")


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


def _execute_step_tool_calls(
    step: int,
    tool_calls: list[Any],
    model_thought: str,
    repo_root: Path,
    recent_calls: list[str],
    trace: list[dict[str, Any]],
    messages: list[dict[str, Any]],
) -> tuple[str, list[dict[str, Any]]] | None:
    for tool_call in tool_calls:
        tool_msg, is_final, review_data, action_desc = _process_single_tool_call(
            repo_root, tool_call, recent_calls
        )
        trace.append(
            {
                "step": step,
                "thought": model_thought,
                "action": action_desc,
                "output": str(tool_msg.get("content") or ""),
            }
        )

        if is_final and review_data:
            summary = str(review_data.get("summary", "")).strip()
            inline_comments = review_data.get("inline_comments") or []
            return summary, inline_comments

        messages.append(tool_msg)
    return None


def run_agent_loop(
    client: OpenAI,
    model: str,
    system_prompt: str,
    user_prompt: str,
    repo_root: Path,
    pr_number: int,
    mode: str,
    level: str,
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
    trace: list[dict[str, Any]] = []

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
        model_thought = message.content or ""
        if model_thought:
            print(f"[Agent Thoughts]:\n{model_thought.strip()}")

        if not message.tool_calls:
            print(f"[Agent]: Direct response generated ({len(model_thought)} chars).")
            elapsed = time.time() - start_time
            trace.append(
                {
                    "step": step,
                    "thought": model_thought,
                    "action": "Generated direct response",
                    "output": model_thought,
                }
            )
            _write_transcripts(
                repo_root,
                pr_number,
                mode,
                level,
                model,
                elapsed,
                total_model_calls,
                trace,
                model_thought,
                [],
                messages,
            )
            return model_thought, [], total_model_calls

        final_result = _execute_step_tool_calls(
            step,
            message.tool_calls,
            model_thought,
            repo_root,
            recent_calls,
            trace,
            messages,
        )
        if final_result:
            summary, inline_comments = final_result
            elapsed = time.time() - start_time
            print(
                f"\n[Agent]: Final review submitted at step {step} ({elapsed:.1f}s, {total_model_calls} calls)."
            )
            _write_transcripts(
                repo_root,
                pr_number,
                mode,
                level,
                model,
                elapsed,
                total_model_calls,
                trace,
                summary,
                inline_comments,
                messages,
            )
            return summary, inline_comments, total_model_calls

        milestone_reminder = _check_step_milestones(step, max_steps, notice_steps)
        if milestone_reminder:
            print(f"  [Milestone Alert]: {milestone_reminder}")
            messages.append({"role": "user", "content": milestone_reminder})
            if trace:
                trace[-1]["notice"] = milestone_reminder

    elapsed = time.time() - start_time
    print(
        f"\n[Agent]: Reached maximum step limit ({max_steps}). Finalizing review ({elapsed:.1f}s)."
    )
    last_content = (
        messages[-1].get("content", "") if isinstance(messages[-1], dict) else ""
    )
    fallback_summary = str(last_content) or "Review completed with maximum step limit."
    _write_transcripts(
        repo_root,
        pr_number,
        mode,
        level,
        model,
        elapsed,
        total_model_calls,
        trace,
        fallback_summary,
        [],
        messages,
    )
    return (
        fallback_summary,
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
        pr_number=args.pr,
        mode=args.mode,
        level=final_level,
        max_steps=max_steps,
        notice_steps=notice_steps,
    )
    print(f"Review session completed with {total_calls} model interactions.")

    if not summary.strip():
        sys.exit("Agent terminated without producing a review summary.")

    post_final_review(gh, args.pr, summary, inline_comments, progress_comment_id)


if __name__ == "__main__":
    main()
