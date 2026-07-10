# US-085 Symphony Web Server Resilience

## Status

implemented

## Lane

normal

## Product Contract

The local Symphony Web UI backend must stay available while clients misbehave:
a client that disconnects early, resets the connection, or opens a socket
without sending a request must not stop or stall the server for other
requests. The unguarded `POST /api/runs/<run_id>/reject` endpoint, which the
UI no longer uses and which bypassed all recovery guardrails, is removed.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`

## Acceptance Criteria

- A connection error (broken pipe, reset, accept failure) is logged as a
  warning and the accept loop continues serving later requests.
- An idle connection that never sends a request does not block other
  requests; each connection is handled on its own thread with a 30s socket
  read/write timeout so idle sockets cannot hold handler threads forever.
- `POST /api/runs/<run_id>/reject` returns 404 and no longer mutates run
  state; recovery remains available only through the guarded
  request-changes, recover, and pr-retry endpoints.

## Design Notes

- Commands: `harness-symphony web`
- API: removed `POST /api/runs/<run_id>/reject`
- Domain rules: per-connection handler (`handle_connection`) swallows and
  logs request/response IO errors instead of propagating them into the
  accept loop (`serve`) in `crates/harness-symphony/src/web.rs`.
- UI surfaces: none (backend availability fix).

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | `connection_handler_swallows_response_write_failures`, `removed_reject_endpoint_returns_not_found_and_keeps_run_state` in `web::tests` |
| Integration | `serve_answers_requests_while_another_connection_is_idle`, `serve_survives_clients_that_disconnect_early` boot a real listener |
| E2E | Manual: live server survived idle socket, unread-response closes, and an SO_LINGER=0 RST client; `/health` and `/api/board` answered afterwards; log shows swallowed `Broken pipe (os error 32)` warning |
| Platform | n/a |
| Release | n/a |

## Harness Delta

None; defects were found by direct code review, not harness friction.

## Evidence

- `cargo test -p harness-symphony` — 158 passed, 0 failed.
- Live check on `harness-symphony web --no-open --port 4399`: idle
  connection + abusive disconnect/RST clients, then `/health` returned
  `{"ok":true}` and `POST /api/runs/run_x/reject` returned 404; server log
  contained `warning: symphony web response write failed: Broken pipe` and
  the process kept serving.
