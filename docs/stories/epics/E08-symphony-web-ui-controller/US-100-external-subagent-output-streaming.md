# US-100 External Subagent Output Streaming With Winter Executor Names

## Status

implemented

## Lane

normal

## Product Contract

An external-executor run streams its subagent's live output into the run's
event stream so the Web UI In Progress task console shows the work as it
happens, and external subagents receive stable human-readable names
(`Winter1`–`Winter5`) that the Web UI displays as the executor.

## Relevant Product Docs

- `docs/SYMPHONY_QUICKSTART.md` (External Executor Lifecycle)

## Acceptance Criteria

- `harness-symphony runs output <run_id>` reads stdin and appends each
  non-empty line as an `output` RunEvent attributed to the run's executor
  name, so the existing `/api/runs/<id>/events` polling and RunConsole render
  it live.
- `runs output` refreshes the external lease while streaming, so a chatty
  subagent does not need a parallel heartbeat loop; it rejects runs that are
  not running external runs.
- `runs start <run_id>` no longer requires `--executor`: when omitted,
  Symphony assigns the next name in the `Winter1`–`Winter5` rotation (based on
  the most recent WinterN assignment, wrapping after Winter5; Symphony's
  single-active-run lock means the names identify successive subagent runs,
  not concurrent ones). An explicit `--executor` still works unchanged.
- Existing event compaction, lease TTL, and stale-run reconciliation behavior
  is unchanged.

## Design Notes

- Commands: `runs output <run_id>` (stdin streaming), `runs start [--executor]`.
- Queries: `RunStateStore::list_runs` filtered to running external runs for
  Winter name assignment.
- API: no new HTTP endpoints; reuses `RUN_EVENTS.jsonl` + `events_response`.
- Domain rules: output events bounded per line (reuse 64/200-char style caps
  with a larger output bound); lease refresh piggybacks on stream activity.
- UI surfaces: existing RunConsole in run detail; event `agent` field carries
  the Winter name.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-100 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Winter rotation picks next name and wraps after Winter5; output line truncation bounds. |
| Integration | `runs output` appends output events readable via `read_events_after` and refreshes the lease; non-external runs rejected. |
| E2E | Manual: pipe a subagent transcript into `runs output` and observe live console in Web UI. |
| Platform | n/a |
| Release | `cargo fmt --check`, `cargo clippy`, `cargo test -p harness-symphony`. |

## Harness Delta

Documents the previously missing streaming surface for external subagent
output noted during intake; no harness process change required.

## Evidence

- `cargo test -p harness-symphony` — 266 passed (2026-07-15), including
  `external::tests::output_streams_lines_as_events_and_refreshes_lease`,
  `output_rejects_runs_that_are_not_running_external`,
  `output_truncates_oversized_lines`, `start_rotates_winter_names_across_runs`,
  `start_wraps_winter_rotation_after_winter5`, `start_keeps_explicit_executor_name`.
- Binary smoke (2026-07-15): seeded prepared run, `runs start run_smoke`
  auto-assigned `Winter1`; `printf 'a\nb\nc' | runs output run_smoke` produced
  sequenced `output` events with `"agent":"Winter1"` in
  `.harness/runs/run_smoke/RUN_EVENTS.jsonl` and refreshed `heartbeat_at`.
- `cargo fmt --all` and `cargo clippy -p harness-symphony --all-targets` clean.
