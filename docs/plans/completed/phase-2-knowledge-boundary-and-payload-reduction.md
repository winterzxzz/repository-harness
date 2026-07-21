# Phase 2 Epic: Knowledge Boundary And Payload Reduction

Date: 2026-07-21

## Status

Completed on 2026-07-21. The default is the ten-file repository-centered core;
CLI compatibility is explicit, atomic, cross-platform validated, and preserved
during ordinary core refreshes.

## Outcome

Make the repository-centered Harness a physically small and unambiguous
installable core.

A default installation receives only the current repository workflow and its
Git-native knowledge and planning structure. The Rust CLI, SQLite runtime,
orchestration contract, and legacy lifecycle remain supported as one explicit
compatibility bundle. Upstream development material and historical evidence
remain available in this source repository without being installed into an
ordinary consumer.

Phase 2 makes the Phase 1 authority change structurally true. It does not claim
application legibility or improved agent behavior without a real application
and observed evidence.

## Context

- `AGENTS.md` and `docs/WORKFLOW.md` define the repository-centered default.
- `docs/decisions/0019-repository-centered-default-workflow.md` removed the
  SQLite lifecycle from the default task path while preserving compatibility.
- `docs/plans/completed/phase-1-workflow-decoupling.md` records the completed
  Phase 1 transition and its rollback boundary.
- `docs/README.md` already distinguishes current workflow material from
  compatibility references, but the installer payload still distributes both.
- `scripts/harness-install-files.txt`, the Bash and PowerShell installers, and
  their tests define the current consumer payload.
- `docs/contracts/harness-orchestration-v1.md` and the existing CLI release
  path are compatibility contracts that must not be broken by core reduction.
- OpenAI Harness Engineering remains the anchor: give agents a small map,
  preserve structured repository knowledge, and avoid a large competing
  instruction surface.

Decision `0019` currently names application legibility as the next investment.
Before implementation, Phase 2 must promote the knowledge-boundary and payload
direction into lasting decision documentation and explicitly defer
application-legibility claims until real application evidence exists.

## Scope

In scope:

- Classify installed and source-only artifacts as `core`, `compatibility`,
  `upstream-only`, or `historical`.
- Define a minimal default consumer payload.
- Separate core and compatibility installation profiles.
- Make the CLI and its required runtime one explicit, atomic compatibility
  add-on.
- Introduce a compatibility window before core-only installation becomes the
  default.
- Stop installing upstream `repository-harness` product and maintenance
  material as consumer truth.
- Keep current, compatibility, and historical knowledge separately indexed.
- Preserve existing installations, databases, binaries, release assets, and
  orchestration paths during the transition.
- Validate Bash and PowerShell behavior, merge/override/refresh safety, payload
  boundaries, links, and compatibility contracts.

Out of scope:

- Delete or rewrite an existing SQLite database or tracked core snapshot.
- Remove Rust CLI commands, migrations, changesets, or protocol-v1 behavior.
- Refactor the Rust CLI implementation.
- Build browser control, application runtime isolation, logs, metrics, or
  agent-behavior observation.
- Claim that the reduced payload improves application-development outcomes.
- Mass-move all story, review, migration, and evidence files in one change.
- Impose one architecture or fabricated validation command on consumers.
- Add a replacement task database, maturity ladder, trace, or scoring system.

## Product Boundaries

Every artifact must have one primary class and audience.

| Class | Primary audience | Default discovery | Default install |
| --- | --- | --- | --- |
| Core | Ordinary repository agents and consumers | Yes | Yes |
| Compatibility | Explicit CLI or orchestration users | Through a compatibility index | No |
| Upstream-only | `repository-harness` maintainers | In this source repository | No |
| Historical | Provenance and forensic review | Through a historical index | No |

Decision `0020` makes these boundaries lasting product behavior. The following
classification covers every path in the pre-Phase-2 installer manifest plus
the separately discovered schema set.

### Core Classification

| Path | Audience and reason |
| --- | --- |
| `AGENTS.md` | Consumer agents; compact entry map and authority boundary. |
| `docs/WORKFLOW.md` | Consumer agents and humans; canonical default workflow. |
| `docs/README.md` | Consumers; compact current documentation map. |
| `docs/product/README.md` | Consumers; generic location and update rule for real product truth. |
| `docs/plans/README.md` | Consumers; durable-plan selection and lifecycle. |
| `docs/plans/active/README.md` | Consumers; active-plan directory contract. |
| `docs/plans/completed/README.md` | Consumers; retained-plan directory contract. |
| `docs/decisions/README.md` | Consumers; lasting-decision contract and index boundary. |
| `docs/templates/decision.md` | Consumers; template required by the decision contract. |
| `docs/templates/exec-plan.md` | Consumers; template required by the durable-plan workflow. |

### Compatibility Classification

| Path | Audience and reason |
| --- | --- |
| `docs/FEATURE_INTAKE.md` | Explicit CLI users; intake lifecycle semantics. |
| `docs/GLOSSARY.md` | Explicit CLI users; legacy control-plane terminology. |
| `docs/HARNESS_AUDIT.md` | Explicit CLI users; audit command semantics. |
| `docs/HARNESS_BACKLOG.md` | Explicit CLI users; backlog command semantics. |
| `docs/HARNESS_COMPONENTS.md` | Explicit CLI users; implemented control-plane component map. |
| `docs/HARNESS_MATURITY.md` | Explicit CLI users; legacy maturity model. |
| `docs/IMPROVEMENT_PROTOCOL.md` | Explicit CLI users; proposal and intervention lifecycle. |
| `docs/TEST_MATRIX.md` | Explicit CLI users; story proof-matrix semantics. |
| `docs/TOOL_REGISTRY.md` | Explicit CLI users; tool registry semantics. |
| `docs/TRACE_SPEC.md` | Explicit CLI users; trace and scoring semantics. |
| `docs/contracts/harness-orchestration-v1.md` | External orchestrators; versioned process contract. |
| `docs/stories/README.md` | Explicit CLI users; legacy story storage and discovery. |
| `docs/stories/backlog.md` | Explicit CLI users; legacy file-backed backlog reference. |
| `docs/templates/spec-intake.md` | Explicit CLI users; compatibility intake template. |
| `docs/templates/story.md` | Explicit CLI users; compatibility story template. |
| `docs/templates/validation-report.md` | Explicit CLI users; compatibility proof report. |
| `docs/templates/high-risk-story/design.md` | Explicit CLI users; legacy high-risk packet. |
| `docs/templates/high-risk-story/execplan.md` | Explicit CLI users; legacy high-risk packet. |
| `docs/templates/high-risk-story/overview.md` | Explicit CLI users; legacy high-risk packet. |
| `docs/templates/high-risk-story/validation.md` | Explicit CLI users; legacy high-risk packet. |
| `scripts/bootstrap-harness.sh` | CLI users on Unix; creates/migrates local state. |
| `scripts/bootstrap-harness.ps1` | CLI users on Windows; creates/migrates local state. |
| `scripts/harness-cli-release-tag` | CLI users and installers; pins the default artifact tuple. |
| `scripts/schema/*.sql` | CLI runtime; complete discovered migration history. |
| generated `.gitignore` rules | CLI runtime; keep local databases and downloaded binaries untracked. |
| platform CLI binary | CLI users; checksum-verified compatibility executable. |

### Upstream-Only Classification

| Path | Audience and reason |
| --- | --- |
| `README.md` | Harness maintainers; describes this Rust product and distribution. |
| `docs/ARCHITECTURE.md` | Harness maintainers; mixes upstream CLI architecture with consumer discovery guidance. |
| `docs/CONTEXT_RULES.md` | Harness maintainers; extended design guidance beyond the minimal installed workflow. |
| `docs/HARNESS.md` | Harness maintainers; full product model and compatibility boundary. |
| `docs/WORKTREE_CONFLICTS.md` | Harness maintainers; source-state diagnosis for this repository. |
| `docs/demo/README.md` | Harness maintainers; upstream demonstration material. |
| `docs/decisions/0019-repository-centered-default-workflow.md` | Harness maintainers; records this product's workflow migration. |
| `docs/product/installation-profiles.md` | Harness maintainers; installer product contract and profile inventory. |
| `scripts/README.md` | Harness maintainers; build, release, validation, and compatibility operations. |
| `scripts/agent-harness-block.md` | Installers; canonical source fragment, consumed but not installed. |
| `scripts/claude-harness-block.md` | Installers; canonical source fragment, consumed but not installed. |
| `scripts/materialize-core-state.sh` | Harness maintainers; restores this source repository's tracked baseline. |
| `scripts/materialize-core-state.ps1` | Harness maintainers; restores this source repository's tracked baseline. |
| `scripts/publish-core-snapshot.sh` | Harness maintainers; publishes this source repository's baseline. |
| `scripts/verify-core-snapshot.sh` | Harness maintainers; validates this source repository's baseline. |
| `scripts/verify-materialized-core-parity.sh` | Harness maintainers; checks this source repository's baseline replay. |
| `.gitignore` | Harness maintainers; source-repository ignore policy, replaced by generated CLI rules for consumers. |

### Historical Classification

| Path | Audience and reason |
| --- | --- |
| `docs/decisions/0001-harness-first-development.md` | Provenance; superseded mandatory Harness-first workflow. |
| `docs/decisions/0002-post-spec-product-lifecycle.md` | Provenance; superseded product lifecycle. |
| `docs/decisions/0003-generic-spec-intake-harness.md` | Provenance; superseded intake-centered default. |
| `docs/decisions/0004-sqlite-durable-layer.md` | Compatibility provenance; SQLite ownership decision. |
| `docs/decisions/0005-prebuilt-rust-harness-cli.md` | Compatibility provenance; binary distribution decision. |
| `docs/decisions/0006-phase-4-benchmark-triage.md` | Compatibility provenance; legacy benchmark behavior. |
| `docs/decisions/0007-improvement-proposal-rules.md` | Compatibility provenance; legacy proposal behavior. |
| `docs/decisions/0011-reproducible-core-state.md` | Compatibility provenance; source-state reconstruction decision. |

The target default payload is intentionally small. The reviewed core paths are:

```text
AGENTS.md
docs/WORKFLOW.md
docs/README.md
docs/product/README.md
docs/plans/README.md
docs/plans/active/README.md
docs/plans/completed/README.md
docs/decisions/README.md
docs/templates/decision.md
docs/templates/exec-plan.md
```

The full `docs/HARNESS.md`, upstream root `README.md`, and current
`docs/ARCHITECTURE.md` are upstream product truth rather than generic consumer
truth, so they are not members of the core profile.

The optional CLI bundle must be complete rather than binary-only. At minimum it
must account for:

```text
platform Harness CLI binary
bootstrap scripts
schema migrations
required compatibility documentation
protocol contract
database ignore rules
upgrade and checksum behavior
```

## Delivery Strategy

Use a compatibility-first migration:

```text
classify current artifacts
  -> define and validate a core-only preview
  -> retain the current full installation during a compatibility window
  -> make core-only the default
  -> require explicit selection for the CLI bundle
  -> relocate or delete obsolete material only in later work
```

Existing installations are never silently stripped. A core refresh leaves an
existing CLI and database untouched. CLI removal, if ever added, requires a
separate explicit and recoverable operation.

## Ordered Workstreams

### P2-01 — Reconcile Direction And Classify Artifacts

Depends on: none.

- Record the lasting Phase 2 direction and reconcile the application-legibility
  follow-up in decision `0019` before changing installer behavior.
- Classify every current installer-manifest path.
- Identify source files that are incorrectly presented as consumer truth.
- Identify required transitive members of the optional CLI bundle.
- Record unresolved classifications instead of guessing.

Exit evidence:

- Every installed path has one class, audience, install profile, and reason.
- No installer or filesystem behavior has changed.

### P2-02 — Define The Minimal Core Payload

Depends on: P2-01.

- Select the exact core files from the reviewed classification.
- Ensure the core map contains no mandatory CLI or SQLite lifecycle.
- Ensure the core does not replace a consumer's README or claim an unselected
  architecture as current truth.
- Define how an existing consumer's local product, architecture, and validation
  material is preserved.

Exit evidence:

- A proposed core manifest has no compatibility, upstream-only, or historical
  paths.
- Every core path is reachable from the compact documentation map.

### P2-03 — Add A Reversible Core-Only Preview

Depends on: P2-02.

- Add an explicit core-only/without-CLI installation mode while retaining the
  existing default during the compatibility window.
- Make dry-run reveal the selected profile and every planned write.
- Ensure core-only mode performs no CLI release download and adds no
  database-specific ignore rules.
- Keep Bash and PowerShell behavior equivalent.

Exit evidence:

- A fresh core-only fixture contains exactly the reviewed core payload.
- Existing full-install and upgrade behavior remains unchanged.

Implementation note: the core-only preview was exercised as an isolated branch
fixture before the default assertion was changed. No redundant `--without-cli`
flag remains in the final interface: omitting `--with-cli` is the core profile.
Phase 1 supplied the compatibility window by retaining the full installed CLI
after removing it from the documented default workflow.

### P2-04 — Make The CLI An Atomic Optional Bundle

Depends on: P2-01 and P2-03.

- Add an explicit `--with-cli` / `-WithCli` selection for the compatibility
  bundle.
- Keep the stable binary path and immutable release/checksum rules.
- Require or imply CLI selection for explicit CLI upgrades.
- Fail without leaving a partial new binary, schema, bootstrap, or contract
  installation.
- Leave an already installed CLI untouched during an ordinary core refresh.

Exit evidence:

- Core plus CLI installs the complete reviewed compatibility bundle.
- Download or checksum failure leaves the core usable and an old CLI runnable.
- Protocol and historical upgrade tests remain green.

### P2-05 — Flip The Default After The Compatibility Window

Depends on: P2-03 and P2-04.

- Make core-only the default for fresh installations.
- Require explicit compatibility selection for the CLI bundle.
- Keep backed-up merge, override, refresh, and CLI-upgrade behavior explicit.
- Document the change without presenting compatibility commands as default
  workflow steps.

Exit evidence:

- The default installer performs no CLI download.
- Explicit CLI consumers retain the stable command path and protocol.
- Existing installations are not destructively stripped.

### P2-06 — Separate Current, Compatibility, And Historical Discovery

Depends on: P2-01 and P2-02. May proceed alongside P2-03 and P2-04 after the
classification is stable.

- Keep one small current documentation map.
- Add or refine explicit compatibility and historical indexes.
- Separate current, compatibility, and historical decisions in the decision
  index without renumbering history.
- Stop default documentation from deep-linking into historical lifecycle
  instructions.
- Use banners or thin redirect documents during path deprecation; do not
  mass-move the historical tree.

Exit evidence:

- Current workflow retrieval does not require compatibility or historical
  material.
- Compatibility and provenance remain deliberately discoverable.
- Link and documentation-contract tests pass.

### P2-07 — Close The Epic With Repository Evidence

Depends on: P2-04, P2-05, and P2-06.

- Run focused payload, installer, documentation, and compatibility checks.
- Run the full pre-merge repository contract in a fresh checkout or worktree.
- Review the final default and optional payloads as concrete file lists.
- Record limitations and deferred application-legibility work.
- Move this plan to `docs/plans/completed/` only after the default has flipped
  and all compatibility evidence passes.

## Dependency Map

```text
P2-01 classify and decide
  -> P2-02 define core
      -> P2-03 preview core-only
          -> P2-04 optional CLI bundle
              -> P2-05 flip default
  -> P2-06 separate discovery

P2-05 + P2-06 -> P2-07 close and retain evidence
```

## Risks And Recovery

### External consumer breakage

Risk: an external runner may assume that a normal install always provides
`scripts/bin/harness-cli`.

Mitigation: retain the existing default during a compatibility window, provide
explicit CLI selection, preserve the stable path, and keep protocol/upgrade
tests.

Recovery: restore the prior installer default without reconstructing deleted
data or binaries. Do not restore mandatory CLI use to the repository workflow.

### Partial compatibility installation

Risk: the binary is installed without schemas, bootstrap, or its required
contract.

Mitigation: treat CLI compatibility as one reviewed atomic bundle and test
failure at download and checksum boundaries.

Recovery: leave the previous CLI and complete bundle untouched; remove only a
staged temporary candidate.

### Source and consumer truth remain conflated

Risk: upstream README, architecture, release, or maintenance material continues
to appear authoritative in consumers.

Mitigation: classify audience before profile membership and test consumer
fixtures containing their own README and architecture.

Recovery: remove the upstream-only path from the core manifest; do not overwrite
the consumer file.

### Historical evidence becomes unreachable

Risk: reducing default discovery accidentally removes provenance required for
maintenance or compatibility review.

Mitigation: index and demote before relocating; preserve Git history and old
paths during the compatibility window.

Recovery: restore an index or redirect. Do not copy the entire historical tree
back into the core payload.

### Profile complexity becomes a new permanent product

Risk: transitional flags and manifests create another large configuration
surface.

Mitigation: support only core and core-plus-CLI profiles, document their
sunset/default behavior, and reject combinatorial feature selection.

Recovery: collapse aliases after the compatibility window while keeping one
explicit CLI opt-in.

## Acceptance Criteria

- The default fresh install contains only reviewed core paths.
- The default fresh install downloads no CLI binary.
- The default fresh install contains no schema, bootstrap, SQLite lifecycle,
  orchestration, legacy story, scoring, audit, proposal, or historical files.
- The default fresh install does not replace an existing consumer README,
  architecture document, product contract, or validation configuration.
- The optional CLI selection installs one complete, checksum-verified
  compatibility bundle.
- Core dry-run makes no CLI network request and reports no database-specific
  writes.
- Core refresh leaves an existing CLI and database untouched.
- Explicit CLI upgrade retains immutable ref, checksum, backup, and atomic
  replacement behavior.
- Bash and PowerShell install profiles have equivalent contracts.
- Current, compatibility, upstream-only, and historical material have separate
  indexes and no artifact has conflicting default authority.
- Existing protocol-v1, fresh bootstrap, historical upgrade, and release
  compatibility proof continues to pass.
- Phase 2 records no claim about application legibility or agent behavior.

## Validation

Focused proof passed:

- `tests/installer/assert-install-manifest-links.sh` proves the exact ten-file
  core, exact core-plus-CLI file set, safe/disjoint manifests, no default CLI
  lookup, and valid installed links.
- `tests/installer/test-install-harness-modes.sh` proves fresh, merge, override,
  shim, dry-run, explicit CLI, checksum rollback, immutable upgrade, and
  existing CLI/database preservation behavior on Bash.
- `tests/installer/test-install-harness-modes.ps1` proves the equivalent Windows
  profile, rollback, preservation, and upgrade behavior in CI.
- `tests/installer/assert-agent-authority-contract.sh` and
  `tests/docs/test-doc-contracts.sh` prove the compact authority and separated
  current, compatibility, and historical discovery boundaries.
- `scripts/test-install-harness-cli-upgrade.sh` and
  `tests/installer/assert-consumer-changeset-trackable.sh` preserve the previous
  immutable upgrade and consumer changeset contracts.

The repository-required command set passed:

```text
tests/installer/assert-agent-authority-contract.sh
tests/installer/assert-install-manifest-links.sh
tests/installer/test-install-harness-modes.sh
tests/installer/test-install-harness-modes.ps1
tests/docs/test-doc-contracts.sh
tests/protocol/smoke-native-artifact.sh
tests/protocol/smoke-native-artifact.ps1
tests/installer/test-cli-upgrade-candidate.sh
scripts/validate-premerge.sh
git diff --check
```

`scripts/validate-premerge.sh` passed from a disposable checkout of commit
`1fce67c` after reconstructing its ignored CLI and database from current source
and tracked core state. The final committed state `c6d5705` then passed both
jobs in [GitHub Actions run 29798147153](https://github.com/hoangnb24/repository-harness/actions/runs/29798147153):

- Linux full pre-merge contract and initial-to-candidate CLI upgrade.
- Windows PowerShell installer profiles, checksum rollback, state preservation,
  bootstrap, and initial-to-candidate upgrade.

The original working directory's full gate was not used as completion evidence
because its ignored `harness.db` contains local intake rows beyond tracked core
state. That file and the user's `harness.db.bk`/`scripts/bin/` paths were left
untouched.

## Progress

- [x] Agree that repository-only Phase 2 should not claim unobserved application
      legibility or agent behavior.
- [x] Define the epic outcome, boundaries, migration order, risks, and initial
      acceptance criteria.
- [x] P2-01: decision `0020` reconciles lasting direction and every previous
      installed path has a primary classification.
- [x] P2-02: the reviewed ten-file minimal core and complete CLI transitive
      boundary are defined above.
- [x] P2-03: isolated Bash and PowerShell fixtures define and exercise the exact
      core-only payload and dry-run behavior.
- [x] P2-04: `--with-cli` / `-WithCli` selects a staged, checksum-verified,
      rollback-protected compatibility bundle; explicit upgrade implies it.
- [x] P2-05: core-only is the fresh default, CLI/database writes require opt-in,
      and core refresh leaves an existing scripts tree and database untouched.
- [x] P2-06: the installed current map and source-only compatibility,
      provenance, and decision indexes have separate authority.
- [x] P2-07: focused, clean-checkout, Linux CI, and Windows CI evidence passes;
      the result is recorded and this plan is archived.

## Decisions

- 2026-07-21: Phase 2 is repository-only knowledge-boundary and payload
  reduction. Application legibility is deferred until a real application and
  observable evidence exist.
- 2026-07-21: The CLI becomes optional through explicit atomic packaging, not
  deletion or silent removal from existing installations.
- 2026-07-21: Core and core-plus-CLI are the only intended install profiles;
  Phase 2 will not create arbitrary feature combinations.
- 2026-07-21: Classification and a reversible preview precede changing the
  installer default.
- 2026-07-21: Historical evidence is indexed and demoted before any relocation.
- 2026-07-21: `docs/HARNESS.md`, the root README, and `docs/ARCHITECTURE.md`
  remain upstream-only because they describe or mix in this repository's Rust
  product rather than a consumer application's accepted truth.
- 2026-07-21: The core includes `docs/templates/decision.md` in addition to the
  initial candidate list because the installed decision index directly requires
  that template.
- 2026-07-21: Do not retain a redundant `--without-cli` / `-WithoutCli` alias
  after the default flip. The absence of CLI selection is the core profile, and
  the isolated installer fixtures preserve preview evidence.

## Result

Phase 2 is complete.

Cause and effect:

1. The default installer reads only `scripts/harness-install-files.txt`, so a
   fresh consumer receives exactly ten core files and no CLI, schema, bootstrap,
   database ignore rule, upstream README, or upstream architecture.
2. `--with-cli` / `-WithCli` selects
   `scripts/harness-cli-install-files.txt`, fourteen discovered migrations,
   generated ignore rules, and a checksum-verified binary, so compatibility is
   complete without competing with the default workflow.
3. The compatibility inputs and binary are staged before target mutation and
   prior files are snapshotted, so download, checksum, or apply failure restores
   the old bundle while leaving the newly installed core usable.
4. Core conflict detection excludes the scripts tree, so an ordinary core merge
   or override leaves an existing CLI and database untouched.
5. The installed map contains only current generic structure, while source-only
   compatibility and provenance indexes retain deliberate access to older
   behavior and evidence.

No CLI commands, SQLite schemas, historical evidence, existing databases, or
existing binaries were deleted. External automation that needs the CLI must now
select the compatibility profile explicitly. Application legibility and agent
behavior remain unmeasured and are deliberately not claimed as Phase 2 results.
