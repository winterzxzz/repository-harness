# Symphony Web Auto-Open Design

## Intent

Starting the local Symphony Web UI should take the operator directly to the
task and run board. Today, `harness-symphony web` binds the loopback server and
prints its URL, but the operator must open that URL manually.

## Approved Behavior

- `harness-symphony web` opens the system default browser once, after the HTTP
  listener binds successfully.
- The opened URL uses the listener's resolved address, so an ephemeral
  `--port 0` launch opens the actual assigned port.
- `--no-open` disables browser launch for CI, tests, SSH sessions, and other
  headless use.
- A browser-launch failure prints a warning to stderr and leaves the Web UI
  server running.
- Task execution does not open additional tabs.
- The Electron desktop shell remains unchanged because it already creates its
  own application window.

## Approaches Considered

### Open from `harness-symphony web` after binding

Selected. This places the behavior at the command that owns the server
lifecycle, guarantees the URL is valid before launch, and opens only one tab.

### Open whenever a Symphony task run starts

Rejected. CLI, automation, and Web UI-started runs would create duplicate or
unexpected tabs, and browser behavior would become coupled to task execution.

### Open from an editor or repository startup hook

Rejected. Editor hooks are environment-specific, cannot reliably own the
server process, and would make repository behavior depend on a particular
development surface.

## Architecture

The CLI interface adds an `open` boolean to `WebServerOptions`, defaulting to
true and exposed as the inverse `--no-open` flag. The web server binds first,
reads the resolved listener address, prints the controller URL, and then asks a
small browser-launch boundary to open that URL.

The browser-launch boundary should use a maintained cross-platform Rust crate
rather than embedding platform-specific `open`, `xdg-open`, or `start` command
logic in the CLI. Keeping the boundary injectable or independently testable
allows unit tests to prove launch and failure behavior without opening a real
browser.

## Data Flow

1. Parse `harness-symphony web [--host <host>] [--port <port>] [--no-open]`.
2. Resolve repository configuration as today.
3. Bind the TCP listener.
4. Read the listener's actual local address and construct `http://<address>`.
5. Print the listening message.
6. Unless `--no-open` is set, invoke the browser-launch boundary once.
7. If launch fails, print a warning and continue accepting HTTP connections.

## Error Handling

- Bind failures remain fatal and must not attempt browser launch.
- Browser-launch failures are non-fatal because the printed URL remains a
  valid manual recovery path.
- `--no-open` must skip the launcher entirely.
- No browser launch occurs from Electron's backend child process; Electron
  should pass `--no-open` when it starts `harness-symphony web` so only the
  existing `BrowserWindow` is created.

## Documentation

- Update `docs/product/symphony-web-ui-controller.md` with the auto-open
  product contract and headless escape hatch.
- Update `docs/SYMPHONY_QUICKSTART.md` to describe the default and `--no-open`.
- Keep browser mode and Electron mode documented as separate launch surfaces.

## Validation

- CLI parsing proves browser opening defaults on and `--no-open` disables it.
- Unit tests prove successful bind triggers exactly one launch with the
  resolved URL.
- Unit tests prove launcher failure does not stop server startup.
- Electron smoke proves the desktop shell still creates only its own window.
- Existing Rust Web UI tests prove API and static serving behavior remains
  unchanged.
- Release checks include Rust formatting, targeted tests, workspace tests,
  clippy, Web UI desktop smoke, and `git diff --check`.

## Scope Boundaries

- No background daemon or login-item behavior.
- No editor-specific repository-open hook.
- No new run-history page or API.
- No change to task execution, run isolation, board state, or active-run rules.
- No authentication or non-loopback serving changes.
