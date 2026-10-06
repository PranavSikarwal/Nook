# Phase A implementation plan: Autonomous, self-contained end-to-end distribution

Phase A ensures that Nook installs and runs completely out of the box with zero manual steps, zero sudo requirements, and full automated coordination between the desktop overlay, the background daemon, and the Python worker.

## Architectural context

- **Current state**:
  - The desktop overlay (`nook-panel`), background daemon (`nookd`), and control CLI (`nookctl`) are implemented and passing tests.
  - The packaging workflow in GitHub Actions only compiled `panel/` and did not package `nookd` or `nookctl`.
  - The one-command installer downloaded `Nook.dmg` but lacked `nookd`.
  - The daemon supervisor resolved the Python worker path via a relative directory `./worker`, failing when launched outside the repository root.
- **Target state**:
  - The packaging workflow compiles `nookd` and `nookctl` and embeds them into release assets.
  - The installer sets up all three binaries (`nook-panel`, `nookd`, `nookctl`) and the `nook` launcher script in `~/.local/bin/` with zero sudo.
  - The installer initializes `config.toml` automatically if missing.
  - `nookd` discovers the Python worker across standard bundle, environment, and user paths, invoking the virtual environment Python directly without requiring `uv` in GUI application PATHs.
  - `nook-panel` guarantees daemon startup before connecting on send or retry.

## Phasing and tasks

### A1. Automatic worker resolution in `supervisor.rs`
1. Replace fragile relative-path checks with multi-tier discovery:
   - Explicit `config.worker_command` if present.
   - Environment variables `NOOK_WORKER_COMMAND` and `NOOK_WORKER_DIR`.
   - Executable-relative candidate paths (app bundle `Contents/Resources/worker`, workspace root, etc.).
   - User standard paths (`~/.local/share/nook/worker`, repository checkout paths).
2. Prefer direct virtual environment interpreter invocation (`<worker_dir>/.venv/bin/python -m nook_worker`) when `.venv` exists.
3. Fall back to resolved `uv` binary across standard installation directories (`~/.local/bin/uv`, `~/.cargo/bin/uv`, `/usr/local/bin/uv`).

### A2. Daemon auto-healing in `nook-panel`
1. In `panel/src-tauri/src/lib.rs`, call `ensure_daemon_started` when sending messages if the socket is not yet responsive.
2. Ensure spawned daemon processes receive `PATH` including `~/.local/bin`.

### A3. Complete CI release packaging in `package.yml`
1. Add a step in `.github/workflows/package.yml` on macOS and Ubuntu to build `nookd` and `nookctl` in release mode.
2. Package `nookd` inside the app bundle / distribution.
3. Upload standalone daemon and control binaries (`nookd`, `nookctl`) as GitHub Release assets for each architecture.

### A4. Zero-config installer script (`install.sh`)
1. Download both the desktop overlay and daemon binaries (`nook-panel`, `nookd`, `nookctl`).
2. Place all binaries in `~/.local/bin/` and configure the `nook` launcher.
3. Automatically create a default `config.toml` in `~/Library/Application Support/Nook/` (macOS) or `~/.config/nook/` (Linux) if not present.
4. Ensure zero `sudo` prompts on macOS and unprivileged Linux installs.

### A5. Validation and release
1. Run local test suites (`cargo test`, `cargo clippy`, `smoke.sh`).
2. Test installation from clean environment.
3. Open pull request, verify CI and SonarCloud checks, merge, and publish updated release.
