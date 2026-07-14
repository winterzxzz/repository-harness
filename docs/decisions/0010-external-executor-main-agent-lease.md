# 0010 Main-Agent-Owned External Executor Lease

Date: 2026-07-14

## Status

Accepted

## Context

Claude Code can spawn a subagent only from its main session, so Symphony cannot
launch or supervise that executor like a built-in adapter. Letting the
subagent update root Symphony state would weaken the prepared-worktree
boundary, while PID recovery cannot represent a process Symphony did not
spawn.

## Decision

The main agent owns all external lifecycle commands and maintains a logical
heartbeat lease in Symphony state. The subagent performs implementation only
inside the prepared worktree and reports milestones to the main agent.

External runs are distinguished from managed adapter runs by an additive
`execution_mode` field. Managed runs retain PID recovery. External runs use
heartbeat expiry and are never interrupted merely because the Web controller
PID changed. Prepared runs continue to hold the existing active-run lock.

Artifact completion preserves canonical `RESULT.json` outcomes. A canonical
logical digest of the copied Harness database makes the changeset requirement
decidable when durable Harness state changes.

## Alternatives Considered

1. Let the subagent call lifecycle commands against root state. Rejected
   because it crosses the control-plane boundary.
2. Add an adapter that polls for result artifacts. Rejected because it models
   the wrong executor and relies on implicit process coordination.
3. Add a supervisor daemon. Rejected as unnecessary for a local single-run
   controller when transactional lazy expiry and a Web timer are sufficient.

## Consequences

Positive:

- Worktree isolation remains explicit.
- Managed adapters keep their current ownership and recovery behavior.
- External liveness and lock release become durable and testable.
- CLI, status, events, and Web UI keep one source of truth.

Tradeoffs:

- The main agent must remain responsive enough to refresh the lease.
- State reads that depend on the active run must reconcile expired leases.
- Run state, validation, Web UI, docs, and installer proof change together.

## Follow-Up

- Implement the accepted design in `US-094`.
- Validate one real Claude main-agent/subagent run before marking the story
  implemented.
