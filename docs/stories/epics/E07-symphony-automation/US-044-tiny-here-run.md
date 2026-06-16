# US-044 Tiny-Lane Here Run

## Status

implemented

## Lane

normal

## Product Contract

`harness-symphony run <story-id> --here` may run tiny-lane stories in the
current checkout, but database isolation and result artifacts remain required.

## Relevant Product Docs

- `docs/SYMPHONY_SCOPE.md`
- `docs/FEATURE_INTAKE.md`

## Acceptance Criteria

- `--here` is allowed only for stories whose lane is `tiny`.
- Normal and high-risk stories are refused with a clear message.
- `HARNESS_DB_PATH` points to a copied DB under `.symphony/runs/`.
- Operation log, `RESULT.json`, and `SUMMARY.md` are still required.
- The run is flagged `lightweight` in run state and summary.

## Design Notes

- Command: `harness-symphony run <story-id> --here`.
- Depends on local runner and result validation stories.
- This story is normal because it changes runner behavior, even though it serves
  tiny-lane work.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-044 --unit 1 --integration 1 --e2e 1 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Lane eligibility tests. |
| Integration | Tiny story uses copied DB and writes artifacts. |
| E2E | Normal/high-risk `--here` refusal smoke. |
| Platform | Shell smoke. |
| Release | `cargo test --workspace`; `cargo fmt --check`; `cargo clippy --workspace -- -D warnings`. |

## Harness Delta

Keeps Symphony usable for tiny work without weakening isolation guarantees.

## Evidence

- Implemented `harness-symphony run <story-id> --here` for tiny-lane stories.
- Added lightweight run state and CLI output, copied DB isolation under
  `.symphony/runs/<run-id>/harness.db`, and summary marking with
  `lightweight: true`.
- Normal-lane `--here` smoke refused with:
  `story US-NORMAL cannot use --here because lane is normal; only tiny stories may run in the current checkout`.
- Validation passed: `cargo test -p harness-symphony`, `cargo test --workspace`,
  `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and temp-repo
  CLI smoke for tiny success plus normal refusal.
