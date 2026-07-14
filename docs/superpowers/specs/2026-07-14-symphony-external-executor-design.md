# Symphony External Executor (Orchestrator + Subagent) Design

## Intent

Let a main agent session orchestrate a Symphony run while a spawned subagent
implements the story inside Symphony's prepared worktree. The external path
must preserve the same isolation, artifact validation, canonical outcomes,
status surfaces, and single-active-run invariant as a built-in adapter run.

Claude Code is the first consumer because it spawns subagents from inside its
main session. Symphony cannot launch that subagent directly, so the main agent
needs a supported way to drive the prepared run lifecycle.

## Current Behavior

- `harness-symphony run <story-id>` prepares a worktree, copied `harness.db`,
  and `RUN_CONTRACT.json`, launches a configured adapter, and owns the run
  lifecycle.
- `harness-symphony run <story-id> --prepare-only` creates the same isolated
  resources without launching an adapter. The resulting `prepared` row already
  holds the single-active-run lock.
- `run_state.agent` records the selected adapter, while managed execution also
  records controller and child-process ownership, heartbeat, lifecycle stage,
  cancellation, and terminal reason.
- Web startup reconciles managed `prepared` and `running` rows through verified
  process identity. That PID policy cannot safely represent an external
  subagent which Symphony did not launch.

## Approved Scope

### In scope

- Three lifecycle subcommands owned by the main agent:
  - `harness-symphony runs start <run_id> --executor <name>`
  - `harness-symphony runs heartbeat <run_id> [--step <text>]`
  - `harness-symphony runs complete <run_id>`
- An `execution_mode` run-state field with `managed` and `external` values.
  The existing `agent` field records the executor name and supplies the Web UI
  executor badge; no duplicate `executor` column is added.
- A logical external lease represented by `execution_mode=external`,
  `status=running`, and `heartbeat_at`.
- Normalized heartbeat events rendered in the existing run-detail progress
  surface.
- Stale-lease reconciliation which releases the lock without requiring a new
  supervisor daemon.
- The same result contract and canonical terminal outcomes used by the adapter
  path: `completed`, `blocked`, `needs_intake`, `partial`, `failed`, and
  `cancelled`.
- Decidable changeset validation based on a canonical logical digest of the
  copied Harness database.
- Agent and operator documentation in `AGENTS.md` and
  `docs/SYMPHONY_QUICKSTART.md`, including fresh-install delivery of the
  Quickstart.

### Out of scope

- Multiple simultaneous active runs.
- Changing Codex, OpenCode, or custom adapter launch behavior.
- Symphony spawning, signalling, or supervising the external subagent.
- Remote execution, queues, scheduling, authentication, or a new Web UI flow.
- Letting the subagent write the root checkout, root `harness.db`, or root
  `.symphony/state.db` directly.

## Roles

| Party | Responsibilities | Explicitly not responsible for |
| --- | --- | --- |
| Main agent | Intake, selection, prepare, start, spawn, periodic heartbeat, milestone forwarding, complete, and human report | Editing story code |
| Subagent | Implementation and verification inside the worktree, milestone reports, result artifacts, and changeset-producing Harness CLI writes | Invoking root lifecycle commands or touching root state |
| Symphony | Isolation, state transitions, lease reconciliation, validation, normalized events, Web UI | Launching or killing the subagent |

The main agent invokes every lifecycle command from the source repository, or
with an explicit `--repo-root` pointing to it. The subagent receives only the
worktree path, the worktree-local contract, and the run environment. This keeps
control-plane state outside the implementation boundary.

## Run Flow

```text
[1] main: harness-symphony run <story-id> --prepare-only
      -> creates worktree, copied harness.db, RUN_CONTRACT.json
      -> records PREPARED and already holds the active-run lock

[2] main: harness-symphony runs start <run_id> --executor claude-subagent
      -> atomically verifies PREPARED + existing lock ownership
      -> records execution_mode=external, agent=claude-subagent
      -> transitions to RUNNING and starts the heartbeat lease

[3] main spawns the subagent in the prepared worktree

[4] subagent reports milestones; main maintains the lease
      -> runs heartbeat <run_id>
      -> runs heartbeat <run_id> --step "<bounded milestone>"

[5] subagent writes SUMMARY.md, RESULT.json, and a changeset when Harness
    durable state changed, then returns its final report

[6] main: harness-symphony runs complete <run_id>
      -> Symphony enters validation stage and validates worktree artifacts
      -> the validated RESULT.json outcome becomes the terminal run status
      -> validation failure records FAILED

[7] main reports review, PR, sync, retry, or intake next steps
```

The main agent sends at least one heartbeat during every 30-second interval
while waiting for the subagent. `--step` is used only when the milestone text
changes; ordinary lease refreshes do not create duplicate progress events.

## State and Ownership Model

### Active lock

`prepared` and `running` remain active statuses. Preparing the run acquires the
lock under the existing invariant. `runs start` does not acquire a second lock;
it atomically verifies that the target prepared run is the active row before
transitioning it to `running`.

### Execution modes

- `managed`: existing adapter path. PID identity, cancellation, and managed
  runtime reconciliation remain unchanged.
- `external`: no Symphony-owned agent PID exists. Liveness comes only from the
  heartbeat lease. PID reconciliation must skip these rows.

Existing rows migrate to `execution_mode=managed`. The existing `agent` field
continues to contain `codex`, `opencode`, `custom`, or an external name such as
`claude-subagent`.

### External lease

The default external heartbeat TTL is 120 seconds. The
`runs.external_heartbeat_ttl_seconds` setting may override it with a positive
value, but the orchestrator contract requires a heartbeat interval no greater
than one quarter of the configured TTL.

Expired external leases are reconciled in two places:

1. Before state reads or writes whose answer depends on the active run,
   including `status`, `runs list/show`, prepare, start, heartbeat, complete,
   board derivation, cleanup, and Web API reads.
2. Every five seconds in the Web server so an open UI reflects expiry without
   another user action.

Reconciliation uses one immediate transaction: re-read a `running external`
row, compare `heartbeat_at + ttl` with the supplied clock, update it to
`stale`, record the terminal reason, and release active ownership. A concurrent
heartbeat either wins before the expiry check or is rejected after `stale`;
it can never resurrect the row.

### State transitions

```text
prepared --runs start--------------------------> running
running  --heartbeat---------------------------> running
running  --complete + valid completed result---> completed
running  --complete + valid blocked result-----> blocked
running  --complete + other valid outcome------> matching canonical outcome
running  --complete + validation error----------> failed
running  --heartbeat TTL exceeded---------------> stale
stale    --complete + valid result--------------> matching canonical outcome
stale    --complete + validation error----------> failed
```

`runs start` accepts only the active `prepared` row with an existing worktree.
Heartbeat accepts only `running external`. Complete accepts `running external`
or `stale external`. Managed runs and never-started prepared runs are rejected.
Completing a stale run does not inspect or mutate the active lock held by a
newer run.

The board treats `stale` as Needs Attention. Retention and cleanup treat it as
a failed-worktree class, preserving it under the configured failed-worktree
policy.

## Lifecycle Command Contract

All three commands resolve Symphony state from the source repository. The
main agent must run them from that repository or pass the global
`--repo-root <source-repo>` option.

### Start

`runs start` atomically checks run existence, `prepared` status, active-row
identity, worktree presence, and external executor name. It then records
`execution_mode=external`, stores the name in `agent`, sets stage `agent`, and
sets the initial heartbeat timestamp.

Repeated start is rejected rather than silently changing executor identity.

### Heartbeat

Heartbeat updates only `heartbeat_at` for a `running external` row. When
`--step` is present, Symphony validates a bounded non-empty string and appends
a normalized event to the existing `RUN_EVENTS.jsonl` stream. Arbitrary step
text does not overwrite the canonical `current_stage` field.

### Complete

Complete sets stage `validation`, reconstructs `PreparedRun` from durable run
state, and calls the shared adapter-path artifact validator. It promotes the
same artifacts and persists the validated result outcome. Missing or invalid
artifacts record `failed` with the validation error while preserving the
worktree.

## Harness Database and Changeset Validation

At prepare time Symphony calculates and stores a canonical logical digest of
the copied `harness.db`. The digest covers schema and ordered durable table
content while excluding SQLite implementation metadata. It is stable across
read-only opens, checkpoints, and equivalent row ordering.

At complete time Symphony calculates the digest again:

- Unchanged digest: a changeset is optional.
- Changed digest: `.harness/changesets/<run_id>.changeset.jsonl` is required,
  must have the matching header/run ID, must contain semantic operations, and
  must pass the existing changeset parser.

This catches a copied-database mutation made without the required run
environment. Direct raw SQLite mutation remains unsupported; the agent
contract requires Harness CLI writes so changeset operations are produced
transactionally.

## Web UI and Events

The run card and detail use the existing `agent` value as an Executor badge.
External progress reuses normalized events and the current lifecycle stage.
No second status file, endpoint family, board bucket, or event format is
introduced.

Web startup first expires external leases, then applies verified PID recovery
only to managed runs. It must never interrupt an external run solely because
the Web controller PID changed.

## Error Handling

- Subagent dies: the main agent stops observing progress and stops heartbeat;
  the lease becomes stale and releases the lock.
- Main agent ends: heartbeat stops even if the subagent remains alive, because
  Symphony has lost its orchestrator. The next state access or Web timer marks
  the run stale.
- Web server restarts: managed rows use PID recovery; live external rows remain
  running until their lease actually expires.
- Artifacts are invalid: complete records failed and preserves evidence.
- Late result arrives after stale: complete validates it without disturbing a
  newer active run.
- Copied Harness DB changed without a valid changeset: complete records failed.
- Main agent cannot maintain the required heartbeat cadence: it must not start
  the external run.

## Alternatives Considered

### Main-agent-owned external lease

Selected. It preserves the worktree boundary, avoids giving the subagent
control-plane write access, and requires no new daemon.

### Subagent invokes root lifecycle commands

Rejected. It requires exposing root state paths and granting implementation
workers control-plane mutation rights, contradicting the isolation role.

### Wait-for-artifact adapter shim

Rejected. It models the wrong executor, relies on implicit polling, and still
needs separate failure and liveness conventions.

### External status file rendered by the Web UI

Rejected. It creates a second source of truth and bypasses shared validation.

## Documentation and Fresh Install

- `AGENTS.md` describes the main-agent lifecycle and prohibits the subagent
  from invoking root lifecycle commands.
- `docs/SYMPHONY_QUICKSTART.md` documents exact external-run commands,
  heartbeat cadence, recovery, and late completion.
- `docs/SYMPHONY_QUICKSTART.md` is added to
  `scripts/harness-install-files.txt`.
- `scripts/validate-install-payload.sh` proves a fresh install and refreshed
  agent shim contain the external-executor guidance without source-only run
  history.

## Validation

- Unit: every transition and rejection above, external TTL boundaries,
  heartbeat/expiry race, managed-versus-external reconciliation, canonical
  outcomes, stable logical DB digest, and changeset requirement.
- Integration: prepare, start, periodic heartbeat, normalized step event,
  valid completion for every outcome class, validation failure, stale release,
  late completion while a newer run is active, and copied-DB mutation without
  changeset.
- Web: executor badge, external heartbeat event, stale Needs Attention state,
  and Web restart preserving a live external run.
- Fresh install: Quickstart and agent guidance are present in the installer
  payload on Bash and PowerShell paths.
- Manual: one real Claude subagent run, observed through CLI and Web UI from
  prepare through review-ready completion.

## Implementation Boundary

This design changes public CLI behavior, durable run state, recovery semantics,
validation, Web presentation, and installer payload. It is a high-risk story.
Implementation must stop if it would weaken result validation, worktree
isolation, changeset durability, or the single-active-run invariant.
