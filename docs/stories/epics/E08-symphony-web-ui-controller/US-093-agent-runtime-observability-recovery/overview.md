# US-093 Agent Runtime Observability And Recovery

## Status

planned

## Lane

high-risk

## Current Behavior

Web-started Codex execution still uses `agent_timeout_minutes` as an absolute
deadline even though the controller reports an uncapped runtime. Codex writes
live JSON-RPC events, while OpenCode writes only a raw output artifact that the
Web event endpoint does not expose. Web controller termination can also leave
an unowned active run that blocks all later starts.

The active task lifecycle is derived from broad board states, so successful
runs do not visibly pass through validation and pull-request creation.

## Target Behavior

Codex runs until a terminal event, process exit, explicit cancellation,
protocol stall, or required evidence failure. Codex and OpenCode both emit
normalized sequenced events. Web-started runs persist process ownership,
heartbeat, cancellation, and current lifecycle stage; controller startup
reconciles orphaned runs safely. The task detail can cancel an active run, and
the seven-step lifecycle follows durable stages through Done.

## Affected Users

- Developer operating the local Symphony Web UI.
- Reviewer monitoring agent execution and deciding whether to cancel it.
- Codex and OpenCode adapters producing run evidence.

## Affected Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/SYMPHONY_SCOPE.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/US-065-unlimited-codex-app-server-runtime.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/US-078-opencode-agent-selection.md`
- `docs/stories/US-090-symphony-active-task-flow.md`
- `docs/superpowers/specs/2026-07-14-symphony-agent-runtime-observability-recovery-design.md`
- `docs/decisions/0009-agent-runtime-ownership-and-events.md`

## Non-Goals

- Multiple active runs.
- A separate supervisor daemon.
- WebSocket or Server-Sent Events transport.
- Automatic agent resumption after controller restart.
- Changes to worktree isolation, result validation, PR policy, or sync policy.
- Redesigning unrelated board and review surfaces.

## Acceptance Criteria

- Codex has no fixed wall-clock deadline and retains terminal/stall handling.
- OpenCode and Codex both expose incremental normalized events in task detail.
- Event polling accepts a sequence cursor and returns only newer retained events.
- Web runs persist owner PID, agent PID identity, heartbeat, stage, and cancel state.
- Web startup safely interrupts stale owned runs and releases the active lock.
- The cancel endpoint and confirmed UI action terminate the process tree and
  retain partial artifacts.
- Lifecycle stages visibly progress through Agent, Validation, Pull Request,
  Review, Sync, and Done using authoritative run state.
- Existing raw Codex/OpenCode artifacts and legacy event responses remain
  readable.

