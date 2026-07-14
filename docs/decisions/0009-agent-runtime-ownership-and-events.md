# 0009 Agent Runtime Ownership And Normalized Events

Date: 2026-07-14

## Status

Accepted

## Context

Symphony supports Codex app-server and OpenCode headless execution, but they
currently expose incompatible live evidence. Web-started runs also lack durable
process ownership, making cancellation and controller-crash recovery unsafe.
A documented uncapped Codex contract conflicts with the fixed deadline still
present in the adapter.

## Decision

Keep the local single-run controller and polling transport while adding durable
run ownership/control state and one normalized sequenced event artifact shared
by all adapters. Codex execution has no absolute wall-clock deadline; OpenCode
and custom commands retain the configured timeout. Cancellation is a durable
request consumed by adapter loops. Startup reconciliation interrupts, rather
than resumes, runs owned by a previous controller and signals a recorded child
only after process identity verification.

## Alternatives Considered

1. Expose adapter-specific log endpoints. Rejected because clients would retain
   incompatible status models.
2. Add a supervisor daemon and push event transport. Rejected because service
   lifecycle, IPC, and packaging exceed this local-controller slice.

## Consequences

Positive:

- Codex behavior matches its uncapped product contract.
- Codex and OpenCode share one live monitoring model.
- Operators can cancel work and recover safely after controller crashes.
- Lifecycle stages come from durable runtime state.

Tradeoffs:

- `run_state` gains additive process-lifecycle fields.
- Polling remains near-real-time rather than true push delivery.
- A controller restart interrupts an active agent instead of resuming it.

## Follow-Up

- Reconsider a separate supervisor only if multiple or remote concurrent runs
  become a product requirement.
