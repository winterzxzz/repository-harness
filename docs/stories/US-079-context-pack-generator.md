# US-079 Context Pack Generator

## Status

implemented

## Lane

normal

## Product Contract

`harness-cli context` generates a paste-ready markdown context pack from
durable Harness story records and compiled context rules.

## Relevant Product Docs

- `docs/CONTEXT_RULES.md`
- `docs/TOOL_REGISTRY.md`
- `docs/HARNESS.md`

## Acceptance Criteria

- `scripts/bin/harness-cli context --story US-XXX` prints story-specific context.
- `scripts/bin/harness-cli context --lane high-risk` prints lane-generic context.
- Context output includes required docs, relevant decisions, validation expectations, and tool availability notes.
- `--out <path>` writes the same markdown to a file.

## Design Notes

- Commands: add top-level `context` command to `harness-cli`.
- Queries: read `story`, optional dependency/hierarchy edges, decisions, and tools from `harness.db`.
- API: stdout by default, file output with `--out`.
- Tables: no schema change.
- Domain rules: compile context rules in Rust, matching the existing `score-context` posture.
- UI surfaces: none.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id <id> --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | `cargo test -p harness-cli context_pack -- --nocapture` |
| Integration | CLI smoke for `context --story`, `context --lane`, and `context --out` |
| E2E | Not needed; no browser/user workflow |
| Platform | `cargo fmt --check`; `cargo clippy --workspace -- -D warnings`; `git diff --check` |
| Release | `scripts/bin/harness-cli story verify US-079` |

## Harness Delta

Harness context guidance becomes an executable CLI surface instead of only a
markdown rule for agents to follow manually.

## Evidence

- `cargo test -p harness-cli context_pack -- --nocapture`
- `cargo test -p harness-cli command_help_documents_lane_values_and_version -- --nocapture`
- `scripts/bin/harness-cli context --story US-079`
- `scripts/bin/harness-cli context --lane high-risk`
- `scripts/bin/harness-cli context --story US-079 --out /tmp/harness-context-pack.md`
- `cargo test --workspace`
- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`
- `git diff --check`
- `scripts/bin/harness-cli story verify US-079`
