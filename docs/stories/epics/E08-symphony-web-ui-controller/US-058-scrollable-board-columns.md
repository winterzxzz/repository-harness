# US-058 Scrollable Board Columns

## Status

implemented

## Lane

normal

## Product Contract

The Symphony Web UI board must keep each state column bounded within the board
viewport. Columns should scroll internally when they contain many tasks instead
of increasing the whole page height.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/design/symphony-web-ui-controller/README.md`

## Acceptance Criteria

- `Ready`, `Blocked`, `In Progress`, `Review`, `Needs Attention`, and `Done`
  columns all have bounded heights in the board view.
- Each column's task list scrolls vertically when its content exceeds the
  available column body height.
- Column headers remain visible while the task list scrolls.
- Adding many cards to one state does not expand the whole page vertically.
- Desktop keeps horizontal board scrolling across the six columns when needed.
- Mobile remains usable with stacked columns and internal scrolling that does
  not hide task cards or controls.
- Existing filtering, task selection, detail popup, and start/review/sync
  controls continue to work.

## Design Notes

- Commands: `harness-symphony web`, Vite build, Playwright E2E.
- Queries: `GET /api/board`.
- API: no new runtime API expected.
- Tables: no new tables.
- Domain rules: visual layout only; board state derivation remains unchanged.
- UI surfaces: `crates/harness-symphony/web-ui/src/main.tsx` and Tailwind
  classes/styles used by the board shell and columns.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-058 --unit 1 --integration 1 --e2e 1 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | TypeScript build catches layout component regressions. |
| Integration | Vite production build succeeds after bounded-height layout changes. |
| E2E | Playwright verifies all six columns render and at least one column scrolls internally without page-height expansion. |
| Platform | Browser visual check confirms desktop and mobile layout do not overlap or clip controls. |
| Release | Not required. |

## Harness Delta

No harness process change expected.

## Evidence

- `npm --prefix crates/harness-symphony/web-ui run build` passed.
- `npm --prefix crates/harness-symphony/web-ui run e2e` passed with 3
  Chromium tests, including dense board-column internal scroll coverage for
  desktop and mobile viewports.
- `git diff --check` passed.
