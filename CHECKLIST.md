# Nook v1 checklist

Task details and "done when" checks are in `docs/plan.md`. Specs are in `docs/spec/`. An item can start once everything after "needs" is checked.

## Phase S: Setup

- [x] S1. Install the toolchain (Rust, Postgres database `nook`). Needs: none
- [x] S2. Create the repo skeleton and first commit. Needs: none
- [x] S3. Run the model check on the real endpoint and record the result. Needs: none
- [x] S4. Add an image step to the model check. Needs: S3

## Phase C: Contracts

- [x] C1. Write the JSON Schemas and examples. Needs: S2
- [x] C2. Add the contract check script. Needs: C1

## Phase W: Worker

- [ ] W1. Worker skeleton with a fake agent. Needs: C1
- [ ] W2. Real agent with streaming. Needs: W1, S3
- [ ] W3. Postgres checkpointer and Memory. Needs: W2, S1
- [ ] W4. Attachments. Needs: W2, S4
- [ ] W5. Summarization at 750k tokens. Needs: W3
- [ ] W6. Titles. Needs: W2
- [ ] W7. Cancel and error mapping. Needs: W2
- [ ] W8. Delete a Chat's Memory. Needs: W3

## Phase D: Daemon

- [ ] D1. Workspace, config, and socket server. Needs: C1, S1
- [ ] D2. Migrations and database layer. Needs: D1
- [ ] D3. Worker supervisor. Needs: D1, W1
- [ ] D4. The `send_message` flow. Needs: D2, D3, W2
- [ ] D5. List, get, and delete. Needs: D4, W8
- [ ] D6. Cancel. Needs: D4, W7
- [ ] D7. Settings and Keychain. Needs: D3
- [ ] D8. `nookctl`. Needs: D4, D5
- [ ] D9. LaunchAgent scripts. Needs: D4

## Phase P: Panel

- [ ] P0. Overlay spike, bundle script, and the answer to open point 3. Needs: none
- [ ] P1. Daemon client and protocol types. Needs: P0, C1
- [ ] P2. Compact and expanded views with streaming. Needs: P1, D4
- [ ] P3. Markdown rendering. Needs: P2
- [ ] P4. History. Needs: P2, D5
- [ ] P5. Attachments. Needs: P2, W4
- [ ] P6. Settings. Needs: P2, D7
- [ ] P7. Errors, Stop, and Retry. Needs: P2, D6

## Phase I: Integration

- [ ] I1. End-to-end smoke test. Needs: D8, W4, W8
- [ ] I2. Install and runbook. Needs: D9, P7, I1
