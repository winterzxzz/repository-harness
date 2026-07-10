# US-083 Clean Installer Payload

## Status

planned

## Lane

normal

## Product Contract

A fresh Harness installation contains reusable operating policies, templates,
schema, CLI tooling, and empty project scaffolds without copying task history
or project identity from `repository-harness`.

## Relevant Product Docs

- `README.md`
- `docs/HARNESS.md`
- `docs/FEATURE_INTAKE.md`
- `docs/superpowers/specs/2026-07-10-clean-installer-payload-design.md`
- `scripts/README.md`

## Acceptance Criteria

- Fresh installs do not copy the source repository root `README.md`.
- Fresh installs do not copy numbered files from `docs/decisions/`.
- Fresh installs retain `docs/decisions/README.md` as an empty scaffold.
- Fresh installs retain core policy documents including `docs/HARNESS.md`,
  `docs/FEATURE_INTAKE.md`, `docs/ARCHITECTURE.md`, and
  `docs/CONTEXT_RULES.md`.
- Fresh installs retain templates, schema migrations, CLI tooling, and empty
  product, story, backlog, decision, and test-matrix scaffolds.
- Installer payload validation rejects source history, run artifacts,
  changesets, databases, and source-repository planning artifacts.
- Bash and PowerShell installers continue to read the same payload manifest.
- Existing files in `--merge` targets are not deleted or rewritten because an
  entry was removed from the source manifest.
- Installer documentation explains that installed projects generate their own
  docs and operational history after installation.

## Design Notes

- Commands: `scripts/install-harness.sh --directory <target> --yes --dry-run`
- Queries: none.
- API: none.
- Tables: none.
- Domain rules: installation copies the Harness operating kit, not the source
  repository's task history or identity.
- UI surfaces: terminal installer output only.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id US-083 --unit 1 --integration 1 --e2e 0 --platform 1`.

| Layer | Expected proof |
| --- | --- |
| Unit | Payload-boundary assertions reject forbidden history paths and require core policy/scaffold paths. |
| Integration | Bash installer dry-run reports core policy files and omits root README and numbered decisions. |
| E2E | Fresh install smoke creates the reusable operating kit without source-project history. |
| Platform | Shared manifest is consumed by both Bash and PowerShell installers; Bash syntax and local macOS execution pass. |
| Release | Not required; no CLI binary or schema change. |

## Harness Delta

Clarifies the installer boundary between reusable Harness operating files and
project-generated operational history, with a mechanical guard against future
payload drift.

## Evidence

No implementation evidence yet; the story is planned.
