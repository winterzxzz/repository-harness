# Release Runbook (Manual)

Source-repo-only document; it is intentionally not part of
`scripts/harness-install-files.txt`.

GitHub Actions on this repository is locked by a billing issue, so **manual
local release is the default path** until that is resolved. The
`harness-cli-release.yml` and `harness-kit-release.yml` workflows exist but
cannot run. When the user asks to release, publish, or "update the version",
follow this runbook end to end without asking for the process.

## 1. Decide which releases are needed

- **Harness CLI release** (`harness-cli-vA.B.C`): required when
  `crates/harness-cli/**` or `scripts/schema/**` changed since the last
  `harness-cli-v*` tag.
- **Harness kit release** (`harness-kit-vX.Y.Z`): required for changes under
  `crates/harness-symphony/**` (including `web-ui/`), `scripts/harness*`,
  installers, or when a new CLI release must be bundled.

## 2. Prepare versions

1. If the CLI changed: bump `version` in `crates/harness-cli/Cargo.toml` and
   write the matching tag into `scripts/harness-cli-release-tag`
   (`harness-cli-v<version>`); they must agree —
   `scripts/validate-harness-cli-release.sh` checks this.
2. Bump `scripts/harness-kit-version` (plain `X.Y.Z`).
3. Add a CHANGELOG entry titled `## <date> - Manual release harness-kit-vX.Y.Z`
   summarizing shipped changes; state either the new CLI release or "Keep the
   bundled Harness CLI at harness-cli-vA.B.C".
4. `cargo build -p harness-cli` (refreshes `Cargo.lock`), then
   `cargo test -p harness-cli` and `scripts/validate-install-payload.sh`.
5. Commit `chore(release): prepare ...`, create the tag(s), push `main` and
   the tags together.

## 3. Publish the CLI release (when needed)

```bash
rm -rf dist
scripts/build-harness-cli-release.sh --target aarch64-apple-darwin --out-dir dist
scripts/build-harness-cli-release.sh --target x86_64-apple-darwin --out-dir dist
scripts/validate-harness-cli-release.sh --artifact-dir dist
gh release create harness-cli-vA.B.C --title "harness-cli-vA.B.C" \
  --notes "<summary>" dist/harness-cli-macos-*
```

Linux and Windows binaries cannot be cross-built on this Mac (no cross/zig/
docker). Note the gap in the release notes; after the GitHub billing issue is
fixed, backfill with:

```bash
gh workflow run harness-cli-release.yml -f release_tag=harness-cli-vA.B.C \
  --ref harness-cli-vA.B.C
```

## 4. Publish the kit release

```bash
cargo build --release -p harness-symphony
cargo build --release -p harness-symphony --target x86_64-apple-darwin
cd crates/harness-symphony/web-ui && npm run build && cd -
scripts/build-harness-macos-kit.sh --platform macos-arm64 \
  --cli dist/harness-cli-macos-arm64 \
  --symphony target/release/harness-symphony --out-dir dist
scripts/build-harness-macos-kit.sh --platform macos-x64 \
  --cli dist/harness-cli-macos-x64 \
  --symphony target/x86_64-apple-darwin/release/harness-symphony --out-dir dist
(cd dist && shasum -a 256 harness-macos-arm64.tar.gz > harness-macos-arm64.tar.gz.sha256 \
          && shasum -a 256 harness-macos-x64.tar.gz > harness-macos-x64.tar.gz.sha256)
scripts/validate-harness-macos-kit.sh
gh release create harness-kit-vX.Y.Z --title "Harness macOS Kit vX.Y.Z" \
  --notes "<summary>" dist/harness-macos-*.tar.gz dist/harness-macos-*.tar.gz.sha256
```

The kit build reuses the CLI binaries from step 3; if no CLI release was
needed, download the pinned tag's assets instead:
`gh release download "$(cat scripts/harness-cli-release-tag)" --pattern 'harness-cli-macos-*' --dir dist`.

## 5. Update the Homebrew tap

```bash
git clone https://github.com/winterzxzz/homebrew-tap.git <tmp-dir>
scripts/render-homebrew-formula.sh --kit-version X.Y.Z \
  --arm-sha "$(awk '{print $1}' dist/harness-macos-arm64.tar.gz.sha256)" \
  --intel-sha "$(awk '{print $1}' dist/harness-macos-x64.tar.gz.sha256)" \
  --output <tmp-dir>/Formula/harness.rb
cd <tmp-dir> && git commit -am "chore: update harness to X.Y.Z" && git push origin HEAD:main
```

## 6. Upgrade and verify locally

```bash
brew update && brew upgrade harness
test "$(harness --version)" = "X.Y.Z"
harness-symphony doctor
```

Restart any long-running `harness-symphony web` servers so they pick up the
new binary.
