# Nook implementation plan

Nook has one active implementation plan. The completed plans and retired version
1 specifications are historical records in `docs/archive/`.

## Active work

[Tool-platform closeout and next phase](plans/tool-platform-closeout-and-next-phase.md)
tracks the remaining authorization work and future tool providers.

[Tool platform open gaps](plans/tool-platform-open-gaps.md) lists what is not
done after Stage 2, with the risk and the decision for each item.

### Close the current web-tool authorization path

**TP1. Load and validate policy assets at Daemon startup.**

The Daemon loads YAML and Cedar policy assets before it accepts tool work.

**TP2. Define one normalized authorization request.**

The Worker validates arguments, then sends normalized tool facts to the Daemon.

**TP3. Move grants and decisions to the Daemon.**

The Daemon owns scoped grants and evaluates Cedar before and after approval.

**TP4. Make approval actions policy-driven.**

The Panel receives actions and display metadata that the Daemon loaded from
`policy/tools.yaml`.

**TP5. Complete tool lifecycle events and cancellation.**

Approval, deny, cancellation, and resume behavior stays bound to its original
request.

**TP6. Verify the complete web-tool path.**

Deterministic and packaged desktop tests prove that denied tools do not run and
approved tools resume correctly.

### Extend the tool platform

**TP7. Define the provider and sandbox contract.**

Select and test an isolation implementation before Nook adds privileged tools.

**TP8. Add explicit MCP server registration.**

Start with a configured server. Defer public discovery and installation.

**TP9. Add sandboxed filesystem tools.**

Use structured arguments, configured roots, narrow grants, and escape tests.

**TP10. Add sandboxed terminal tools.**

Use command policy, environment filtering, resource limits, and cancellation.

**TP11. Add browser and native-system capability designs.**

Approve each platform-specific design before implementation.

**TP12. Add editable policy and persistent grants.**

Users can manage validated preferences without bypassing Cedar.

**TP13. Add additional search providers.**

Add image, news, video, and book search one provider type at a time.

## Current documentation

- [Architecture](architecture.md)
- [Contracts](contracts.md)
- [Tools and authorization](tools-and-authorization.md)
- [Release and installation](release.md)
- [Testing](testing.md)
- [Architecture decisions](adr/)
- [Archived records](archive/)
