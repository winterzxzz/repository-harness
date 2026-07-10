# US-086 macOS Homebrew Harness Bootstrap

## Status

in_progress

## Lane

normal

## Product Contract

A macOS user can install a versioned global `harness` command with Homebrew,
initialize a project without executing a remote shell script, and explicitly
update only unchanged Harness-managed files.

## Relevant Product Docs

- `README.md`
- `scripts/README.md`
- `docs/decisions/0005-prebuilt-rust-harness-cli.md`
- `docs/superpowers/specs/2026-07-10-macos-homebrew-harness-bootstrap-design.md`

## Acceptance Criteria

- `brew install winterzxzz/tap/harness` installs `harness` on macOS arm64 and x64.
- `harness init` installs the packaged Harness kit into the current directory;
  `harness --init` is a compatible alias.
- `harness init` preserves the existing interactive Merge, Override, and Stop
  choices; non-interactive conflicts require `--merge` or `--override`.
- The packaged launcher verifies its local CLI checksum and does not download
  payload files or executable code after Homebrew installation.
- `harness update` updates only Harness-managed files whose hashes still match
  the recorded installer state; modified files remain untouched unless
  `--force` creates a backup first.
- The Homebrew Formula pins versioned arm64 and x64 kit archives with SHA-256.
- `brew bundle` can install the same formula on multiple Macs from a Brewfile.
- Agents continue using `scripts/bin/harness-cli` inside initialized projects.

## Design Notes

- Commands: `harness init`, `harness update`, `brew upgrade harness`.
- Queries: `target/debug/harness-cli query matrix` in the source checkout.
- API: GitHub Releases and the Homebrew tap repository only during release.
- Tables: installer state is an ignored TSV file, not a durable Harness database table.
- Domain rules: project updates never overwrite locally modified managed files
  unless the operator passes `--force`.
- UI surfaces: Bash prompts, Homebrew output, and README instructions.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | Launcher parsing, checksum, and managed-file hash tests pass. |
| Integration | Fresh install, merge, update, adopt, modified-file, and backup smoke checks pass. |
| E2E | Homebrew installs a released kit, initializes a project, upgrades, and safely updates it. |
| Platform | arm64 and x64 kit archives install on their matching macOS runners. |
| Release | Formula pins release checksums and Homebrew audit passes. |

## Harness Delta

Adds a versioned macOS distribution channel while retaining the existing
repository-local Harness operating model.

## Evidence

- Pending implementation and release validation.
