# Contributing to Nook

This document outlines the branch, commit, and pull request workflow for Nook.

## Branch strategy

Always work on a dedicated branch. Never commit directly to `main`.

### Branch naming

Branch names follow this pattern:

`<type>/<task-id>-<description>`

Common types:
- `feat/`: new capabilities or spec implementations
- `fix/`: bug fixes
- `docs/`: documentation updates
- `test/`: test additions or test harness updates
- `refactor/`: code refactoring without behavior changes

In standard GitHub workflows, a task ID refers to a work item or ticket identifier. For this project specifically, the task ID is the phase letter plus the step number from `CHECKLIST.md` and `docs/plan.md`:
- Setup: `s1` to `s4`
- Contracts: `c1`, `c2`
- Worker: `w1` to `w8`
- Daemon: `d1` to `d9`
- Panel: `p0` to `p7`
- Integration: `i1`, `i2`

Examples:
- `feat/c1-json-schemas`
- `feat/w1-worker-skeleton`
- `fix/d3-worker-restart-delay`

## Development workflow

Follow these steps for every change:

1. Update your local `main` branch.
   ```sh
   git checkout main
   git pull origin main
   ```
2. Create your task branch.
   ```sh
   git checkout -b feat/c1-json-schemas
   ```
3. Make your changes and run the relevant tests.
   - For Worker: `cd worker && uv run pytest`
   - For Daemon: `cd daemon && cargo test`
   - For Panel: `cd app && swift test`
   - For contracts: `uv run scripts/check-contracts`
4. Stage and commit your changes using concise commit messages.
   ```sh
   git add <files>
   git commit -m "feat(contracts): add panel-daemon and daemon-worker schemas"
   ```
5. Push the branch to your remote fork or repository.
   ```sh
   git push -u origin feat/c1-json-schemas
   ```
6. Open a pull request on GitHub.

## Commit message format

Commit messages follow conventional formatting:

`<type>(<scope>): <subject>`

Allowed types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `style`.

Examples:
- `feat(contracts): add json schema files and examples`
- `test(worker): add fake model streaming test`
- `fix(daemon): handle worker exit during stream`

Do not include author attribution trailers, co-author lines, or generator notices in commit messages.

## GitHub account and commit identity

If you maintain multiple GitHub accounts on your machine, keep operations in this repository isolated so other workspaces remain unaffected. Follow the profile and GitHub CLI rules documented in `AGENTS.md`.

### Commit author identity

Configure the local git identity for this repository so commits use your personal account rather than your global work account:

```sh
git config --local user.name "Pranav Sikarwal"
git config --local user.email "<your-personal-email>"
```

This modifies `.git/config` for Nook only and leaves your global git settings untouched.

### GitHub CLI queries and pull requests

Do not run `gh auth switch`. Running `gh auth switch` alters global configuration in `~/.config/gh/hosts.yml`, which affects other workspaces and repositories.

Instead, route each `gh` command through the `PranavSikarwal` account by supplying its token directly:

```sh
GH_TOKEN=$(gh auth token --user PranavSikarwal) gh pr create ...
GH_TOKEN=$(gh auth token --user PranavSikarwal) gh pr list
```

This targets the `PranavSikarwal` profile on every call without altering the active account for other tools or projects.

## Stacked pull requests

Many tasks in Nook depend on earlier tasks across phases. For example, Daemon task D4 depends on D2, D3, and Worker task W2.

When building a task that depends on unmerged work:

1. Branch from the parent feature branch instead of `main`.
   ```sh
   git checkout feat/w1-worker-skeleton
   git checkout -b feat/w2-real-agent
   ```
2. Implement your changes on the child branch.
3. Open a pull request on GitHub with the base branch set to the parent branch (`feat/w1-worker-skeleton`).
4. Once the parent pull request merges into `main`, change the base branch of the child pull request to `main` on GitHub, or rebase on `main`.
   ```sh
   git checkout feat/w2-real-agent
   git rebase origin/main
   git push --force-with-lease origin feat/w2-real-agent
   ```

## Pull request guidelines

Each pull request should contain:

1. A clear title matching the commit format.
2. A summary describing what changed.
3. A test plan with the commands you ran and the results.
4. References to the task ID in `CHECKLIST.md`.
