---
name: code-review
description: Code review a pull request
allowed-tools: "read_file, search_code, git_blame, run_command, submit_review"
disable-model-invocation: false
---

Provide a code review for the given pull request.

To do this, follow these steps systematically using your available tools:

1. Examine the pull request diff, description, and repository guidelines (AGENTS.md and CLAUDE.md).
2. Audit the modified areas:
   a. Check repository instructions to make sure the changes comply with repository rules.
   b. Inspect modified files using `read_file` to understand full context beyond the diff hunks.
   c. Use `git_blame` to understand historical context for modified lines.
   d. Use `search_code` to check for broken call sites or contract mismatches across the codebase.
   e. Optionally run test commands (`cargo test`, `pytest`) to verify behavior.
3. Score each potential issue on a scale from 0 to 100 indicating confidence:
   a. 0: False positive or pre-existing issue.
   b. 25: Somewhat confident. Low verification.
   c. 50: Moderately confident. Verified issue, but minor nitpick.
   d. 75: Highly confident. Real issue directly impacting functionality or breaking rules.
   e. 100: Absolutely certain. Confirmed bug with clear evidence.
4. Filter out any issues with a score less than 80 (unless in `max` intensity mode where threshold is 50).
5. When complete, call `submit_review` with your summary and any inline comments. Keep output brief, cite lines, and avoid emojis.

Examples of false positives, for steps 4 and 5:

- Pre-existing issues
- Something that looks like a bug but is not actually a bug
- Pedantic nitpicks that a senior engineer wouldn't call out
- Issues that a linter, typechecker, or compiler would catch (eg. missing or incorrect imports, type errors, broken tests, formatting issues, pedantic style issues like newlines). No need to run these build steps yourself -- it is safe to assume that they will be run separately as part of CI.
- General code quality issues (eg. lack of test coverage, general security issues, poor documentation), unless explicitly required in CLAUDE.md
- Issues that are called out in CLAUDE.md, but explicitly silenced in the code (eg. due to a lint ignore comment)
- Changes in functionality that are likely intentional or are directly related to the broader change
- Real issues, but on lines that the user did not modify in their pull request

Notes:

- Do not check build signal or attempt to build or typecheck the app. These will run separately, and are not relevant to your code review.
- Refer to `examples/example-execution.md` for the exact subagent prompts, step-by-step trace, and verification rules.
- Make a todo list first
- You must cite and link each bug (eg. if referring to a CLAUDE.md, you must link it)
- For your final comment, follow the following format precisely (assuming for this example that you found 3 issues):

---

### Code review

Found 3 issues:

1. <brief description of bug> (CLAUDE.md says "<...>")

<link to file and line with full sha1 + line range for context, note that you MUST provide the full sha and not use bash here, eg. https://github.com/anthropics/claude-code/blob/1d54823877c4de72b2316a64032a54afc404e619/README.md#L13-L17>

2. <brief description of bug> (some/other/CLAUDE.md says "<...>")

<link to file and line with full sha1 + line range for context>

3. <brief description of bug> (bug due to <file and code snippet>)

<link to file and line with full sha1 + line range for context>

🤖 Generated with [Claude Code](https://claude.ai/code)

<sub>- If this code review was useful, please react with 👍. Otherwise, react with 👎.</sub>

---

- Or, if you found no issues:

---

### Code review

No issues found. Checked for bugs and CLAUDE.md compliance.

🤖 Generated with [Claude Code](https://claude.ai/code)

- When linking to code, follow the following format precisely, otherwise the Markdown preview won't render correctly: https://github.com/anthropics/claude-cli-internal/blob/c21d3c10bc8e898b7ac1a2d745bdc9bc4e423afe/package.json#L10-L15
  - Requires full git sha
  - You must provide the full sha. Commands like `https://github.com/owner/repo/blob/$(git rev-parse HEAD)/foo/bar` will not work, since your comment will be directly rendered in Markdown.
  - Repo name must match the repo you're code reviewing
  - # sign after the file name
  - Line range format is L[start]-L[end]
  - Provide at least 1 line of context before and after, centered on the line you are commenting about (eg. if you are commenting about lines 5-6, you should link to `L4-7`)
