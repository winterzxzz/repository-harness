# Validation

## Proof Strategy

The stage must be proven in all four outcomes — skip, pass, fail, timeout —
plus the streaming side-effect and the migration's safety on existing
databases. Rust tests cover runner behavior; web-ui model tests cover the new
flow node; a real-binary smoke proves the wiring end to end.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | run/skip decision from contract; event kinds/stages emitted; timeout mapping to failure; config default 15 min |
| Integration | prepare embeds `e2e_command` into both contract copies; finalize skips cleanly without command; passes on exit 0 and proceeds to validation; fails run with `inspect e2e failure` on non-zero; output lines land in RUN_EVENTS.jsonl with stage `e2e`; migration applies to a pre-009 database preserving rows |
| E2E | binary smoke: story with `--e2e "printf ok"` completes; story with `--e2e "false"` fails at stage e2e; story without command skips |
| Platform | Windows shell invocation compiles and is exercised by CI build (best effort locally) |
| Performance | e2e timeout enforced (fixture sleep > timeout) |
| Logs/Audit | run events contain `e2e skipped`/`e2e passed`/failure event with exit status |

## Fixtures

- Temp story rows with `e2e_command` set to `printf ok`, `false`, and NULL.
- Prepared run worktrees under a temp Symphony config (pattern from
  `external.rs` tests).

## Commands

```text
cargo test -p harness-cli
cargo test -p harness-symphony
npm --prefix crates/harness-symphony/web-ui test
cargo fmt --all --check && cargo clippy --all-targets
```

## Acceptance Evidence

Verified 2026-07-15:

- `cargo test -p harness-cli`: 43 passed (includes migration 009 applied to a
  pre-009 database, schema version 9, story.add/story.update changeset replay
  with `e2e_command`).
- `cargo test -p harness-symphony`: 279 passed, including new tests
  `e2e_stage_skips_cleanly_without_command`,
  `e2e_stage_passes_and_streams_output`,
  `e2e_stage_failure_fails_run_with_exit_status`, and
  `execute_e2e_times_out_on_silent_hang` (timeout enforced on a silent hang,
  not only between output lines).
- `npm --prefix crates/harness-symphony/web-ui test`: 12 passed; production
  build passed.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`, `git diff --check`: clean.
- Real-binary smoke in a scratch repo with a custom fake agent:
  `--e2e-command "echo e2e-ok"` completed with `e2e running`, streamed
  `e2e-ok` output event (stage `e2e`), and `e2e passed`;
  `--e2e-command "echo boom; exit 3"` failed the run with
  `e2e command failed (exit code 3)` and next action `inspect e2e failure`;
  a story without a command completed with
  `e2e skipped: story declares no e2e command`.
- Windows PowerShell invocation compiles behind `cfg(windows)`; not exercised
  locally (macOS).

Interface deviation from design: the CLI flag is `--e2e-command`, not
`--e2e`, because `story update --e2e 0|1` already records the E2E proof
column. Recorded in decision 0011.
