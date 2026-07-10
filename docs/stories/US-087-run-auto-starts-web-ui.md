# US-087 Run and Auto Ensure the Symphony Web UI Is Running

## Status

implemented

## Lane

normal

## Product Contract

When an operator or agent executes Symphony work (`harness-symphony run` or
`harness-symphony auto`), the local Symphony Web UI controller must be
available for live visibility without a separate manual `web` command. If no
server is already listening on the configured web address, Symphony spawns one
in the background before the run starts; if one is already listening, it is
reused. The spawned server outlives the run so review can continue after the
run ends. Operators can skip the auto-start with `--no-web`.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- `harness-symphony run <story-id>` with no server on `127.0.0.1:4317` spawns
  a detached `harness-symphony web` process before executing the run, and the
  spawned server opens the browser exactly as a manual `web` start would.
- `harness-symphony run <story-id>` with a server already listening reuses it
  and does not spawn a second process or open another browser tab.
- `harness-symphony auto --enable` applies the same ensure-web behavior.
- `--no-web` on `run` and `auto` skips the check and spawn entirely.
- `run --prepare-only` does not start the web server.
- The spawned web server keeps serving after the run process exits.

## Design Notes

- Commands: `harness-symphony run [--no-web]`, `harness-symphony auto [--no-web]`.
- API: `web::ensure_web_server(&ResolvedConfig, &WebServerOptions) -> EnsureWebOutcome`.
- Domain rules: liveness check is a TCP connect with a short timeout against
  the default web bind address (`127.0.0.1:4317`); spawn uses the current
  executable with `--repo-root` plus the `web` subcommand, detached with
  stdio nulled.
- UI surfaces: no Web UI changes; the existing controller is what gets started.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id <id> --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | `cargo test -p harness-symphony` covering ensure outcome for already-listening vs free port, CLI flag parsing for `--no-web` |
| Integration | Manual: `harness-symphony run <tiny-story> --prepare-only` (no server), then `run` (server appears), then second `run` (reused) |
| E2E | Not required |
| Platform | Not required |
| Release | Not required |

## Harness Delta

None.

## Evidence

- `cargo test -p harness-symphony` — 165 passed, 0 failed (includes new
  `ensure_web_*` unit tests and `--no-web` CLI parsing tests).
- Manual drive (2026-07-11): `run <missing-story>` with no server printed
  `Symphony Web UI starting at http://127.0.0.1:4317` and left a detached
  listener on 4317 after the run process exited; a second `run` printed
  `Symphony Web UI already running`; `run --no-web` printed neither and did
  not touch the port.
