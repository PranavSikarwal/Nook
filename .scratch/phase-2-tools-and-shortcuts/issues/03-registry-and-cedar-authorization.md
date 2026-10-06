# Add the registry and Cedar authorization bridge

Type: task
Status: open
Blocked by: 01

## Goal

Create a Worker registry and a Daemon Cedar authorization interface.

## Work

- Define registered tool metadata and asynchronous executor interfaces.
- Load only the two active web tools.
- Normalize validated requests before authorization.
- Add an in-memory Chat-scoped grant store in the Daemon, binding grants to tool, Chat id, and canonical argument digest.
- Evaluate Cedar before and after an approval grant.
- Ensure the Daemon fails closed if Cedar policy or schema is invalid.
- Block direct executor calls that do not carry a Daemon authorization result.

## Done when

Every registered tool request reaches Cedar. A request without a matching grant
cannot execute when its tier requires approval.
