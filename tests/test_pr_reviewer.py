import importlib.util
from pathlib import Path

# Dynamically import pr_reviewer from .github/workflows/pr_reviewer.py
reviewer_path = (
    Path(__file__).resolve().parent.parent / ".github" / "workflows" / "pr_reviewer.py"
)
spec = importlib.util.spec_from_file_location("pr_reviewer", reviewer_path)
assert spec and spec.loader
pr_reviewer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pr_reviewer)


def test_command_runner_rejects_chained_commands():
    repo_root = Path(__file__).resolve().parent.parent
    res = pr_reviewer.tool_run_command(repo_root, "git status -sb && git diff")
    assert "Error:" in res
    assert "Chained commands and shell operators are not permitted" in res


def test_command_runner_rejects_semicolon_chain():
    repo_root = Path(__file__).resolve().parent.parent
    res = pr_reviewer.tool_run_command(repo_root, "git status ; ls")
    assert "Error:" in res
    assert "Chained commands and shell operators are not permitted" in res


def test_command_runner_rejects_pipes_and_redirects():
    repo_root = Path(__file__).resolve().parent.parent
    res_pipe = pr_reviewer.tool_run_command(repo_root, "git status | grep foo")
    assert "Error:" in res_pipe
    assert "Chained commands and shell operators are not permitted" in res_pipe

    res_redir = pr_reviewer.tool_run_command(repo_root, "git status > /tmp/out")
    assert "Error:" in res_redir
    assert "Chained commands and shell operators are not permitted" in res_redir


def test_command_runner_rejects_subshells_and_backticks():
    repo_root = Path(__file__).resolve().parent.parent
    res_subshell = pr_reviewer.tool_run_command(repo_root, "git log $(whoami)")
    assert "Error:" in res_subshell
    assert "Chained commands and shell operators are not permitted" in res_subshell

    res_backtick = pr_reviewer.tool_run_command(repo_root, "git log `whoami`")
    assert "Error:" in res_backtick
    assert "Chained commands and shell operators are not permitted" in res_backtick


def test_command_runner_rejects_disallowed_commands():
    repo_root = Path(__file__).resolve().parent.parent
    res = pr_reviewer.tool_run_command(repo_root, "curl https://example.com")
    assert "Error:" in res
    assert "Command 'curl' is not allowed" in res


def test_command_runner_rejects_disallowed_git_subcommands():
    repo_root = Path(__file__).resolve().parent.parent
    res = pr_reviewer.tool_run_command(repo_root, "git push origin main")
    assert "Error:" in res
    assert "Disallowed git subcommand" in res


def test_command_runner_executes_allowed_command():
    repo_root = Path(__file__).resolve().parent.parent
    res = pr_reviewer.tool_run_command(repo_root, "git status --short")
    assert "Exit code: 0" in res
