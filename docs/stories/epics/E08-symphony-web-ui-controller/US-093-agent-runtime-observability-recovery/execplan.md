# Exec Plan

## Goal

Provide uncapped and cancellable Codex execution, live normalized monitoring
for Codex and OpenCode, safe Web-run recovery, and authoritative lifecycle
stages without changing Symphony's single-run isolation model.

## Scope

In scope:

- Additive run-state ownership/control/stage fields.
- Normalized cursor-based run events.
- Uncapped Codex and timed OpenCode/custom execution.
- Cancel API/UI and startup orphan reconciliation.
- Lifecycle stage integration and compatibility fallbacks.
- Rust, browser, desktop, workspace, and documentation proof.

Out of scope:

- Multiple active runs or remote execution.
- Separate supervisor service or push transport.
- Agent resumption after restart.
- Changes to result validation, PR, or sync policy.

## Risk Classification

Risk flags:

- Data model: additive SQLite migration.
- Public contracts: event cursor and cancel APIs.
- Existing behavior: agent timeout and lifecycle transitions change.
- Cross-platform: process ownership and termination.
- External systems: Codex and OpenCode CLI process protocols.

Hard gates:

- Never signal an unverified reused PID.
- Never weaken result validation or worktree isolation.
- Preserve raw evidence and backward-compatible event reads.

## Work Phases

1. Add failing state migration and runtime-control tests.
2. Implement durable ownership, heartbeat, cancellation, and stage state.
3. Add failing Codex uncapped/cancel tests and implement runtime control.
4. Add failing normalized event/OpenCode tests and implement the shared writer.
5. Add failing startup reconciliation and API tests, then implement handlers.
6. Add failing lifecycle derivation and browser tests, then implement UI changes.
7. Update product docs and run complete verification.
8. Record durable evidence, trace, and Symphony result artifacts.

## Stop Conditions

Pause for human confirmation if:

- Safe process identity cannot be established on a supported platform.
- A separate supervisor process becomes necessary.
- Backward compatibility requires destructive migration or artifact deletion.
- Validation, worktree isolation, or single-active-run enforcement would need
  to be weakened.
- The approved API shape or lifecycle semantics must materially change.

