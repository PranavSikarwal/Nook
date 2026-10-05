# Nook v1 checklist

Task details and "done when" checks are in `docs/plan.md`. Specs are in `docs/spec/`. An item can start once everything after "needs" is checked.

## Phase S: Setup

- [x] S1. Install the toolchain (Rust, Postgres database `nook`). Needs: none
- [x] S2. Create the repo skeleton and first commit. Needs: none
- [x] S3. Run the model check on the real endpoint and record the result. Needs: none
- [x] S4. Add an image step to the model check. Needs: S3

## Phase C: Contracts

- [x] C1. Write the JSON Schemas and examples. Needs: S2
- [x] C2. Add contract tests in tests/contracts. Needs: C1

## Phase W: Worker

- [x] W1. Worker skeleton with a fake agent. Needs: C1
- [x] W2. Real agent with streaming. Needs: W1, S3
- [x] W3. Postgres checkpointer and Memory. Needs: W2, S1
- [x] W4. Attachments. Needs: W2, S4
- [x] W5. Summarization at 750k tokens. Needs: W3
- [x] W6. Titles. Needs: W2
- [x] W7. Cancel and error mapping. Needs: W2
- [x] W8. Delete a Chat's Memory. Needs: W3

## Phase D: Daemon

- [x] D1. Workspace, config, and socket server. Needs: C1, S1
- [x] D2. Migrations and database layer. Needs: D1
- [x] D3. Worker supervisor. Needs: D1, W1
- [x] D4. The `send_message` flow. Needs: D2, D3, W2
- [x] D5. List, get, and delete. Needs: D4, W8
- [x] D6. Cancel. Needs: D4, W7
- [x] D7. Settings and Keychain. Needs: D3
- [x] D8. `nookctl`. Needs: D4, D5
- [x] D9. LaunchAgent scripts. Needs: D4

## Phase P: Panel

- [x] P0. Overlay spike, bundle script, and the answer to open point 3. Needs: none
- [x] P1. Daemon client and protocol types. Needs: P0, C1
- [x] P2. Compact and expanded views with streaming. Needs: P1, D4
- [x] P3. Markdown rendering. Needs: P2
- [x] P4. History. Needs: P2, D5
- [x] P5. Attachments. Needs: P2, W4
- [x] P6. Settings. Needs: P2, D7
- [x] P7. Errors, Stop, and Retry. Needs: P2, D6

## Phase I: Integration

- [x] I1. End-to-end smoke test. Needs: D8, W4, W8
- [x] I2. Install and runbook. Needs: D9, P7, I1

## Phase T: Tauri cross-platform migration

- [ ] T1. Tauri v2 scaffolding and window management (draggable, persistent, global hotkey). Needs: I2
- [ ] T2. UI design replication (compact input, expanded transcript, markdown, history, settings). Needs: T1
- [ ] T3. Daemon client and contract integration. Needs: T2
- [ ] T4. Packaging and multi-platform distribution (macOS .dmg, Ubuntu .deb/.AppImage, Windows .msi). Needs: T3
