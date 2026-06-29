# US-059 Electron Desktop Shell For Symphony Web UI

## Status

implemented

## Lane

normal

## Product Contract

The Symphony Web UI can run as a local Electron desktop app while preserving the
existing browser controller and HTTP API boundary. The desktop shell must not
create a second source of truth for Harness stories, Symphony runs, PR state, or
sync state.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/README.md`

## Acceptance Criteria

- Electron launches the same React controller used by `harness-symphony web`.
- The Electron app starts a local `harness-symphony web` backend on loopback and
  loads the UI from that backend or the Vite dev server.
- The renderer continues to use the existing relative `/api/*` routes.
- Packaged desktop assets can be served from the Electron app resources without
  changing the repo-root state location.
- Browser mode remains available through `harness-symphony web`.
- Rebuilding the Web UI and desktop package includes updated React assets.

## Design Notes

- Commands: `npm --prefix crates/harness-symphony/web-ui run desktop:dev`,
  `desktop:build`, and `desktop:smoke`.
- Queries: existing local `/api/*` routes.
- API: unchanged from the browser Web UI controller.
- Tables: none.
- Domain rules: single-active-run and state derivation remain owned by
  `harness-symphony`.
- UI surfaces: Electron desktop window wraps the existing React app.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-059 --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | Rust web tests cover packaged asset directory resolution. |
| Integration | Desktop smoke builds React assets, starts the backend, and verifies `/`, `/health`, and `/api/board`. |
| E2E | Existing Playwright browser coverage continues to prove the shared React controller. |
| Platform | Electron builder creates a macOS app directory with bundled backend and web assets. |
| Release | Not required for MVP; signing, notarization, and auto-update remain out of scope. |

## Harness Delta

None expected.

## Evidence

- `cargo test -p harness-symphony web` passed: 15 web tests, including packaged
  desktop asset directory override coverage.
- `npm --prefix crates/harness-symphony/web-ui run desktop:smoke` passed and
  verified built React assets, `/health`, `/`, and `/api/board` through a
  dynamic loopback backend.
- `npm --prefix crates/harness-symphony/web-ui run desktop:build` passed and
  produced `desktop-dist/mac-arm64/Harness Symphony.app` with bundled
  `bin/harness-symphony` and `web-ui-dist` resources.
- Packaged resource smoke passed using the generated app's bundled backend and
  bundled web assets.
- Follow-up packaged resource smoke verified the generated app resolves
  `/Users/long/projects/symphony-app` as the repo root and returns HTTP 200 JSON
  from `/api/board`; this fixes the packaged `ERR_EMPTY_RESPONSE` board load
  failure.
- Follow-up dev smoke verified `npm --prefix crates/harness-symphony/web-ui run
  dev` starts the Rust backend on `127.0.0.1:4317`, starts Vite on
  `127.0.0.1:5177`, and returns HTTP 200 JSON from Vite-proxied `/api/board`;
  `npm run vite:dev` remains available for Vite-only development.
- `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH="/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge" npm --prefix crates/harness-symphony/web-ui run e2e`
  passed: 2 Chromium tests.
- `cargo test --workspace` passed: 36 Harness CLI tests and 70 Symphony tests.
- `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and
  `git diff --check` passed.
- `scripts/bin/harness-cli story verify US-059` passed.
