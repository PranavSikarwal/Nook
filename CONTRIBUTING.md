# Contributing to Nook

Use a dedicated branch for each change. Keep a change focused, run the relevant
checks, and open a pull request before merging.

## Branches and commits

Use a conventional branch prefix such as `feat/`, `fix/`, `docs/`, `test/`, or
`refactor/`. Name the rest of the branch after the capability being changed.

Use conventional commit messages:

```text
<type>(<scope>): <subject>
```

Do not add AI attribution to a commit, a pull request title or body, a comment or a review.

## Development checks

Run the component checks that match the files you changed.

```sh
cd daemon && rtk cargo test
cd daemon && rtk cargo clippy --workspace --all-targets -- -D warnings
cd worker && rtk uv run pytest
cd panel && rtk npm run build
rtk uv run --with pytest --with jsonschema pytest tests/contracts
```

`docs/testing.md` describes native desktop and package checks.

## Documentation

Update current documentation in the same change as a behavior change. Keep
`README.md`, `docs/architecture.md`, `docs/contracts.md`,
`docs/tools-and-authorization.md`, `docs/release.md`, and `docs/testing.md`
consistent with the implementation.

Do not edit accepted ADRs to change history. Create a new ADR when an accepted
decision changes. Task folders stay in `docs/tasks/` after the merge. Completed plans from `docs/plans/` still move to `docs/archive/plans/`.

## Pull requests

A pull request states the behavior change and the checks that were run. Follow
`.agents/rules/sdlc-work-standard.md`. Update `docs/plan.md` and `CHECKLIST.md`
only for work that started before PR 38.

Run GitHub commands through `scripts/gh`. Do not run `gh auth switch`.

## Releases

A release change keeps version metadata aligned, updates `CHANGELOG.md`, and
passes CI, package verification, and the native Tauri test gate. The release
workflow verifies the packaged Daemon and Worker runtime for macOS, Debian, and
Windows artifacts. A pull request that completes a release bumps the application
version and moves "Unreleased" in `CHANGELOG.md` under the new version before it
merges. After the merge, run the Package and Release workflow by hand on `main`
at the merge commit. Create the tag only when the package run passes on macOS,
Ubuntu and Windows.
