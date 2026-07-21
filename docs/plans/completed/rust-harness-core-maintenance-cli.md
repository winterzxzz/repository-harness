# Execution Plan: Rust Harness Core Maintenance CLI

Date: 2026-07-21

## Status

Complete

## Outcome

Ship a Rust CLI named `harness` with the default core. A fresh consumer can use
it to install the core, and an existing consumer can preview and apply a
recoverable three-way core update without silently discarding local changes.
The optional SQLite control plane remains outside the CLI.

## Context

- `AGENTS.md` names this as the current upstream goal.
- Decision `0024-rust-harness-core-maintenance-cli.md` defines the accepted
  ownership boundary.
- `docs/product/installation-profiles.md` describes current installer behavior.
- `scripts/install-harness.sh` and `scripts/install-harness.ps1` are the current
  cross-platform installation boundary.
- The existing `crates/harness-cli/` remains compatibility implementation, not
  the target core-maintenance CLI.

## Scope

In scope:

- A Rust executable named `harness`.
- Immutable, checksum-verified bootstrap installation on supported platforms.
- Tracked installed provenance sufficient to reconstruct the update base.
- Dry-run, three-way update, conflict, backup, atomic-apply, status, and
  diagnostic behavior.
- Migration from existing copy-on-install cores.
- Focused failure, recovery, cross-platform, and consumer-customization proof.

Out of scope:

- Intake, story, matrix, trace, scoring, proposal, or SQLite lifecycle commands.
- Work selection, agent orchestration, pull-request coordination, or evaluation.
- Automatic resolution of conflicting product or workflow policy.

## Approach

1. Specify the command contract, ownership classes, tracked provenance format,
   conflict result, and recovery guarantees before implementation.
2. Build the Rust CLI and focused unit tests independently of the compatibility
   crate.
3. Prove fresh installation and safe updates for unchanged, consumer-only,
   upstream-only, non-overlapping, and conflicting file changes.
4. Reduce Bash and PowerShell to immutable artifact bootstrap and delegation.
5. Prove migration from representative existing installations on macOS, Linux,
   and Windows.
6. Cut over product documentation and the default install only after the new
   behavior passes repository-wide validation.

## Implementation Contract

Commands:

- `harness install [--directory <path>] [--dry-run]` installs missing core
  files and adopts an existing copy-on-install core without overwriting local
  files.
- `harness update [--directory <path>] [--dry-run]` compares the recorded
  upstream base, the consumer file, and the embedded release payload. It applies
  only a conflict-free plan.
- `harness status [--directory <path>] [--json]` reports installed version,
  target version, local modifications, missing files, and update availability
  without mutation.
- `harness doctor [--directory <path>] [--json]` validates provenance, base
  hashes, path safety, merge availability, and interrupted-transaction state.

Tracked provenance lives under `.harness-core/`: `manifest.json` records the
schema, installed core version, paths, ownership, and upstream hashes; `base/`
stores the exact upstream bytes required for the next three-way comparison.
This is Git-visible package state, not task or control-plane state.

The update rules are deterministic:

1. local equals base: take upstream;
2. upstream equals base: preserve local;
3. local equals upstream: keep that content;
4. both changed: use a real three-way text merge;
5. overlapping changes, unsafe paths, missing managed files, or corrupted base
   state: report a conflict and write nothing.

Application writes are transactional at the command boundary. All target and
state changes are staged, prior bytes are backed up, an interruption journal is
made durable before activation, and provenance is committed last. A later
command rolls back an incomplete transaction before doing new work; a committed
transaction only needs cleanup.

Clean Architecture is structural, not naming-only:

```text
domain <- application <- infrastructure
                    <- interface
composition root wires interface and infrastructure
```

The domain contains pure paths, manifests, merge decisions, plans, and reports.
Application use cases depend only on domain types and ports. Infrastructure
implements embedded distribution, filesystem state, hashing, locking,
transactions, and Git-backed three-way merge. The interface parses commands and
renders results. Mechanical tests reject inward layers importing outward ones.

## Risks And Recovery

- **Consumer data loss:** stage the complete result, stop on unresolved
  conflicts, back up affected files, and update provenance only after atomic
  activation.
- **Supply-chain substitution:** bind the bootstrap, binary, and core payload to
  one immutable release identity and verify checksums before execution.
- **Control-plane scope creep:** keep compatibility commands out of the new
  crate and enforce the command boundary mechanically.
- **Premature cutover:** retain the current installers and documentation until
  migration and rollback have been rehearsed.
- **Recovery:** before cutover, revert the feature branch. After cutover,
  restore the backed-up managed files and prior provenance, then run the prior
  immutable `harness` release.

## Progress

- [x] Record the accepted product boundary and current upstream goal.
- [x] Specify commands, ownership, provenance, conflict, and recovery contracts.
- [x] Implement the independent Rust CLI and focused tests.
- [x] Implement immutable bootstraps and existing-install migration.
- [x] Run cross-platform update and recovery proof.
- [x] Cut over current product documentation and default installation.
- [x] Run full repository validation and record the result.

## Decisions

- 2026-07-21: Name the product and executable `harness`.
- 2026-07-21: Install it with the default core and assign it core installation,
  update, provenance, and diagnostic ownership.
- 2026-07-21: Keep the optional SQLite and orchestration control plane outside
  the new CLI.

## Validation

- Focused proof: 10 unit tests, 2 mechanical architecture tests, 2 CLI lifecycle
  tests, and 2 update lifecycle tests pass. They cover fresh install, legacy
  adoption, dry-run, status, diagnostics, one-sided changes, add/remove,
  non-overlapping merge, overlap conflict, missing files, backup, interrupted
  recovery, apply failure rollback, path safety, and all-or-nothing writes.
- Integration or end-to-end proof: the Bash installer profile, migration,
  upgrade, checksum failure, rollback, dry-run, and optional-consumer boundary
  suites pass. PR #56's native Windows installer contract also passes.
- Repository-required checks: `scripts/validate-premerge.sh` passes with the
  full implementation. PR #56's Linux repository contract and native Windows
  installer contract pass on commit `f65f415`.

## Result

Complete. The default installers now bootstrap the independent Rust `harness`
binary, which installs or adopts the core and provides dry-run, transactional
three-way updates, provenance, status, and diagnostics. The optional
`harness-cli` SQLite control plane remains separate. Five-platform immutable
release publication is wired to post-merge maintenance.
