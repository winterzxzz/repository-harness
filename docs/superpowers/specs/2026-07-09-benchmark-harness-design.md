# Benchmark Harness — Design

Date: 2026-07-09
Status: approved (design + spec-review decisions locked); ready for implementation plan
Author: brainstormed with the maintainer

## Summary

The maturity ladder (`docs/HARNESS_MATURITY.md`) claims benchmark indicators —
functional score, harness compliance, trace quality — at every level, but the
repository has **no committed benchmark runner, task set, or protocol**. Prior
"Phase 4 benchmark" runs (see `docs/decisions/0006-phase-4-benchmark-triage.md`)
happened externally and never landed in the repo. As a result:

- The core thesis ("agents need better repositories, not just better prompts")
  is asserted, not measured.
- **H3** stays *partial*: it needs run-over-run attribution of a regression to a
  harness component, which requires repeatable measurement.
- **H5** stays *partial*: `propose` emits improvement suggestions, but there is
  no way to measure whether an applied change helped or hurt.

The Benchmark Harness is the missing scale: run the same task twice — once in a
**bare** repo (H0) and once in a **harnessed** repo (Hn) — score both, and
report the delta and its attribution to harness responsibilities.

Concrete motivating example: the maintainer just shipped "Context Diet"
(US-081), a large change from required-reading to an advisory index. Whether it
made agents better or worse is currently **unknowable**. This harness is built
to answer exactly that class of question.

## Goals

- Prove the thesis: measure functional delta between H0 (bare) and Hn
  (harnessed) on the same tasks.
- Attribute regressions to a harness responsibility/component (unblock H3).
- Make improvement effects measurable: re-run after a harness change and compare
  responsibility scorecards (unblock H5).
- Compose existing CLI verbs (`score-trace`, `score-context`, `story verify`,
  `query`) rather than reinventing scorers.

## Non-Goals (v0)

- **No auto-trigger.** The benchmark runs only when the maintainer invokes it.
  No hooks, no per-commit runs, no CI wiring. (Explicit constraint.)
- **No LLM-judge.** v0 checks are deterministic only. Free-text quality judging
  (e.g. "is this decision substantive") is deferred.
- **No multi-agent matrix.** Pin one agent per run (configurable) for
  apples-to-apples comparison.
- **No concurrency.** Capture is sequential (Symphony already holds a single
  active-run lock).
- **No advanced statistics.** Aggregate with pass-rate + median + min/max range.
- **Small fixture.** One fixture app, ~5–6 tasks, grown later.

## Architecture — Hybrid (capture ⟂ score)

Capture and scoring are decoupled on purpose. Capture is live, stochastic, and
costs API spend; scoring must be deterministic, cheap, and repeatable.

```text
CAPTURE (live, manual, batched)              SCORE (offline, deterministic, on-demand)
  fixture task ──┬─ arm H0 (bare)              run artifact ──┬─ functional  (tests on patch)
                 └─ arm Hn (harnessed)                        ├─ compliance  (query harness.db)
  each arm × K runs (noise)                                   ├─ trace qual  (score-trace)
  artifact = {transcript, patch,                              ├─ context fit (score-context)
              harness.db snapshot, changeset}                 └─► responsibility scorecard
                                                                  + H0−Hn delta
```

Because capture stores the **produced patch**, functional re-scoring is exact
(tests are deterministic on a fixed patch). This is why the design keeps a
functional score that a pure trace-replay would lose. Both halves are
explicit-invoke; neither runs automatically.

## Arms — strip-based (single source of truth)

```text
Hn (harnessed) = full fixture
H0 (bare)      = fixture − (harness file set)   # the reverse of `install`
```

The bare arm is **derived**, not maintained separately, so the two arms can
never drift: product code is identical, only the harness layer differs. The
strip list is the installer manifest `scripts/harness-install-files.txt` (minus
a small allowlist of app-shared files such as the fixture's own `README`).
Stripping also removes `harness.db`, `scripts/bin/harness-cli`, `.harness/`,
`AGENTS.md`, and `CLAUDE.md`, so the bare agent cannot read harness context.

Rejected alternatives: two hand-maintained branches/snapshots (drift risk); an
env/flag toggle (harness files still present → not a true bare arm).

## Fixture + task set

The fixture is a **standalone app template** (not a workspace member) copied to a
temp dir and `git init`-ed per capture. **v0 stack (decided): Rust.** A small
Rust app (consistent with this repo, no new toolchain, deterministic
`cargo test`). The functional check is abstracted as a per-task test command, so
other stacks can be plugged later.

Task slices deliberately span lanes and isolate responsibilities so a score drop
points at a component. T-numbering continues `0006` (T4 = auth):

| Task | Lane | Prompt shape | Primary responsibility stressed |
| --- | --- | --- | --- |
| T1 | tiny | rename/copy a label | Task specification, Context selection (minimal) |
| T2 | normal | add "status change" feature with tests | Verification, Task state |
| T3 | normal | add "email overdue reminder" via a provider SDK stub | Tool access (capability gate), External systems |
| T4 | high-risk | add login/session/password auth | Task specification (lane), Project memory (decision), Permissions, Verification |
| T5 | ambiguous | vague "make tasks better" | Task specification (classify + narrow, not sprawl) |
| T6 (optional) | normal | change existing assignment rule with tests | Verification, existing-behavior regression |

## Rubric — 4 dimensions, 2 families

| Family | Dimension | Measured on | Purpose |
| --- | --- | --- | --- |
| Cross-arm | Functional (tests pass) | **H0 + Hn** | delta Hn−H0 = thesis proof |
| Harness-only | Compliance (right records exist) | Hn | H3 attribution, H5 delta |
| Harness-only | Trace quality | Hn | H3 attribution, H5 delta |
| Harness-only | Context fit | Hn | H3 attribution, H5 delta |

Harness-only dimensions are 0 on the bare arm by construction (no harness), so
they are compared **run-over-run on Hn**, not across arms. Functional is the
only cross-arm comparison.

### Attribution

Every check is tagged with one harness responsibility (from the
`docs/HARNESS_MATURITY.md` responsibility matrix). Scores roll up by tag into a
**component scorecard**. Example for T4 (auth, high-risk):

| Check | Deterministic measure | Responsibility | Family |
| --- | --- | --- | --- |
| auth tests pass | run fixture test command | *(product)* | cross-arm |
| intake row, lane = high-risk | query `harness.db`, expected vs actual | Task specification | harness-only |
| decision row exists | query `harness.db` decisions | Project memory | harness-only |
| story + verify passed | query + `story verify` state | Verification | harness-only |
| trace tier ≥ 2 | `score-trace` | Observability | harness-only |
| read FEATURE_INTAKE + ARCHITECTURE | `score-context` | Context selection | harness-only |

Re-running after Context Diet: if "Context selection" drops while functional
holds, the change hurt context behavior. That is precisely the question H3 asks.

### Deterministic-only (v0)

Existence checks and expected-vs-actual comparisons (lane, verify pass, record
presence, files read) are all mechanical. Only free-text quality ("is the
decision substantive") needs a judge, which is deferred. This keeps the scoring
half clean and repeatable, honoring the on-demand constraint.

### Expected files reuse the harness proof model

Each task carries one `expected` file whose shape mirrors a **story packet's
proof matrix** (lane + validation + expected records). The benchmark grades the
agent against the same proof model the harness itself preaches. New surface is
thin: the per-task expected files + one orchestrator that calls existing verbs
and rolls up.

## Nondeterminism — K runs

Live agent runs are stochastic; one run per (task × arm) is noise.

- Default **K = 3** runs per (task, arm), configurable via flag.
- Functional aggregate = **pass rate** (fraction of K that pass tests).
- Harness-only aggregate = **median**; report min/max range so a noisy result is
  visible.
- Bump K (5–10) manually when a delta is borderline.

## Artifact + report format

Per-run capture artifact:

```text
artifact/<task>/<arm>/<k>/
  meta.json        # task id, arm, k, agent, timestamps, exit code
  transcript.log   # agent transcript
  patch.diff       # produced change (git diff)
  harness.db       # snapshot (Hn only)
  changeset.jsonl  # (Hn only)
```

Report (committed under `benchmark/reports/<timestamp>-<label>.{json,md}`):

- Per task: functional pass-rate H0 vs Hn; harness-only scores (Hn).
- **Responsibility scorecard**: score per responsibility, with delta vs the
  previous report.
- Headline: functional delta Hn−H0, compliance %, trace avg, context avg.

v0 persists reports as committed files (source of truth for run-over-run
comparison). A `benchmark_run` table in `harness.db` for querying is a later
enhancement, not v0-critical.

## Code placement — new `harness-bench` crate

Placement mirrors the hybrid split and respects the existing dependency rule
(`docs/ARCHITECTURE.md`): benchmark is an outer consumer of both existing crates.

- **Capture** reuses **Symphony** — it already owns worktree isolation, agent
  adapters, run contracts, and changesets.
- **Score** reuses **harness-cli** scorers — it owns the DB, `score-trace`,
  `score-context`, `story verify`, and `query`.
- A new workspace crate **`harness-bench`** is the single home for
  benchmark-specific concepts: fixture template, task set, expected files,
  arm strip logic, orchestration, and report format. It depends on the other two
  crates rather than bloating either.

Entry points (illustrative):

```text
harness-bench capture --agent <a> --tasks T1..T6 --k 3   # live, manual, batched
harness-bench score   <capture-dir>                      # deterministic
harness-bench report  <capture-dir>                      # roll-up + delta vs previous
```

## How this unblocks H3 and H5

- **H3**: the responsibility scorecard with run-over-run deltas is the
  "attribute a moved/regressed responsibility" output the ladder requires.
- **H5**: apply a proposed harness change, re-run, compare scorecards → predicted
  vs actual delta. A thin follow-up wires `harness-cli propose` to read benchmark
  report deltas (not v0-critical; v0 delivers the report first).

## v0 milestone slice (smallest useful)

1. `harness-bench` crate skeleton + report/artifact schema.
2. Fixture app template (Rust task-tracker) + T1, T2, T4 (tiny/normal/high-risk).
3. Arm strip from the installer manifest.
4. Capture via one Symphony agent adapter, K = 3, sequential.
5. Deterministic scorers composing existing verbs + responsibility roll-up.
6. First committed report showing H0 vs Hn functional delta + a component
   scorecard.

T3 (tool access), T5 (ambiguous), T6 (regression) follow once the loop is proven.

## Decisions to record at implementation time (harness's own rule)

Landing this introduces a new crate, a new CLI contract, and a measurement
model. Per `docs/FEATURE_INTAKE.md`, this is **high-risk** (new architecture
direction, multi-component, public-ish CLI surface). Implementation must add a
durable decision under `docs/decisions/` covering: the benchmark contract, the
arm-derivation method, and the report format.

## Resolved decisions (spec review)

- **Fixture stack: Rust.** Chosen over a faster scripting stack: toolchain
  simplicity and consistency with this repo outweigh raw K-run iteration speed.
  The functional check stays a pluggable test command, so another stack can be
  added later if iteration speed becomes the bottleneck.
- **v0 task scope: T1, T2, T4 only** (tiny / normal / high-risk). T3 (tool
  access), T5 (ambiguous scope), and T6 (regression) are deferred until the
  capture→score→report loop is proven.
