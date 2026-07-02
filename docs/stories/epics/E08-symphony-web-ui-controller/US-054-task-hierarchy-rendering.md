# US-054 Task Hierarchy Rendering

## Status

implemented

## Lane

high-risk

## Product Contract

The Symphony Web UI must make Harness story hierarchy visible so task owners can
understand how a larger request decomposes into executable work without relying
on an external planning note.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- Harness can store parent-child story relationships in durable data.
- Board API responses include parent id, child ids, and hierarchy depth for
  each story.
- The browser task detail surface shows hierarchy context for the selected
  story.
- Board cards expose enough hierarchy cues for users to understand grouped
  work.
- Existing dependency and board-state derivation keeps working when hierarchy
  records exist.

## Design Notes

- Commands: `harness-symphony work board`, `harness-symphony web`.
- Queries: `GET /api/board`.
- API: board item JSON includes `parent_id`, `children`, and
  `hierarchy_depth`.
- Tables: `story_hierarchy`.
- Domain rules: hierarchy is explanatory task structure, not a blocker rule;
  dependency edges still own Ready and Blocked derivation.
- UI surfaces: board cards and task detail hierarchy panel.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-054 --unit 1 --integration 1 --e2e 0 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Hierarchy derivation tests cover parent, children, and depth. |
| Integration | Web/board tests prove hierarchy fields are included in API output. |
| E2E | Browser E2E verifies hierarchy is visible in task detail. |
| Platform | Web UI build proves the React surface compiles against the hierarchy fields. |
| Release | Not required. |

## Harness Delta

Added `story_hierarchy` as durable planning structure for Symphony board
presentation.

## Evidence

- `cargo test -p harness-symphony hierarchy` passed.
- `npm --prefix crates/harness-symphony/web-ui run e2e` passed.
- `npm --prefix crates/harness-symphony/web-ui run build` passed.
