# Code review

Provide a code review for the pull request.

## Review steps

1. Check repository guidelines: Read root AGENTS.md, CLAUDE.md, and any relevant markdown documentation in directories modified by the pull request.
2. Summarize changes: Inspect the diff and list the modified areas.
3. Multi-angle code audit:
   - Audit changes to ensure they comply with repository instructions (AGENTS.md and CLAUDE.md).
   - Read the file changes and do a scan for obvious bugs, edge cases, and unexpected side effects.
   - Inspect git context and history for the modified lines.
   - Verify that changes comply with existing inline comments in the modified files.
4. Confidence scoring:
   - For every flagged issue, assign a confidence score from 0 to 100:
     - 0: False positive or pre-existing issue.
     - 25: Somewhat confident. Might be a real issue, but unverified.
     - 50: Moderately confident. Verified issue, but low impact or nitpick.
     - 75: Highly confident. Real issue directly impacting functionality or breaking project rules.
     - 100: Absolutely certain. Critical bug verified by evidence.
   - Filter out any issues with a score less than 80.
5. Format findings:
   - Keep comments brief.
   - Avoid emojis.
   - Cite specific file paths and line ranges.

## Output format

If issues were found:

### Code review

Found X issues:

1. <brief description of bug or violation> (Reference: <reason/file>)
   - File: `path/to/file` (lines L-L)
   - Explanation and suggested resolution.

2. ...

If no issues were found:

### Code review

No issues found. Checked for bugs and repository guideline compliance.
