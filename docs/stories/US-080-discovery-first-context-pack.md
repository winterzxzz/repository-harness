# US-080 Discovery-First Context Pack

## Status

implemented

## Lane

normal

## Product Contract

`harness-cli context --story <id>` should help agents start exploration without
turning Harness docs into a rigid reading checklist. When a story markdown file
exists but the durable story row is missing, the command should still return a
useful pack instead of failing.

## Relevant Product Docs

- `docs/CONTEXT_RULES.md`
- `docs/HARNESS.md`
- `docs/TOOL_REGISTRY.md`

## Acceptance Criteria

- Story context falls back to `docs/stories/*.md` when the SQLite story row is
  missing.
- Context pack copy says "Recommended Context" rather than "Required Context".
- The paste prompt tells the agent to start from recommended context and then
  follow imports, search results, and nearby tests.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | `cargo test -p harness-cli context_pack -- --nocapture` |
| Integration | `cargo run -p harness-cli -- context --story US-080` |
| E2E | Not needed; CLI behavior only |
| Platform | `cargo fmt --check`; `cargo clippy --workspace -- -D warnings`; `git diff --check` |

## Evidence

- `cargo test -p harness-cli context_pack -- --nocapture`
- `cargo run -p harness-cli -- context --story US-080`
- `cargo run -p harness-cli -- context --story US-079`
- `cargo fmt -p harness-cli --check`
- `cargo clippy -p harness-cli -- -D warnings`
- `git diff --check`
- `scripts/bin/harness-cli story verify US-080`
