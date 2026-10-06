# Define tool approval contracts and policy assets

Type: task
Status: open
Blocked by: none

## Goal

Add approval request and decision messages to both protocols. Create strict YAML
metadata, Cedar schema, and Cedar policy files for the two web tools.

## Work

- Extend JSON Schemas, examples, Rust types, Python models, and TypeScript types.
- Define `approval_requested` and `approval_decision` payloads.
- Add `policy/tools.yaml` with exact action mappings.
- Add Cedar policy and schema files.
- Add tests that reject invalid policy metadata and contract data.

## Done when

Every layer decodes all approval messages. Contract tests cover valid and invalid
examples. Cedar and YAML validation fail before the Worker accepts requests.
