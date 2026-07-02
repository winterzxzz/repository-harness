# Develop Into Main PR Review

Review target: `develop` -> `main`

Base: `main` at `f07cd06db8f329cbe4009b730704d67ed8c3016e`
Head: `develop` at `85b62b35492ad0ddc50b30a130f5fae54604e597`

Intake: `#146`

## Review Intent

This review treats the branch as a product and architecture slice, not only a
line-by-line patch. The intended feature set appears to be:

- Add Harness Symphony as a local story runner.
- Make durable semantic changesets replayable into the Harness SQLite index.
- Provide isolated run worktrees, run state, PR creation, approval sync, and
  retention.
- Add a local browser/Electron controller for board, detail, review, sync, logs,
  dependency graph, and task UX.
- Document the Symphony operating model through product docs and story packets.

## Findings

### P1: Committed changesets do not rebuild the branch's durable story index

Status: Resolved.

Files:

- `.harness/changesets/run_1782473523_99206.changeset.jsonl`
- `.harness/changesets/run_1782536604_52965.changeset.jsonl`
- `.harness/changesets/run_1782543459_701.changeset.jsonl`
- `.harness/changesets/run_1782550121_26667.changeset.jsonl`
- `docs/SYMPHONY_SCOPE.md`

Issue: The branch documents committed changesets as the source of truth for
rebuilding `harness.db`, but the only committed changesets seed/update four
stories: `US-057`, `US-058`, `US-060`, and `US-062`. A fresh rebuild from the
branch changesets succeeds but produces only those four story rows:

```text
HARNESS_DB_PATH=<tmp>/harness.db scripts/bin/harness-cli db rebuild --from .harness/changesets
Applied 4 changeset(s), 20 operation(s).
story count: 4
story ids: US-057,US-058,US-060,US-062
```

The live local `harness.db` used by `scripts/bin/harness-cli query matrix`
contains the broader Symphony story set (`US-028` through `US-063`), but that
database is ignored and not present in `main..develop`. A fresh clone that
follows the documented rebuild path therefore loses most of the branch's
durable task/proof index, including the runner, sync, web backend, review, and
desktop stories.

Impact: The branch's core architecture says "`harness.db` is a local index" and
"Any clone can rebuild `harness.db` by replaying committed changesets." With the
current committed changesets, fresh clones can rebuild a syntactically valid but
semantically incomplete index. That makes `work list`, `work board`, proof
matrix checks, and future Symphony task selection drift from the committed docs.

Expected fix: Commit complete seed changesets for the durable story/proof state
introduced by this branch, or change the rebuild path to import the committed
story docs before applying incremental changesets. Add a regression proof that a
fresh `db rebuild --from .harness/changesets` produces the expected Symphony
story rows and proof records.

Resolution: Commit `29a0d46` added
`.harness/changesets/run_0000000000_seed_symphony_index.changeset.jsonl` and
`scripts/validate-changeset-rebuild.sh`. The rebuild proof now restores 37
Symphony story rows, including `US-028` through `US-063`, `US-061` planned, and
`US-SYM-001`.

### P2: Keep diff-check validation passing

Status: Resolved.

Files include:

- `crates/harness-symphony/web-ui/index.html`
- `crates/harness-symphony/web-ui/postcss.config.js`
- `crates/harness-symphony/web-ui/src/components/ui/badge.tsx`
- `crates/harness-symphony/web-ui/src/components/ui/card.tsx`
- `crates/harness-symphony/web-ui/src/lib/utils.ts`
- `crates/harness-symphony/web-ui/tsconfig.json`
- `crates/harness-symphony/web-ui/vite.config.ts`
- multiple new story Markdown files
- `scripts/schema/007-story-dependencies.sql`
- `scripts/schema/008-story-hierarchy.sql`

Issue: `git diff --check main..develop` fails because 20 newly added files
contain blank lines at EOF. This blocks the repository's documented proof
command even when Rust and web builds pass.

Impact: Any reviewer, CI job, or agent loop that runs the documented validation
can reject the branch before testing functional behavior.

Expected fix: Remove the trailing blank lines or whitespace errors from all
affected files, then rerun `git diff --check`.

Resolution: Removed the blank EOF lines from the affected files. `git diff
--check main` now passes for the working tree; after committing this fix,
`git diff --check main..develop` will cover the same committed branch diff.

### P2: Idempotent sync skip does not transition the reviewed run to Done

Status: Resolved.

Files:

- `crates/harness-symphony/src/sync.rs`
- `crates/harness-symphony/src/web.rs`
- `crates/harness-symphony/src/work.rs`

Issue: `sync_changeset()` is meant to be idempotent, but `apply_changeset_path`
updates the run's `sync_status` only when `harness-cli db changeset apply`
prints an "applied" result. If the root `harness.db` already has the changeset
but `.symphony/state.db` does not show the run as synced, the CLI apply path can
return a skipped/idempotent result. In that case `changeset_sync` is recorded,
but `run_state.sync_status` remains `not_applied`.

The Web API then returns `applied: false`, and board derivation keeps a
completed run with a PR URL in `Review` because `Done` depends on
`run.sync_status` being `applied`, `synced`, or `synced_locally`.

Impact: A user can mark a PR merged and approve sync after the changeset has
already been applied by a CLI sync, an earlier partial Web attempt, or a rebuilt
database, yet the UI remains stuck in `Review` instead of acknowledging the
idempotent success. This makes the controller a second, stale source of truth
over the same durable changeset.

Expected fix: Treat "already applied" as a successful sync for the run as well
as for `changeset_sync`. Update `run_state.sync_status` to `synced` whenever
`harness_db_has_changeset` is true after the apply attempt, or have
`apply_changeset_path` return an explicit already-applied state that the Web
route maps to `Done`.

Resolution: `apply_changeset_path` now treats durable already-applied state as
successful for the run, records `changeset_sync` as applied, and heals
`run_state.sync_status` to `synced`. Added regression coverage for both skipped
CLI apply output and preexisting `changeset_sync` rows whose run state still
said `not_applied`.

### P2: Symphony sync still reads changeset_applied before migrating old databases

Status: Resolved.

Files:

- `crates/harness-symphony/src/sync.rs`
- `crates/harness-cli/src/infrastructure.rs`

Issue: The CLI changeset apply path now calls `migrate()` before checking
`changeset_applied`, but Symphony sync still calls `harness_db_has_changeset`
before invoking `scripts/bin/harness-cli db changeset apply`. That helper opens
the root `harness.db` directly and queries:

```sql
SELECT 1 FROM changeset_applied WHERE id=?1;
```

For a repository whose local DB was created before migration 006, both
`sync_changeset()` and `unapplied_changesets()` can fail with `no such table:
changeset_applied` before the fixed CLI apply path gets a chance to migrate.

Impact: Upgraded local checkouts can still be unable to use Symphony Web sync or
sync status checks unless the user manually runs a migration first. This weakens
the branch's "idempotent post-merge sync" workflow because the migration fix is
present in the CLI but bypassed by the Symphony precheck.

Expected fix: Ensure the root Harness DB is migrated before Symphony reads
`changeset_applied`, or make `harness_db_has_changeset` treat a missing
`changeset_applied` table as "not applied" and let the CLI apply command perform
the authoritative migration and idempotency check.

Resolution: `harness_db_has_changeset` now checks for the
`changeset_applied` table before querying it. Missing migration 006 is treated
as not applied, allowing the CLI apply path to run its migration and
idempotency logic. Added a regression test for old databases without
`changeset_applied`.

### P2: PR creation failures make the retry path reject the completed run

Status: Resolved.

Files:

- `crates/harness-symphony/src/web.rs`
- `crates/harness-symphony/src/pr.rs`
- `crates/harness-symphony/src/interface.rs`
- `docs/product/symphony-web-ui-controller.md`

Issue: When a Web-started run completes successfully but automatic PR creation
fails, `create_review_pr` changes the run status to `failed`. The documented UI
failure workflow expects Needs Attention to expose retry controls when safe, and
the CLI has a `pr retry` command, but that command calls the same `create_pr`
path as initial creation. `plan_pr` rejects runs whose status is `failed`, so
the failed PR creation has converted a completed run with review artifacts into
a state that cannot retry PR creation through the provided command.

Impact: A transient `gh` failure, auth issue, network issue, or provider error
can strand an otherwise valid completed run. The board shows Needs Attention,
but the natural recovery command (`harness-symphony pr retry <run_id>`) fails
because the status was overwritten from `completed` to `failed`.

Expected fix: Represent PR creation failure separately from run outcome, for
example with `pr_status='failed'` or a dedicated next action, while keeping the
run status `completed`. Alternatively allow `pr retry` to plan from runs that
have valid completed artifacts even when their current status is the specific
PR-failed state.

Resolution: PR creation failure now records `pr_status='failed'` and a retry
next action without changing the run outcome from `completed`. Board derivation
shows the completed/no-PR failure in Needs Attention, while the existing PR
planning path remains available for completed runs.

### P2: Web UI story packets and durable story rows are out of sync

Status: Resolved.

Files:

- `docs/stories/epics/E08-symphony-web-ui-controller/README.md`
- `docs/stories/epics/E08-symphony-web-ui-controller/US-061-frankentui-app-server-log-tui.md`
- local durable `story` rows in `harness.db`

Issue: The committed E08 story packet set and the durable story matrix disagree.
The local durable matrix contains `US-054` and `US-055` as implemented stories,
but the branch has no committed story packet for either id. Conversely, the
branch commits `US-061-frankentui-app-server-log-tui.md` and lists `US-061` in
the E08 README, but `scripts/bin/harness-cli query matrix` has no durable
`US-061` row.

Impact: The Web UI board is derived from the durable `story` table, so `US-061`
will not appear as planned/runnable work even though the committed epic docs
advertise it. Reviewers also cannot inspect the committed story scope for
`US-054` and `US-055` even though the proof matrix says those stories were
implemented. This weakens the branch's "UI shows all Harness stories" contract
and makes future Symphony runs depend on whichever source a human happens to
notice first.

Expected fix: Add the missing `US-054` and `US-055` story packets or retire the
durable rows if they should not be part of the branch. Add/import a durable
`US-061` story row with planned status and its verification expectations, or
remove the packet from the E08 story list if it is not accepted work.

Resolution: Added committed `US-054` and `US-055` story packets and updated the
E08 index. Added/imported a planned durable `US-061` row in the local matrix and
the committed rebuild seed changeset, so fresh rebuilds and the Web board see
the same accepted work.

## Investigated Areas Without Confirmed Findings

- `cargo fmt --check`: passed.
- `cargo test --workspace`: passed, including 37 `harness-cli` tests and 79
  `harness-symphony` tests.
- `cargo clippy --workspace -- -D warnings`: passed.
- `npm --prefix crates/harness-symphony/web-ui run build`: passed.
- `npm --prefix crates/harness-symphony/web-ui run e2e`: passed, 6 Chromium
  tests.
- `npm --prefix crates/harness-symphony/web-ui run desktop:smoke`: initially
  failed because the local `node_modules` tree did not contain the declared
  `electron` dev dependency; after `npm --prefix crates/harness-symphony/web-ui
  ci`, the same smoke passed.
- Web UI state refresh after Mark Merged: current code updates the open review
  state after the API response, so the earlier stale Approve Sync disabled-state
  issue is not present in this head.
- Static web assets: current backend serves raw bytes with content-length based
  on the byte vector and has MIME coverage for common binary asset types, so the
  earlier binary corruption issue is not present in this head.
- Run ID generation: current code uses nanoseconds plus a process-local atomic
  sequence, so the earlier same-second collision path is not present in this
  head.
- Active-run preflight: `prepare_run` and `prepare_here_run` check the active
  run lock before creating worktree/run artifacts, so the earlier orphaned
  side-effect path is not present in this head.

## Review Notes

- Do not stop at the first issue.
- Keep this file updated after each review loop.
- Prefer concrete file/line evidence for every finding.
- The confirmed findings above have fixes and validation evidence. After the
  remaining fix commit lands, rerun `git diff --check main..develop` against
  the committed branch tip.
