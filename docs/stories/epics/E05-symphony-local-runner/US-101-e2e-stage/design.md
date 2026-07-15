# Design

## Domain Model

- `Story` gains an optional `e2e_command: TEXT` — the exact shell command that
  proves the story end-to-end (for the target project, e.g. `npm run e2e`).
- `RunContract` gains an optional `e2e_command` field captured at prepare
  time, so managed and external runs finalize identically without re-reading
  the database.
- Business rule: a run may only reach `validation` after its E2E command has
  passed or been cleanly skipped (no command declared).

## Application Flow

1. `prepare_run` / `prepare_here_run`: read `e2e_command` from the story row
   in `harness.db`; embed it in `RUN_CONTRACT.json` (root and worktree copy).
2. `finalize_prepared_run` (shared by managed runs and `runs complete`):
   - `set_stage(run_id, "e2e")`.
   - No command in the contract → append run event
     `lifecycle/e2e: "e2e skipped: story declares no e2e command"`, continue.
   - Command present → spawn via the project shell (`sh -c` / PowerShell on
     Windows) with cwd = worktree, stdout/stderr drained into
     `RUN_EVENTS.jsonl` as `output` events with stage `e2e` (reuse the
     US-100 drain), bounded by `e2e.timeout_minutes`.
   - Exit 0 → event `lifecycle/e2e: "e2e passed"`, proceed to the existing
     validation logic. Non-zero exit or timeout → `finish_execution(failed,
     "inspect e2e failure")`, terminal event with the exit status.
3. Existing validation, review, sync stages unchanged.

## Interface Contract

- `harness-cli story add|update --e2e "<command>"` (optional flag; update may
  clear with an empty value rejected, use explicit `--e2e-clear`? No — keep
  minimal: `--e2e` sets, omitted leaves unchanged; clearing is a manual SQL
  concern until needed).
- `RUN_CONTRACT.json`: new optional `"e2e_command": "<string>"` field;
  absent for stories without one. Contract version unchanged (additive).
- Web `GET /api/runs/<id>` / flow payload: stage list becomes
  `["start","agent","e2e","validation","review","sync","done"]`.
- New config key in `.harness/symphony.yml`:
  `e2e.timeout_minutes` (default 15).

## Data Model

- Migration `scripts/schema/009-story-e2e.sql`:
  `ALTER TABLE story ADD COLUMN e2e_command TEXT;`
  Additive, nullable, no backfill, follows the `002-story-verify.sql`
  precedent. Auto-discovered by the installers.

## UI / Platform Impact

- `web.rs`: add `"e2e"` to the durable stage list and flow-step ordering.
- `web-ui/src/features/symphony/task-flow-model.ts`: `headIds` becomes
  `["start","agent","e2e","validation"]`; update model tests.
- CLI: new `--e2e` flag surfaces in `harness-cli story` help.
- Windows: command execution goes through the same shell used by existing
  custom-adapter commands.

## Observability

- Run events: `e2e skipped…`, live `output` events (stage `e2e`),
  `e2e passed` / terminal failure event with exit status.
- Run record: `current_stage = "e2e"` while running; failures set
  `next_action = "inspect e2e failure"`.

## Alternatives Considered

1. Run the E2E command inside the existing `validation` stage — cheaper (no
   web UI change) but hides the step the operator asked to see; rejected.
2. Project-wide E2E command in `symphony.yml` — simpler config but cannot
   vary per story; rejected by the operator (per-story chosen).
3. Trust agent-reported E2E evidence in RESULT.json — keeps the current
   "trust with evidence" model and its blind spot; rejected: the point of the
   stage is independent execution.
