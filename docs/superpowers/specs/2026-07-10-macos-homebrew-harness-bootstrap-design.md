# macOS Homebrew Harness Bootstrap Design

## Intent

Make Harness quick to install and update across multiple Macs without relying
on a pasted `curl | bash` command. A person should install one trusted
Homebrew formula, then use a short command inside each project.

## Scope

### In scope

- A public `winterzxzz/homebrew-tap` repository with a `harness` formula.
- macOS Apple Silicon and Intel distributions only.
- A global `harness` bootstrap command supplied by the formula.
- Safe project initialization, update, dry-run, and conflict handling.
- Versioned artifacts, checksums, release automation, and documented
  multi-Mac setup.
- Correcting the old upstream repository URLs in installer code and docs.

### Out of scope

- Linux, Windows, npm, Cargo, or Homebrew core distribution.
- Replacing the repository-local `scripts/bin/harness-cli` command used by
  agents after project setup.
- Automatic discovery or bulk modification of every project on a Mac.
- Silent replacement of user-modified Harness files.

## User Experience

On every Mac, installation is one command:

```bash
brew install winterzxzz/tap/harness
```

Homebrew taps `winterzxzz/homebrew-tap` automatically. A user then enters a
project and initializes Harness:

```bash
cd /path/to/project
harness init
```

`harness --init` remains a compatibility alias. `harness init /path/to/project`
selects another target. It accepts the existing installer flags:

```text
--merge  --override  --yes  --dry-run  --claude  --refresh-agent-shim
```

Conflicting `AGENTS.md`, `docs/`, or `scripts/` paths keep the existing safe
behavior: interactive runs offer Merge, Override, or Stop; non-interactive
runs stop unless `--merge` or `--override` is explicit.

For several Macs, the recommended `Brewfile` entry is:

```ruby
tap "winterzxzz/tap"
brew "harness"
```

Running `brew bundle` installs the same formula on each machine. A global kit
updates with `brew update && brew upgrade harness`.

## Distribution Architecture

Each Harness release produces two immutable, checksummed macOS kit archives:

```text
harness-macos-arm64.tar.gz
harness-macos-x64.tar.gz
```

Each archive contains:

```text
bin/harness                         # Global bootstrap launcher
libexec/harness-kit/                # Versioned, local installer source
  AGENTS.md
  .gitignore
  docs/
  scripts/install-harness.sh
  scripts/harness-install-files.txt
  scripts/schema/
  scripts/bin/harness-cli
  scripts/bin/harness-cli.sha256
```

The Homebrew formula selects the archive for the host architecture and pins
both its version and SHA-256. It installs `bin/harness` and the local kit under
the formula's `libexec`; Homebrew owns updating and removal.

The launcher invokes the packaged `install-harness.sh`, not a URL on `main`.
It supplies the packaged CLI binary and checksum through explicit local-path
arguments. The installer verifies that checksum before copying the binary into
the target project's `scripts/bin/harness-cli`. Therefore, after
`brew install`, `harness init` does not fetch payload files or executable code
from the network.

The source repository remains the canonical source of policies, templates,
installer code, and release artifacts. The new tap repository contains only
`Formula/harness.rb`, a concise README, and formula-specific CI.

## Project Updates

`harness init` is for first setup and preserves its current additive merge
semantics. It does not overwrite pre-existing files.

`harness update` is the explicit, safe project-update path:

1. Initial installation records a small, ignored installer-state file with the
   kit version and hashes for files created by Harness. It is installer metadata,
   not a database, changeset, or run history.
2. An update replaces a managed file only when its current hash matches the
   previously recorded Harness hash.
3. A user-modified managed file is left untouched and reported. `--force`
   creates a timestamped backup before replacing it.
4. New managed files are added; no target-owned file is deleted.
5. Symlinked managed paths are rejected before an update can follow them
   outside the target project.
6. Legacy installations without state receive additive `--merge` behavior and
   a clear `--adopt` path that begins tracking only confirmed Harness-owned
   files without overwriting them.

This separates a safe global update (`brew upgrade harness`) from a deliberate
per-project update (`harness update`). A user can update projects whenever
appropriate rather than allowing a package upgrade to mutate repositories
silently.

## Release and Tap Flow

The install kit has its own version and release tag, for example
`harness-kit-v0.1.0`. Its version changes when any packaged policy, template,
schema migration, installer, launcher, package manifest, or bundled CLI version
changes. This is separate from the Rust CLI version because a policy-only
change must still reach Homebrew users.

1. A merged kit-worthy change validates the Rust CLI and installer inputs, then
   bumps the kit version and creates an immutable kit release tag.
2. The kit release workflow assembles the two macOS kit archives, computes
   checksums, and uploads them to that tag. It bundles the CLI version named by
   `scripts/harness-cli-release-tag`.
3. A release automation job updates `Formula/harness.rb` in
   `winterzxzz/homebrew-tap` with the tag and architecture-specific checksums.
4. Before pushing the formula change, the release job installs it from the tap
   on a macOS runner, checks `harness --version`, runs `harness init --dry-run`,
   and uninstalls it.

Cross-repository publishing requires a narrowly scoped GitHub token or GitHub
App credential stored as `HOMEBREW_TAP_TOKEN` in the source repository. It
needs access only to update the tap repository. Until that credential exists,
the release process documents the manual formula bump; it must never fall back
to a mutable `latest` URL.

The installer receives one repository configuration source, rather than
hard-coded owner names in multiple files. Bash, PowerShell, README examples,
and CLI release URLs must consistently resolve to
`winterzxzz/repository-harness`.

## Error Handling and Safety

- Formula installation fails before writing when Homebrew's archive checksum
  does not match.
- `harness init` fails before copying the target CLI if the bundled binary
  checksum fails.
- Unsupported architectures return a clear macOS-only error.
- The launcher validates its packaged-kit paths before invoking the installer.
- `--dry-run` never writes target files or installer state.
- No release token, Homebrew credential, or target path is printed in normal
  output.
- The global bootstrap never runs a remote shell script; users can inspect the
  formula and installed kit with ordinary Homebrew commands.

## Validation

| Layer | Proof |
| --- | --- |
| Unit | Test launcher argument mapping, `--init` aliasing, managed-file hash comparison, and local CLI checksum rejection. |
| Integration | Assemble both kit layouts; run `harness init --dry-run`, fresh init, safe merge, shim refresh, legacy adoption, and update conflict cases in temporary projects. |
| Formula | Run `brew audit --strict Formula/harness.rb` and `brew install --build-from-source` against the local formula for native architecture. |
| E2E | In a clean macOS environment, install from the tap, initialize a fresh project, run its repo-local `scripts/bin/harness-cli --version`, upgrade the formula, and safely update the project. |
| Release | Confirm release archives and checksums match, the formula pins their immutable tag, and tap CI installs the released formula. |

## Rollout

1. Publish the initial formula and a release with both macOS kits.
2. Document `brew install winterzxzz/tap/harness` as the primary macOS flow.
3. Keep the existing direct installers as an advanced/fallback path while their
   URLs are corrected.
4. Announce `harness update` for project updates; do not deprecate repository-
   local `scripts/bin/harness-cli`.

## Decisions

- macOS-only is intentional for the first release.
- The Homebrew formula is the global bootstrap distribution; Harness behavior
  inside projects remains repository-local.
- Homebrew formula and kit releases are version-pinned and checksummed.
- Project update is explicit and content-aware; it never silently overwrites
  local work.
