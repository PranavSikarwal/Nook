# Contributing to Nook

This document outlines the branch, commit, and pull request workflow for Nook.

## Branch strategy

Always work on a dedicated branch. Never commit directly to `main`.

### Branch naming

Branch names follow this pattern:

`<type>/<phase-name>`

Common types:
- `feat/`: new capabilities or phase implementations
- `fix/`: bug fixes
- `docs/`: documentation updates
- `test/`: test additions or test harness updates
- `refactor/`: code refactoring without behavior changes

Phases from `CHECKLIST.md` and `docs/plan.md`:
- Contracts: `feat/phase-c-contracts` (or `feat/c1-json-schemas`)
- Worker: `feat/phase-w-worker`
- Daemon: `feat/phase-d-daemon`
- Panel: `feat/phase-p-panel`
- Integration: `feat/phase-i-integration`

Examples:
- `feat/phase-w-worker`
- `feat/phase-d-daemon`
- `fix/phase-w-stream-reconnect`

## Development workflow

Follow these steps for every change:

1. Update your local `main` branch.
   ```sh
   git checkout main
   git pull origin main
   ```
2. Create your phase branch.
   ```sh
   git checkout -b feat/phase-w-worker
   ```
3. Make your changes and run the relevant tests and linters.
   Before committing anything, verify all linting and type checks pass:
   - Python linting: `uv tool run ruff check .` and `uv tool run ruff format --check .`
   - Python type checking: `uv run --with pyright pyright` (in Python packages) or `uv run --with pyright --with pytest --with jsonschema pyright tests/contracts` (for root tests)
   - Rust linting: `cd daemon && cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
   - Swift tests: `cd app && swift test`
   - Contract tests: `uv run --with pytest --with jsonschema pytest tests/contracts`
4. Stage and commit your changes using concise commit messages.
   ```sh
   git add <files>
   git commit -m "feat(worker): implement agent runner and streaming"
   ```
5. Push the branch to your remote repository.
   ```sh
   git push -u origin feat/phase-w-worker
   ```
6. Open a pull request on GitHub (one pull request per phase).

## Code quality and linting rules

Before committing any change to the repository, you must run the following checks. All checks must pass with zero errors and zero warnings before creating a commit.

### 1. Python linting and formatting (Ruff)

Run Ruff linting and formatting checks across the repository:

```sh
uv tool run ruff check .
uv tool run ruff format --check .
```

To automatically resolve fixable lint issues and format code:

```sh
uv tool run ruff check --fix .
uv tool run ruff format .
```

### 2. Python type checking (Pyright)

Run Pyright type checking across Python packages:

```sh
# For worker:
cd worker && uv run --with pyright pyright

# For model-check:
cd model-check && uv run --with pyright pyright
```

### 3. Rust quality checks (Clippy and rustfmt)

Run Cargo checks in the daemon workspace:

```sh
cd daemon
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

### 4. Contract validation

Run the contract pytest suite whenever message schemas, contracts, or examples are modified:

```sh
uv run --with pytest --with jsonschema pytest tests/contracts
```

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
git config --local credential.helper '!f() { echo username=PranavSikarwal; echo "password=$(gh auth token --user PranavSikarwal)"; }; f'
```

This modifies `.git/config` for Nook only and leaves your global git settings untouched.

### GitHub CLI queries and pull requests

Do not run `gh auth switch`. Running `gh auth switch` alters global configuration in `~/.config/gh/hosts.yml`, which affects other workspaces and repositories.

Instead, run each GitHub CLI command through the repository wrapper script `scripts/gh`:

```sh
scripts/gh pr create ...
scripts/gh pr list
```

This passes the `PranavSikarwal` account token automatically on every call without altering the active account for other tools or projects. Do not run `gh` directly and do not run `gh auth token`.

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
