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

Core performs no CLI download, schema discovery, bootstrap installation, or
database-specific `.gitignore` write. A core refresh does not remove an existing
CLI or database.

## Core Plus CLI

`--with-cli` in Bash or `-WithCli` in PowerShell adds the manifest at
`scripts/harness-cli-install-files.txt`, every `scripts/schema/*.sql` migration,
generated database/binary ignore rules, and a checksum-verified platform
binary. `--upgrade-cli` / `-UpgradeCli` implies this profile.

The compatibility inputs and binary are staged before compatibility target
files change. A staging, download, checksum, or apply failure restores the
previous compatibility files. Core files already installed remain usable.

## Ownership

The installers do not copy this repository's root README, architecture, build
scripts, tests, CI, historical decisions, or provenance into a consumer. Those
paths describe upstream Harness or its evolution, not the consumer product.
