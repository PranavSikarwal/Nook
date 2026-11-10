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

Do not add author-attribution trailers or generator notices to commits.

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
decision changes. Keep completed plans in `docs/archive/plans/`.

## Pull requests

A pull request states the behavior change and the checks that were run. Update
`docs/plan.md` and `CHECKLIST.md` when the change completes or adds an active
task.

Run GitHub commands through `scripts/gh`. Do not run `gh auth switch`.

## Releases

A release change keeps version metadata aligned, updates `CHANGELOG.md`, and
passes CI, package verification, and the native Tauri test gate. The release
workflow verifies the packaged Daemon and Worker runtime for macOS, Debian, and
Windows artifacts.
