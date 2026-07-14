# Exec Plan

## Goal

Make an orchestrator-plus-subagent run a first-class Symphony execution path
without weakening worktree isolation, validation, or single-active-run state.

## Scope

In scope:

- Additive execution-mode and copied-DB-digest state.
- External lifecycle CLI commands.
- Transactional lease expiry and Web timer reconciliation.
- Shared completion validation and canonical result outcomes.
- Executor/progress Web presentation.
- Agent, Quickstart, installer, and validation documentation.

Out of scope:

- Multiple runs, remote workers, queues, or a supervisor daemon.
- Built-in adapter launch changes.
- Direct subagent access to root state.

## Risk Classification

Risk flags:

- Data model.
- Public CLI contracts.
- Existing run and recovery behavior.
- Cross-platform browser and Electron surfaces.
- Validation and changeset durability.

Hard gates:

- Do not weaken artifact or changeset validation.
- Do not let the subagent mutate root control-plane state.
- Do not change managed-adapter PID recovery.
- Do not permit more than one active prepared/running run.

## Work Phases

1. Add failing state migration, transition, expiry-race, and outcome tests.
2. Implement execution mode, external lease reconciliation, and DB digest.
3. Add failing CLI contract and shared-completion integration tests.
4. Implement start, heartbeat, complete, normalized milestone events, and
   changeset enforcement.
5. Add failing board, Web restart, executor badge, and stale presentation
   tests; implement shared UI behavior.
6. Update AGENTS, Quickstart, installer manifest, and fresh-install proof.
7. Run focused, workspace, browser, desktop, payload, and manual validation.
8. Record story verification evidence and Symphony artifacts.

## Stop Conditions

Pause for human confirmation if:

- External execution requires subagent access to root state.
- Safe transactional expiry cannot coexist with heartbeat writes.
- Logical DB digest cannot distinguish durable changes reliably.
- Shared validation or canonical outcomes would need to diverge by executor.
- Managed adapter recovery or single-active-run behavior must materially change.
