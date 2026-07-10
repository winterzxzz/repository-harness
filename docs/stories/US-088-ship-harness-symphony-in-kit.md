# US-088 Ship harness-symphony In The macOS Kit

## Status

in-progress

## Lane

normal

## Product Contract

Homebrew users get the `harness-symphony` binary on PATH from the same kit
that installs `harness`, so Symphony execution — including the US-087
auto-started Web UI — works in every Harness project without cloning this
source repository or building from source. The kit archive carries a
per-architecture release build of `harness-symphony` with a recorded SHA-256,
the Formula links it into `bin`, and the kit release workflow builds it on the
matching runner architecture.

## Relevant Product Docs

- `docs/product/symphony-web-ui-controller.md`
- `docs/stories/US-086-macos-homebrew-bootstrap.md` (distribution channel)

## Acceptance Criteria

- `scripts/build-harness-macos-kit.sh` requires a `--symphony <path>` binary,
  stages it at `bin/harness-symphony` in the archive, and records its SHA-256.
- The rendered Homebrew Formula symlinks `harness-symphony` into `bin` and its
  test block exercises `harness-symphony --help`.
- `harness-kit-release.yml` builds `harness-symphony` in release mode on each
  matrix runner and passes it to the kit build.
- `scripts/validate-harness-macos-kit.sh` fails when the kit archive, Formula,
  or workflow omits `harness-symphony`.
- `scripts/harness-kit-version` bumps to `0.2.0`.
- After `brew install winterzxzz/tap/harness` at v0.2.0, `harness-symphony
  --help` works from PATH on macOS.

## Design Notes

- Commands: `build-harness-macos-kit.sh --platform <p> --cli <path> --symphony <path> --out-dir <path>`.
- Domain rules: single copy of the binary lives at `bin/harness-symphony` in
  the stage tree (next to the `harness` launcher); the SHA-256 sidecar lives
  alongside it. Project-local installs of `harness-symphony` via `harness
  init` stay out of scope — PATH-level availability is the contract.
- API: GitHub Releases assets `harness-macos-{arm64,x64}.tar.gz` unchanged in
  name; contents gain `bin/harness-symphony`.

## Validation

When updating durable proof status, use numeric booleans:
`scripts/bin/harness-cli story update --id <id> --unit 1 --integration 1 --e2e 0 --platform 0`.

| Layer | Expected proof |
| --- | --- |
| Unit | `bash -n` on changed scripts; workflow YAML parses |
| Integration | `scripts/validate-harness-macos-kit.sh` passes with symphony staged; fails when omitted |
| E2E | Kit v0.2.0 installed via Homebrew tap exposes working `harness-symphony` |
| Platform | arm64 native verified; x64 archive cross-built and published |
| Release | `harness-kit-v0.2.0` release assets + tap formula checksums updated |

## Harness Delta

Extends the US-086 macOS distribution channel with the Symphony runner binary.

## Evidence

Pending implementation.
