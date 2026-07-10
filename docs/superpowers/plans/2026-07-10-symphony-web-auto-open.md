# Symphony Web Auto-Open Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `harness-symphony web` open the local controller after binding, while supporting `--no-open` and preserving Electron's single-window behavior.

**Architecture:** Keep browser launch at the Rust Web server startup boundary, after `TcpListener::bind` resolves the actual address. Use the `webbrowser` crate behind an injectable preparation function so tests can verify URL selection, opt-out, and non-fatal failure without opening a real browser. Electron passes `--no-open` because it already owns the visible window.

**Tech Stack:** Rust 2021, Clap 4, `std::net::TcpListener`, `webbrowser`, Node.js CommonJS Electron helpers, Cargo tests, Electron smoke.

---

## File Map

- Modify `crates/harness-symphony/Cargo.toml` and `Cargo.lock` for `webbrowser`.
- Modify `crates/harness-symphony/src/interface.rs` for `--no-open` parsing.
- Modify `crates/harness-symphony/src/web.rs` for bind-then-open behavior.
- Modify `crates/harness-symphony/web-ui/electron/backend.cjs` and `smoke.cjs` so Electron suppresses browser launch.
- Modify `docs/SYMPHONY_QUICKSTART.md` and the US-082 story evidence.

### Task 1: Add the CLI opt-out contract

**Files:**
- Modify: `crates/harness-symphony/src/interface.rs`
- Modify: `crates/harness-symphony/src/web.rs`

- [ ] **Step 1: Write failing CLI parsing tests**

```rust
#[test]
fn web_auto_open_cli_defaults_to_open() {
    let cli = Cli::try_parse_from(["harness-symphony", "web"]).unwrap();
    let Command::Web(args) = cli.command else {
        panic!("expected web command");
    };
    assert!(!args.no_open);
}

#[test]
fn web_auto_open_cli_accepts_no_open() {
    let cli = Cli::try_parse_from(["harness-symphony", "web", "--no-open"]).unwrap();
    let Command::Web(args) = cli.command else {
        panic!("expected web command");
    };
    assert!(args.no_open);
}
```

- [ ] **Step 2: Run RED**

Run `cargo test -p harness-symphony web_auto_open_cli -- --nocapture`.

Expected: compilation fails because `WebArgs` has no `no_open` field.

- [ ] **Step 3: Add the minimal option and mapping**

Add to `WebArgs`:

```rust
/// Start the local server without opening the system browser.
#[arg(long)]
no_open: bool,
```

Map the command into:

```rust
WebServerOptions {
    host: args.host,
    port: args.port,
    open_browser: !args.no_open,
}
```

Add the mapped field to `WebServerOptions`:

```rust
pub open_browser: bool,
```

- [ ] **Step 4: Run GREEN**

Run `cargo test -p harness-symphony web_auto_open_cli -- --nocapture`.

Expected: both parsing tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-symphony/src/interface.rs crates/harness-symphony/src/web.rs
git commit -m "feat(symphony): add web no-open option"
```

### Task 2: Open the resolved URL after binding

**Files:**
- Modify: `crates/harness-symphony/Cargo.toml`
- Modify: `crates/harness-symphony/src/web.rs`
- Modify: `Cargo.lock`

- [ ] **Step 1: Write failing listener preparation tests**

Add tests using `std::cell::{Cell, RefCell}`:

```rust
#[test]
fn web_auto_open_uses_resolved_listener_url() {
    let opened_url = RefCell::new(None);
    let listener = prepare_web_server(
        WebServerOptions {
            host: "127.0.0.1".to_owned(),
            port: 0,
            open_browser: true,
        },
        |url| {
            opened_url.replace(Some(url.to_owned()));
            Ok::<_, String>(())
        },
    )
    .unwrap();

    let expected = format!("http://{}", listener.local_addr().unwrap());
    assert_eq!(opened_url.borrow().as_deref(), Some(expected.as_str()));
}

#[test]
fn web_auto_open_skips_launcher_when_disabled() {
    let called = Cell::new(false);
    let listener = prepare_web_server(
        WebServerOptions {
            host: "127.0.0.1".to_owned(),
            port: 0,
            open_browser: false,
        },
        |_| {
            called.set(true);
            Ok::<_, String>(())
        },
    )
    .unwrap();

    assert_ne!(listener.local_addr().unwrap().port(), 0);
    assert!(!called.get());
}

#[test]
fn web_auto_open_failure_keeps_listener_available() {
    let listener = prepare_web_server(
        WebServerOptions {
            host: "127.0.0.1".to_owned(),
            port: 0,
            open_browser: true,
        },
        |_| Err("no browser available"),
    )
    .unwrap();

    assert_ne!(listener.local_addr().unwrap().port(), 0);
    assert_eq!(
        browser_open_warning("http://127.0.0.1:4317", "no browser available"),
        "warning: could not open Symphony Web UI at http://127.0.0.1:4317: no browser available. Open the URL manually."
    );
}
```

- [ ] **Step 2: Run RED**

Run `cargo test -p harness-symphony web_auto_open -- --nocapture`.

Expected: compilation fails because `prepare_web_server` and `browser_open_warning` do not exist.

- [ ] **Step 3: Add the dependency**

Run `cargo add webbrowser --package harness-symphony`.

Expected: Cargo records a compatible `webbrowser` version in the crate manifest and lockfile.

- [ ] **Step 4: Implement preparation and non-fatal launch**

```rust
fn browser_open_warning(url: &str, error: impl std::fmt::Display) -> String {
    format!(
        "warning: could not open Symphony Web UI at {url}: {error}. Open the URL manually."
    )
}

fn prepare_web_server<F, E>(
    options: WebServerOptions,
    open_browser: F,
) -> Result<TcpListener, WebError>
where
    F: FnOnce(&str) -> Result<(), E>,
    E: std::fmt::Display,
{
    let listener = TcpListener::bind(format!("{}:{}", options.host, options.port))?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}");
    println!("Symphony Web UI Controller listening at {url}");
    if options.open_browser {
        if let Err(error) = open_browser(&url) {
            eprintln!("{}", browser_open_warning(&url, error));
        }
    }
    Ok(listener)
}
```

Make `run_web_server` call `prepare_web_server(options, webbrowser::open)?` before its unchanged incoming-connection loop.

- [ ] **Step 5: Run GREEN and regression tests**

```bash
cargo test -p harness-symphony web_auto_open -- --nocapture
cargo test -p harness-symphony web -- --nocapture
```

Expected: all focused and existing Web tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock crates/harness-symphony/Cargo.toml crates/harness-symphony/src/web.rs
git commit -m "feat(symphony): open web ui after server bind"
```

### Task 3: Keep Electron backend startup headless

**Files:**
- Modify: `crates/harness-symphony/web-ui/electron/backend.cjs`
- Modify: `crates/harness-symphony/web-ui/electron/smoke.cjs`

- [ ] **Step 1: Add a failing backend-argument assertion**

Import `backendArgs` in `smoke.cjs`, then add:

```javascript
const args = backendArgs({ repoRoot, host: "127.0.0.1", port: 0 });
if (!args.includes("--no-open")) {
  throw new Error("Electron backend must disable the CLI browser launcher");
}
```

- [ ] **Step 2: Run RED**

Run `npm --prefix crates/harness-symphony/web-ui run desktop:smoke`.

Expected: Node fails because `backendArgs` is not exported.

- [ ] **Step 3: Extract pure argument construction**

Add in `backend.cjs`:

```javascript
function backendArgs(options) {
  const args = [
    "--repo-root",
    options.repoRoot,
    "web",
    "--host",
    options.host ?? "127.0.0.1",
    "--port",
    String(options.port ?? 0)
  ];
  if (options.openBrowser !== true) {
    args.push("--no-open");
  }
  return args;
}
```

Use `backendArgs({ repoRoot, host, port, openBrowser: options.openBrowser })` in `spawn`, and export `backendArgs`.

- [ ] **Step 4: Run GREEN**

Run `npm --prefix crates/harness-symphony/web-ui run desktop:smoke`.

Expected: syntax checks, Rust build, backend health, root UI, and board API smoke pass without opening an external browser.

- [ ] **Step 5: Commit**

```bash
git add crates/harness-symphony/web-ui/electron/backend.cjs crates/harness-symphony/web-ui/electron/smoke.cjs
git commit -m "fix(symphony): keep electron backend headless"
```

### Task 4: Document and close US-082

**Files:**
- Modify: `docs/SYMPHONY_QUICKSTART.md`
- Modify: `docs/stories/epics/E08-symphony-web-ui-controller/US-082-web-ui-auto-open.md`

- [ ] **Step 1: Document browser and headless commands**

Add after the readiness check:

````markdown
### Open The Local Controller

Start the Web UI from the repository root:

```bash
target/debug/harness-symphony web
```

After the server binds, Symphony opens the controller in the system default
browser. For CI, SSH, Electron, or other headless use:

```bash
target/debug/harness-symphony web --no-open
```

If the browser cannot be opened, Symphony prints the controller URL and keeps
the server available for manual opening.
````

- [ ] **Step 2: Run story verification**

```bash
cargo fmt --check
scripts/bin/harness-cli story verify US-082
```

Expected: formatting and the configured `web_auto_open` test filter pass.

- [ ] **Step 3: Record proof**

Run:

```bash
scripts/bin/harness-cli story update --id US-082 --status implemented --unit 1 --integration 1 --e2e 0 --platform 1 --evidence "harness-symphony web opens the resolved controller URL after bind; --no-open suppresses launch; launch failure is non-fatal; Electron backend arguments include --no-open; focused Rust tests, Web tests, workspace tests, clippy, desktop smoke, and diff check passed."
```

Set the story markdown status to `implemented` and replace the planned Evidence paragraph with the exact commands and outcomes.

- [ ] **Step 4: Commit**

```bash
git add docs/SYMPHONY_QUICKSTART.md docs/stories/epics/E08-symphony-web-ui-controller/US-082-web-ui-auto-open.md
git commit -m "docs(symphony): document automatic web ui opening"
```

### Task 5: Release verification and trace

**Files:**
- Verify all changed files; fix only failures caused by US-082.

- [ ] **Step 1: Run Rust verification**

```bash
cargo fmt --check
cargo test -p harness-symphony web_auto_open -- --nocapture
cargo test -p harness-symphony web -- --nocapture
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: every command exits 0.

- [ ] **Step 2: Run desktop and repository verification**

```bash
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
git diff --check
git status --short
```

Expected: desktop smoke passes and only intentional implementation-plan tracking changes remain.

- [ ] **Step 3: Record completion trace**

```bash
scripts/bin/harness-cli trace --summary "Implemented Symphony Web UI automatic browser opening" --intake 22 --story US-082 --agent codex --outcome completed --actions "Added --no-open; opened resolved URL after bind; kept launch failure non-fatal; suppressed browser launch for Electron backend; documented and verified behavior." --read "docs/superpowers/specs/2026-07-10-symphony-web-auto-open-design.md,docs/superpowers/plans/2026-07-10-symphony-web-auto-open.md,crates/harness-symphony/src/interface.rs,crates/harness-symphony/src/web.rs,crates/harness-symphony/web-ui/electron/backend.cjs" --changed "Cargo.lock,crates/harness-symphony/Cargo.toml,crates/harness-symphony/src/interface.rs,crates/harness-symphony/src/web.rs,crates/harness-symphony/web-ui/electron/backend.cjs,crates/harness-symphony/web-ui/electron/smoke.cjs,docs/SYMPHONY_QUICKSTART.md,docs/stories/epics/E08-symphony-web-ui-controller/US-082-web-ui-auto-open.md" --decisions "CLI web auto-opens after bind; --no-open is the headless escape hatch; launch failure warns and continues; Electron backend defaults to no-open." --friction "none"
```

Expected: trace tier meets the normal-lane requirement and US-082 verification is already passed.
