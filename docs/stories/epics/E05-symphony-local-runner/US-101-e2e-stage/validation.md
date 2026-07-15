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

Add results after verification.
