# US-091 Symphony Board Status Ownership

## Status

planned

## Lane

normal

## Product Contract

The Symphony controller presents its four board statuses as `Planned`, `Agent
working`, `Human review`, and `Done`, with concise ownership explanations, while
preserving the existing seven-step active-task lifecycle and all internal task
state behavior.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/superpowers/specs/2026-07-13-symphony-board-status-ownership-design.md`

## Acceptance Criteria

- The command status rail, board headers, sidebar, accessible names, and empty
  states consistently use the four approved ownership labels.
- Status headers retain icons and counts and add bounded ownership microcopy.
- `Blocked` remains an exception within Planned; `Needs Attention` remains an
  exception within Agent working with its existing explanation and recovery.
- Internal bucket keys, backend/API contracts, grouping rules, task actions,
  and the seven-step lifecycle are unchanged.
- Desktop and narrow viewports remain readable without new horizontal card or
  page overflow, and status meaning does not depend on color alone.

## Design Notes

- Commands: no new commands.
- Queries: no new queries.
- API: no response-shape changes.
- Tables: no schema changes.
- Domain rules: keep existing Symphony task states and `bucketForItem` mapping.
- UI surfaces: shared bucket presentation metadata, command status rail, board
  column headers, sidebar navigation, accessible labels, and affected tests.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-091 --unit 1 --integration 1 --e2e 1 --platform 1`.

The verify command must unset `HARNESS_RUN_ID` and `HARNESS_RUN_MODE` for its
entire subprocess. Workspace tests exercise Harness CLI fixture writes; allowing
them to inherit the live run id pollutes the run changeset with test operations.

| Layer | Expected proof |
| --- | --- |
| Unit | TypeScript presentation mapping and Rust lifecycle derivation regressions pass. |
| Integration | Board rendering keeps internal grouping while exposing ownership labels. |
| E2E | Playwright covers all four labels, exception placement, unchanged lifecycle order, and narrow viewport overflow. |
| Platform | Web build and Electron desktop smoke pass when available. |
| Release | `cargo fmt --check`, relevant/workspace tests, clippy, design detector, and `git diff --check`. |

## Harness Delta

None. This is a source-repo Symphony controller presentation change and does
not alter the fresh-install Harness template contract.

## Evidence

- `env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE -u HARNESS_SYMPHONY_WEB_DIST_DIR sh -c 'npm --prefix crates/harness-symphony/web-ui run build && npm --prefix crates/harness-symphony/web-ui run e2e && cargo test -p harness-symphony web -- --nocapture && cargo test --workspace && cargo fmt --check && cargo clippy --workspace -- -D warnings && git diff --check'` passed: the Web UI built, all 38 Playwright tests passed, 70 focused Symphony web tests passed, the complete Rust workspace passed, and formatting, clippy, and diff checks exited zero.
- `env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE -u HARNESS_SYMPHONY_WEB_DIST_DIR node .agents/skills/impeccable/scripts/detect.mjs --json crates/harness-symphony/web-ui/src/features/symphony/constants.ts crates/harness-symphony/web-ui/src/features/symphony/board.tsx crates/harness-symphony/web-ui/src/features/symphony/sidebar.tsx` passed with `[]`.
- `env -u HARNESS_RUN_ID -u HARNESS_RUN_MODE -u HARNESS_SYMPHONY_WEB_DIST_DIR npm --prefix crates/harness-symphony/web-ui run desktop:smoke` passed at `http://127.0.0.1:54371`.
