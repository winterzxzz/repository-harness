# Installation Profiles

The installers expose two product profiles and no arbitrary feature matrix.

## Core

Core is the default. Its exact files are declared in
`scripts/harness-install-files.txt`:

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

The platform installer downloads a checksum-verified `harness` binary, places
it at `scripts/bin/harness` (or `.exe`), and delegates core installation to it.
The CLI records the exact upstream base in `.harness-core/`; future updates use
that base for a conflict-safe three-way merge and persistent backup.

Core performs no compatibility-CLI download, schema discovery, database
bootstrap installation, or database-specific `.gitignore` write. A core update
does not remove an existing `harness-cli` or database.

## Core Plus CLI

`--with-cli` in Bash or `-WithCli` in PowerShell adds the optional
compatibility manifest at
`scripts/harness-cli-install-files.txt`, every `scripts/schema/*.sql` migration,
generated database/binary ignore rules, and a checksum-verified platform
binary. `--upgrade-cli` / `-UpgradeCli` implies this profile.

The compatibility inputs and binary are staged before compatibility target
files change. A staging, download, checksum, or apply failure restores the
previous compatibility files. Core files already installed remain usable.

## Core Update Contract

`harness update --dry-run` reports the planned merge without writing. A normal
update takes upstream content when the consumer did not change a file, keeps a
consumer-only edit, and uses Git's three-way text merge when both sides changed.
An overlapping edit, missing managed file, unsafe path, or corrupt base stops
the complete update. Successful updates write provenance last and retain prior
bytes under `.harness-backup/`.

## Ownership

The installers do not copy this repository's root README, architecture, build
scripts, tests, CI, historical decisions, or provenance into a consumer. Those
paths describe upstream Harness or its evolution, not the consumer product.
