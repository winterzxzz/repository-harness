# Symphony External Executor (Orchestrator + Subagent) Design

## Intent

Let a main agent session act as the orchestrator for Symphony runs while a
spawned subagent performs the implementation, without losing Symphony's
isolation, validation, or status flow. A run executed by a subagent must appear
in `harness-symphony status`, `runs list`, and the Web UI exactly like a run
executed by a built-in adapter.

The concrete gap today: Claude Code spawns its subagents from inside the main
session via its Agent tool, so Symphony cannot launch them the way it launches
`codex exec` or `opencode run`. Runs executed this way are invisible to
Symphony unless the run lifecycle can be driven from outside.

## Current Behavior

- `harness-symphony run <story-id>` prepares an isolated worktree, a copied
  `harness.db`, and `RUN_CONTRACT.json`, then launches a configured adapter
  (Codex or OpenCode) and tracks the run itself.
- `harness-symphony run <story-id> --prepare-only` creates the same workspace
  and contract but launches nothing. The run stays `prepared` and there is no
  supported way for an external process to move it to `running` or to have its
  result validated and recorded.
- A single active-run lock in `.symphony/state.db` allows one running run at a
  time. The Web UI renders that one flow.

## Approved Scope

### In scope

- Three new run lifecycle subcommands so an external executor can drive a
  prepared run through the normal status flow:
  - `harness-symphony runs start <run_id> --executor <name>`
  - `harness-symphony runs heartbeat <run_id> --step <text>`
  - `harness-symphony runs complete <run_id>`
- A recorded `executor` field on the run (for example `claude-subagent`,
  `codex`, `opencode`) shown as a badge in the Web UI run card and run detail.
- Heartbeat steps rendered in the existing run detail progress surface.
- A heartbeat TTL: an externally executed run whose heartbeat goes silent past
  the TTL transitions to `stale` instead of holding `running` and the
  active-run lock forever.
- `runs complete` performs the same artifact validation as the adapter path:
  `SUMMARY.md`, `RESULT.json`, and, when durable records were written, the
  `.harness/changesets/<run_id>.changeset.jsonl` changeset.
- Documentation: the orchestrator flow in `docs/SYMPHONY_QUICKSTART.md` and the
  agent-facing instructions in `AGENTS.md` (main agent orchestrates; subagent
  executes inside the prepared worktree).

### Out of scope

- Multiple simultaneous runs. The single active-run lock stays; the
  orchestrator runs stories sequentially.
- Any change to the Codex and OpenCode adapter paths. Those already implement
  the orchestrate-then-execute pattern with Symphony as the launcher.
- Spawning or supervising the subagent process from Symphony. The main agent
  owns the subagent's lifecycle.
- A new Web UI flow or board layout. Externally executed runs reuse the
  existing single-run surfaces.
- Remote execution, queues, or scheduling.

## Roles

| Party | Responsibilities | Explicitly not responsible for |
| --- | --- | --- |
| Main agent | Intake, story selection, `run --prepare-only`, `runs start`, spawning the subagent, `runs complete`, reporting back to the human | Editing story code itself |
| Subagent | All implementation inside the worktree, heartbeats at major milestones, producing `SUMMARY.md`, `RESULT.json`, and the changeset | Touching the root repo or root `harness.db` |
| Symphony | Workspace isolation, run state, validation, Web UI | Launching the subagent |

## Run Flow

```text
[1] main agent: harness-symphony run <story-id> --prepare-only
      -> worktree, copied harness.db, RUN_CONTRACT.json
      -> run visible in UI as PREPARED

[2] main agent: harness-symphony runs start <run_id> --executor claude-subagent
      -> run becomes RUNNING, takes the active-run lock
      -> UI shows the run with an executor badge

[3] main agent spawns the subagent with the worktree path and contract path

[4] subagent works inside the worktree
      -> harness-symphony runs heartbeat <run_id> --step "<milestone>"
      -> produces SUMMARY.md, RESULT.json, changeset when applicable

[5] subagent returns its final report to the main agent

[6] main agent: harness-symphony runs complete <run_id>
      -> Symphony validates artifacts exactly like the adapter path
      -> run becomes DONE or FAILED, lock released

[7] main agent summarizes the outcome and next steps (review, PR, sync)
```

The executor is matched to the main agent family. A Claude Code main agent
spawns a Claude subagent through this external path. Codex and OpenCode main
agents keep using the existing full `run` command, where Symphony launches the
headless instance; their runs already flow through the same status surface.

## State Transitions

```text
prepared --runs start--> running
running  --heartbeat----> running (updates current step + heartbeat timestamp)
running  --runs complete + validation pass--> done
running  --runs complete + validation fail--> failed
running  --heartbeat TTL exceeded--> stale (lock released)
stale    --runs complete--> done or failed (late results still validated)
```

`runs start` is rejected when another run holds the active-run lock, when the
run is not in `prepared`, or when the run's worktree is missing. `runs
heartbeat` and `runs complete` are rejected for run ids that were never
started. `runs complete` on a run with missing required artifacts records the
run as `failed` with a validation error, mirroring adapter behavior.

The TTL default follows the heartbeat conventions introduced by the agent
runtime observability and recovery design (2026-07-14); external runs reuse the
same durable heartbeat storage rather than adding a second mechanism.

## Approaches Considered

### Lifecycle subcommands for external executors

Selected. The external executor becomes a first-class Symphony concept. State
lives in Symphony's own store, so the Web UI, `status`, and `runs list` need no
new data source, and any future executor (another agent CLI, a human) reuses
the same commands.

### Wait-for-artifact adapter shim

Rejected. Registering a fake adapter whose command polls for `RESULT.json`
keeps Symphony unchanged but couples two processes through an implicit
convention, hangs when the subagent dies silently, and misrepresents who
executed the run.

### External status file read by the Web UI

Rejected. Having the subagent write its own status file that the UI learns to
render creates a second source of truth for run state and bypasses validation.

## Error Handling

- Subagent dies without completing: heartbeats stop, the run goes `stale`
  after the TTL, and the lock is released. The main agent sees the missing
  final report, inspects the run directory, and either re-runs or reports.
- Main agent session ends between `start` and `complete`: same TTL path; a new
  session finds the `stale` run via `runs list` and can complete or discard it.
- Subagent finishes but artifacts are invalid: `runs complete` marks the run
  `failed` with the validation error; the worktree is preserved for
  inspection, matching adapter behavior.
- `runs start` raced by a Web-started run: the active-run lock decides; the
  loser gets a clear error and the run stays `prepared`.

## Validation

- Unit: state-transition table above, including rejection cases and TTL
  expiry.
- Integration: full external run against a fixture story — prepare, start,
  heartbeats, artifact creation, complete, validation pass and fail variants.
- Manual: one real story run with a Claude subagent, confirming the run and
  its heartbeat steps render in the Web UI with the executor badge.
