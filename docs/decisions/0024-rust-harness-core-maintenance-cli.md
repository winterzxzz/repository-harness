# 0024 Rust Harness Core Maintenance CLI

Date: 2026-07-21

## Status

Implemented. Cross-platform release publication completes after merge to main.

## Context

The default Harness core is currently copied into a consumer repository by
separate Bash and PowerShell installers. Merge mode safely preserves existing
files by skipping them, but this also prevents upstream workflow improvements
from reaching an installed core. Override mode can replace consumer-owned
documentation and is not an acceptable routine update path.

The optional Rust `harness-cli` owns the historical SQLite lifecycle and
orchestration protocol. Making that compatibility control plane a prerequisite
for ordinary core maintenance would restore the dependency that decisions 0019
and 0020 removed.

## Decision

The next upstream product goal is a Rust CLI named `harness`.

`harness` will be installed with the default core and will own:

- initial core installation after the platform bootstrap selects the binary;
- safe core updates based on installed provenance and three-way comparison;
- dry-run and conflict reporting before consumer files change;
- recoverable, atomic application of an accepted update; and
- core version, integrity, and installation diagnostics.

`harness` will not own intake, stories, matrices, traces, scoring, proposals,
SQLite lifecycle state, work selection, orchestration, or evaluation. Those
surfaces remain optional compatibility or external-product responsibilities.

The existing Bash and PowerShell entry scripts become thin platform
bootstraps. They must download an immutable, checksum-verified `harness`
artifact and delegate installation rather than independently implementing core
update semantics.

The compatibility behavior remains separately available through `harness-cli`.

## Alternatives Considered

1. **Keep copy-on-install with manual migrations.** Rejected as the target
   because important core corrections would remain fragmented across consumer
   repositories.
2. **Add update behavior independently to Bash and PowerShell.** Rejected
   because filesystem, merge, recovery, and provenance semantics would be
   duplicated across platforms.
3. **Extend the optional SQLite control-plane CLI.** Rejected because ordinary
   core maintenance must not reactivate the historical lifecycle dependency.
4. **Create a separately named updater.** Rejected because installation and
   maintenance are one user-facing product; the tool should simply be
   `harness`.

## Consequences

Positive:

- Core installation and maintenance have one cross-platform implementation.
- Consumers can receive upstream improvements without silently losing local
  changes.
- The product has an explicit provenance and diagnostics surface.
- The compatibility control plane remains outside ordinary repository work.

Tradeoffs:

- The default core will gain a small binary dependency.
- Artifact publication and bootstrap verification become part of the core
  release contract.
- Merge ownership, conflict UX, recovery, and migration from existing installs
  require executable proof before cutover.

## Follow-Up

Implementation and validation are recorded in
`docs/plans/completed/rust-harness-core-maintenance-cli.md`.
