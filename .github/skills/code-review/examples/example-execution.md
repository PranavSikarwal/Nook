# Code Review Execution Example

This document provides a concrete walkthrough of the `code-review` workflow to prevent deviation, skipped steps, or unauthorized tool exploration.

## Core Rules

1. Always create the todo list before executing any step.
2. Use the user's default configured model for all subagents. Never probe models, query endpoint lists, or switch models.
3. Stop at step 1 if the pull request already received a review on the current HEAD commit, unless the user explicitly orders a re-review.
4. Run subagent steps in parallel only when explicitly specified (steps 4 and 5).
5. Never add AI attribution tags or decorative emojis in review comments.

---

## Step-by-Step Execution Trace

### Step 0: Create the Todo List

Initialize a Markdown checklist in the response before calling any tools:

```markdown
### Todo list
- [ ] Check pull request eligibility (closed, draft, simple/automated, prior review)
- [ ] Locate CLAUDE.md files
- [ ] Generate pull request summary
- [ ] Run 5 parallel code review agents
- [ ] Score identified issues in parallel (0-100 scale)
- [ ] Filter issues (threshold >= 80)
- [ ] Re-check pull request eligibility
- [ ] Post review comment via gh CLI
```

---

### Step 1: Subagent Eligibility Check

Spawn one subagent to inspect the pull request state using the GitHub CLI.

**Subagent Prompt:**
```text
Check whether pull request #<PR_NUMBER> is eligible for a new code review.
Inspect the following criteria:
(a) Is the pull request closed?
(b) Is the pull request a draft?
(c) Does it not need a code review (e.g. automated PR, or very simple and obviously ok)?
(d) Does it already have a code review from earlier on the current HEAD commit?

Run:
gh pr view <PR_NUMBER> --json state,isDraft,headRefOid,commits
gh pr view <PR_NUMBER> --comments

Return your findings for (a), (b), (c), and (d). If any of these four criteria are true, state that review must not proceed.
```

**Decision Rule:**
If condition (d) is met (a review already exists on the current HEAD commit), the agent stops immediately and reports this to the user. It does not proceed to step 2 unless the user explicitly instructed a force re-review.

---

### Step 2: Locate Relevant CLAUDE.md Files

Spawn one subagent to discover guidelines.

**Subagent Prompt:**
```text
Find all CLAUDE.md files relevant to pull request #<PR_NUMBER>:
1. Check the repository root.
2. Check directories containing files modified in the pull request.

Return only a list of file paths. Do not read or return file contents.
```

---

### Step 3: Summarize the Pull Request

Spawn one subagent to fetch the pull request changes.

**Subagent Prompt:**
```text
Inspect pull request #<PR_NUMBER> using:
gh pr view <PR_NUMBER>
gh pr diff <PR_NUMBER>

Return a concise summary covering:
1. Files modified.
2. Functional changes introduced.
3. Context or purpose of the pull request.
```

---

### Step 4: Launch 5 Parallel Review Subagents

Launch five subagents concurrently. Pass each agent the diff and PR summary.

#### Agent 1: CLAUDE.md Compliance
```text
Audit the changes in pull request #<PR_NUMBER> against the instructions in these files:
<LIST_OF_CLAUDE_MD_PATHS>

Check only rules that apply to code review. Return any violations with the exact file, line, and CLAUDE.md quote.
```

#### Agent 2: Obvious Bugs
```text
Read the diff of pull request #<PR_NUMBER>. Perform a shallow scan for major functional bugs on modified lines.
Avoid reading extraneous context. Do not flag small style issues, formatting, or compiler/linter errors.
Ignore false positives and intentional behavior changes.
```

#### Agent 3: Git History and Regressions
```text
Inspect git log and git blame for lines modified in pull request #<PR_NUMBER>.
Identify whether these modifications reintroduce previously fixed bugs or break assumptions established in earlier commits.
```

#### Agent 4: Prior PR Context
```text
Inspect closed pull requests that previously modified the same files as pull request #<PR_NUMBER>.
Check whether previous reviewer comments or concerns apply to the changes in this pull request.
```

#### Agent 5: Code Comments
```text
Read the comments and docstrings in the modified files of pull request #<PR_NUMBER>.
Verify that the new code respects all warnings, prerequisites, and instructions in nearby comments.
```

---

### Step 5: Score Issues in Parallel

For each candidate issue produced by step 4, launch a dedicated subagent with the exact scoring rubric:

**Scoring Subagent Prompt:**
```text
Evaluate the following candidate issue reported for pull request #<PR_NUMBER>:

Issue: <ISSUE_DESCRIPTION>
Location: <FILE_AND_LINE>
Flagged by: <AGENT_TYPE_AND_REASON>

Use this rubric verbatim:
- 0: Not confident at all. This is a false positive that does not stand up to light scrutiny, or is a pre-existing issue.
- 25: Somewhat confident. This might be a real issue, but may also be a false positive. Not verified as a real issue. If stylistic, not called out in CLAUDE.md.
- 50: Moderately confident. Verified real issue, but a nitpick or rare in practice. Relative to the rest of the PR, not very important.
- 75: Highly confident. Double-checked and verified as very likely in practice. The approach is insufficient. Directly impacts functionality, or is directly mentioned in CLAUDE.md.
- 100: Absolutely certain. Confirmed as definitely a real issue that will happen frequently in practice. Evidence directly confirms this.

Return the numeric score and a one-paragraph justification.
```

---

### Step 6: Filter Scores

Discard all issues with a score below 80.
If no issues score 80 or above:
- If step 1 allowed proceeding, comment with the "No issues found" template, or conclude according to user instructions.

---

### Step 7: Repeat Eligibility Check

Spawn one subagent to verify the pull request was not closed, merged, or amended while the review was running:

```text
Run `gh pr view <PR_NUMBER> --json state,isDraft,headRefOid`.
Verify the pull request is still OPEN, not a draft, and the headRefOid matches <INITIAL_HEAD_SHA>.
```

---

### Step 8: Post Final Comment

Post the review using the GitHub CLI:

```sh
gh pr comment <PR_NUMBER> --body "<COMMENT_BODY>"
```

**Formatting Checklist:**
1. State the count of issues found.
2. Provide permalinks using the full 40-character commit SHA.
3. Center line ranges with at least one context line before and after (`#L10-L15`).
4. Plain text without decorative emojis.
5. No attribution tags, co-author trailers, or generator links.

---

## Common Anti-Patterns to Avoid

- **Model hunting:** Calling model discovery tools or testing different model identifiers. Use the user's default model for all subagents.
- **Ignoring prior reviews:** Proceeding to review when step 1 condition (d) shows an existing review on the current commit.
- **Running tests or builds:** Attempting to run project test suites or type checkers. CI performs these checks separately.
- **Skipping the todo list:** Beginning work without printing the checklist.
- **Partial git SHAs:** Writing short SHAs or dynamic subshells like `$(git rev-parse HEAD)` in comment links. Always use full 40-character commit hashes.
