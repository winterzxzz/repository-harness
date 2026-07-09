# US-081 Context Diet

## Status

implemented

## Lane

normal

## Product Contract

Harness should provide a small starting index for agents instead of a broad
required-reading list. Normal-lane context should start with entrypoint,
feature intake, and proof matrix, then let agents expand through imports,
search results, risk triggers, stories, and nearby tests.

## Relevant Product Docs

- `AGENTS.md`
- `docs/CONTEXT_RULES.md`
- `docs/FEATURE_INTAKE.md`

## Acceptance Criteria

- `AGENTS.md` no longer tells agents to read the full Harness document set
  before every task.
- `docs/CONTEXT_RULES.md` uses start, expand, and skip language instead of
  default required-reading tables.
- Normal-lane context packs no longer include `README.md`, `docs/HARNESS.md`,
  `docs/CONTEXT_RULES.md`, `docs/TOOL_REGISTRY.md`, or story templates by
  default.
- Story-specific context still includes product docs, story packets, decisions,
  proof expectations, and optional tool availability when present.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | `cargo test -p harness-cli context_pack -- --nocapture` |
| Integration | `scripts/bin/harness-cli context --lane normal` |
| Docs | `rg -n 'Must|Required Context|Read the required|Before work, read' AGENTS.md docs/CONTEXT_RULES.md` should return no matches |
| Platform | `cargo fmt -p harness-cli --check`; `cargo clippy -p harness-cli -- -D warnings`; `git diff --check` |

## Evidence

- `cargo test -p harness-cli context_pack -- --nocapture`
- `cargo build --release -p harness-cli`
- `scripts/bin/harness-cli context --lane normal`
- `scripts/bin/harness-cli context --story US-079`
- `rg -n 'Must|Required Context|Read the required|Before work, read' AGENTS.md docs/CONTEXT_RULES.md` returned no matches.
- `cargo fmt -p harness-cli --check`
- `cargo clippy -p harness-cli -- -D warnings`
- `git diff --check`
- `scripts/bin/harness-cli story verify US-081`
