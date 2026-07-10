# macOS Homebrew Harness Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a macOS-only Homebrew formula that installs a trusted global
`harness` bootstrap command, initializes projects from a versioned local kit,
and safely updates managed Harness files.

**Architecture:** The source repository builds immutable arm64 and x64 kit
archives containing a small Bash launcher, the existing payload, and a verified
repo-local CLI binary. A Formula in `winterzxzz/homebrew-tap` pins those archive
checksums and installs the launcher globally. `harness init` delegates to the
packaged installer; `harness update` uses an ignored hash-state file to update
only files still equal to their last Harness-managed version.

**Tech Stack:** Bash 3.2+, PowerShell, Rust release artifacts, GitHub Actions,
GitHub Releases, Homebrew Formulae, GitHub CLI.

---

## File Structure

- `scripts/install-harness.sh`: keep generic Bash installation behavior; add
  a verified local CLI input, installer-state recording, and explicit update
  mode.
- `scripts/install-harness.ps1`: correct the upstream repository defaults;
  its behavior otherwise remains unchanged because Homebrew is macOS-only.
- `scripts/harness-upstream-repository`: the single source-controlled GitHub
  owner/repository identifier used by local installer and packaging paths.
- `scripts/harness-kit-version`: source-controlled kit version used for
  installer state and release tags.
- `scripts/harness`: global Bash launcher packaged by Homebrew; maps `init`,
  `update`, `--init`, `--version`, and `--help` to the local kit.
- `scripts/build-harness-macos-kit.sh`: builds one self-contained archive for
  each macOS architecture.
- `scripts/render-homebrew-formula.sh`: writes the exact Formula from a kit
  tag, two computed SHA-256 values, and either the production release base URL
  or a local `file://` test base URL.
- `scripts/validate-install-payload.sh`: extends existing fresh/merge smoke
  coverage for local CLI verification and installer state.
- `scripts/validate-harness-macos-kit.sh`: tests the assembled local kit and
  launcher without requiring a published release.
- `packaging/homebrew/Formula/harness.rb.tmpl`: the checked-in formula template
  rendered into the external tap repository.
- `.github/workflows/harness-kit-release.yml`: publishes versioned macOS kit
  archives and updates the tap when a scoped credential is configured.
- `.github/workflows/post-merge-maintenance.yml`: detects kit inputs, bumps
  the kit version, tags the release, and orders CLI then kit publication.
- `docs/stories/US-086-macos-homebrew-bootstrap.md`: normal-lane story and
  verification contract.
- `README.md` and `scripts/README.md`: present Homebrew as the primary macOS
  installation/update path while retaining corrected direct-install fallbacks.

### Task 1: Record the normal-lane work packet

**Files:**
- Create: `docs/stories/US-086-macos-homebrew-bootstrap.md`
- Modify: `docs/TEST_MATRIX.md`

- [ ] **Step 1: Create the story from the normal-story template**

  Copy the template structure and use this contract:

  ```markdown
  # US-086 macOS Homebrew Harness Bootstrap

  ## Status

  planned

  ## Lane

  normal

  ## Product Contract

  A macOS user can install a versioned global `harness` command with Homebrew,
  initialize a project without executing a remote shell script, and explicitly
  update only unchanged Harness-managed files.
  ```

  Add acceptance criteria for `harness init`, the `--init` alias, safe conflict
  prompts, `harness update`, arm64/x64 Formula archives, checksums, the
  `Brewfile` flow, and the repository-local CLI contract.

- [ ] **Step 2: Add a durable matrix row and initialize the database record**

  Add `US-086` with `unit`, `integration`, `e2e`, and `platform` set to `no` in
  `docs/TEST_MATRIX.md`. Then run:

  ```bash
  scripts/bin/harness-cli story add \
    --id US-086 \
    --title "macOS Homebrew Harness bootstrap" \
    --lane normal \
    --contract "docs/stories/US-086-macos-homebrew-bootstrap.md" \
    --verify "scripts/validate-harness-macos-kit.sh"
  ```

  Expected: the command reports that `US-086` was created.

- [ ] **Step 3: Verify the story is visible**

  Run:

  ```bash
  scripts/bin/harness-cli query matrix
  ```

  Expected: one `US-086` row appears with all proof flags unset.

- [ ] **Step 4: Commit the planning packet**

  ```bash
  git add docs/stories/US-086-macos-homebrew-bootstrap.md docs/TEST_MATRIX.md
  git commit -m "docs: add homebrew bootstrap story"
  ```

### Task 2: Make installer origin and CLI sourcing explicit and testable

**Files:**
- Create: `scripts/harness-upstream-repository`
- Modify: `scripts/install-harness.sh:4-48,562-669,757-910`
- Modify: `scripts/install-harness.ps1:1-11,261-357`
- Modify: `README.md:63-158`
- Modify: `scripts/README.md:100-173`
- Modify: `scripts/validate-install-payload.sh:24-123`

- [ ] **Step 1: Add failing installer smoke checks**

  Extend `scripts/validate-install-payload.sh` with a local binary fixture that
  verifies `HARNESS_CLI_BINARY_PATH` and `HARNESS_CLI_CHECKSUM_PATH` are used
  without invoking `curl`:

  ```bash
  LOCAL_TARGET="$TMP_DIR/local-cli-target"
  HARNESS_CLI_BINARY_PATH="$CLI_SOURCE" \
  HARNESS_CLI_CHECKSUM_PATH="$RELEASE_DIR/harness-cli-test-platform.sha256" \
  HARNESS_CLI_PLATFORM=test-platform \
    "$ROOT_DIR/scripts/install-harness.sh" --directory "$LOCAL_TARGET" --yes >/dev/null

  cmp "$CLI_SOURCE" "$LOCAL_TARGET/scripts/bin/harness-cli" ||
    fail "local CLI source was not copied"
  ```

  Also assert that `hoangnb24/repository-harness` is absent from Bash,
  PowerShell, and public installation docs.

- [ ] **Step 2: Run the focused validator and observe failure**

  Run:

  ```bash
  scripts/validate-install-payload.sh
  ```

  Expected: FAIL because the installer ignores the local CLI input and still
  contains the former upstream repository identifier.

- [ ] **Step 3: Implement local CLI input and one upstream configuration**

  Create `scripts/harness-upstream-repository` containing exactly:

  ```text
  winterzxzz/repository-harness
  ```

  In `scripts/install-harness.sh`, add `read_upstream_repository`, which reads
  the first non-comment line from that file in local-source mode and otherwise
  falls back to the same current repository identifier for the direct remote
  bootstrap path. Add these environment inputs before
  `install_harness_cli_binary` runs:

  ```bash
  HARNESS_UPSTREAM_REPOSITORY="${HARNESS_UPSTREAM_REPOSITORY:-$(read_upstream_repository)}"
  LOCAL_CLI_BINARY_PATH="${HARNESS_CLI_BINARY_PATH:-}"
  LOCAL_CLI_CHECKSUM_PATH="${HARNESS_CLI_CHECKSUM_PATH:-}"
  ```

  Change the default URLs to interpolate `HARNESS_UPSTREAM_REPOSITORY`:

  ```bash
  SOURCE_BASE_URL="${HARNESS_SOURCE_BASE_URL:-https://raw.githubusercontent.com/$HARNESS_UPSTREAM_REPOSITORY/main}"

  printf 'https://github.com/%s/releases/download/%s\n' \
    "$HARNESS_UPSTREAM_REPOSITORY" "$release_tag"
  ```

  In `install_harness_cli_binary`, choose the supplied local paths only when
  both variables are non-empty, require both files to exist, copy the binary to
  the temporary directory, read the supplied checksum, and run the existing
  `sha256_file` comparison. Otherwise preserve the current download behavior.
  Do not require `curl` on the local-path branch.

  Mirror the repository-default correction in PowerShell with a
  `Read-UpstreamRepository` function that reads the same file from a local
  source root, then falls back only for the direct remote bootstrap case:

  ```powershell
  $script:UpstreamRepository = if ($env:HARNESS_UPSTREAM_REPOSITORY) {
      $env:HARNESS_UPSTREAM_REPOSITORY
  } else {
      Read-UpstreamRepository
  }
  $script:SourceBaseUrl = if ($env:HARNESS_SOURCE_BASE_URL) {
      $env:HARNESS_SOURCE_BASE_URL.TrimEnd("/")
  } else {
      "https://raw.githubusercontent.com/$($script:UpstreamRepository)/main"
  }
  ```

  Use `$script:UpstreamRepository` in `Get-DefaultCliBaseUrl`. Replace all
  documented remote URLs with `winterzxzz/repository-harness`.

- [ ] **Step 4: Run installer validation**

  Run:

  ```bash
  bash -n scripts/install-harness.sh
  scripts/validate-install-payload.sh
  rg -n "hoangnb24/repository-harness" README.md scripts || true
  ```

  Expected: syntax and payload validation pass; the final search has no output.

- [ ] **Step 5: Commit the installer-source change**

  ```bash
  git add README.md scripts/harness-upstream-repository scripts/install-harness.sh scripts/install-harness.ps1 scripts/README.md scripts/validate-install-payload.sh
  git commit -m "fix(installer): use current upstream and local CLI inputs"
  ```

### Task 3: Add safe, explicit project updates

**Files:**
- Create: `scripts/harness-kit-version`
- Modify: `scripts/install-harness.sh:104-190,613-669,757-921`
- Modify: `scripts/validate-install-payload.sh:92-123`
- Modify: `.gitignore:20-26`

- [ ] **Step 1: Write update-mode smoke cases**

  Add three temporary-target tests to `scripts/validate-install-payload.sh`:

  ```bash
  UPDATE_TARGET="$TMP_DIR/update-target"
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$UPDATE_TARGET" --yes \
    --dry-run >/dev/null

  # A managed file left unchanged is replaced by --update.
  # A managed file edited after install is skipped by --update.
  # The same edited file is backed up then replaced by --update --force.
  ```

  The assertions must check for `.harness/install-state.tsv`, the updated file
  content, the preserved custom content, and the backup file under
  `.harness-backup/`.

- [ ] **Step 2: Run the update smoke tests and observe failure**

  Run:

  ```bash
  scripts/validate-install-payload.sh
  ```

  Expected: FAIL because `--update` and installer state do not yet exist.

- [ ] **Step 3: Implement state recording and update mode**

  Add `scripts/harness-kit-version` with the initial value:

  ```text
  0.1.0
  ```

  Add `--update` and `--adopt` parsing in `install-harness.sh`. Keep the state
  in `$TARGET_DIR/.harness/install-state.tsv` with this exact format:

  ```text
  version	0.1.0
  file	AGENTS.md	<sha256>
  file	docs/HARNESS.md	<sha256>
  file	scripts/bin/harness-cli	<sha256>
  ```

  Add shell functions with these contracts:

  ```bash
  state_file()                 # prints "$TARGET_DIR/.harness/install-state.tsv"
  record_managed_file PATH     # appends "file<TAB>PATH<TAB>SHA" after a successful copy
  write_install_state          # atomically writes version plus recorded entries
  read_recorded_hash PATH      # prints the stored SHA or an empty string
  update_managed_file PATH     # replaces only when target SHA equals stored SHA
  adopt_existing_files         # records hashes without copying or overwriting
  ```

  `--update` must fail with `Run 'harness update --adopt' to begin tracking this
  legacy installation.` when no state file exists. `--adopt` records only
  existing manifest files and the target CLI; it must not copy files. In update
  mode, a changed target logs `skip ... (modified locally)` and remains tracked
  at its previous hash. `--force` first copies it to the existing timestamped
  backup directory, then writes the new source and state hash. `--dry-run`
  prints actions but writes neither state nor backups.

  Preserve first-install behavior: fresh installs write state after successful
  copies; `--merge` records only new files and never adopts pre-existing paths.
  `.harness/install-state.tsv` remains ignored by the existing `.harness/*`
  rule and is explicitly documented as installer metadata, not run history.

- [ ] **Step 4: Run installer regression coverage**

  Run:

  ```bash
  bash -n scripts/install-harness.sh
  scripts/validate-install-payload.sh
  ```

  Expected: fresh install, merge, local binary, update, modified-file skip, and
  forced-backup assertions pass.

- [ ] **Step 5: Commit safe project updates**

  ```bash
  git add .gitignore scripts/harness-kit-version scripts/install-harness.sh scripts/validate-install-payload.sh
  git commit -m "feat(installer): add safe managed-file updates"
  ```

### Task 4: Build and verify the global macOS launcher and kit

**Files:**
- Create: `scripts/harness`
- Create: `scripts/build-harness-macos-kit.sh`
- Create: `scripts/validate-harness-macos-kit.sh`

- [ ] **Step 1: Write launcher and kit validation first**

  `scripts/validate-harness-macos-kit.sh` must create a temporary kit using the
  host architecture and assert all of these commands:

  ```bash
  "$KIT/bin/harness" --help
  "$KIT/bin/harness" --version
  "$KIT/bin/harness" init --dry-run --yes "$TARGET"
  "$KIT/bin/harness" --init --dry-run --yes "$TARGET"
  "$KIT/bin/harness" init --yes "$TARGET"
  "$KIT/bin/harness" update --dry-run --yes "$TARGET"
  ```

  It must also remove the packaged checksum, rerun `init`, and assert a
  non-zero result containing `Local Harness CLI checksum file is missing`.

- [ ] **Step 2: Run the kit validator and observe failure**

  Run:

  ```bash
  scripts/validate-harness-macos-kit.sh
  ```

  Expected: FAIL because no launcher or kit builder exists.

- [ ] **Step 3: Implement the launcher**

  Create executable `scripts/harness` with this command dispatch:

  ```bash
  case "${1:---help}" in
    init|--init)
      shift
      exec "$KIT_ROOT/scripts/install-harness.sh" "$@"
      ;;
    update)
      shift
      exec "$KIT_ROOT/scripts/install-harness.sh" --update "$@"
      ;;
    --version)
      cat "$KIT_ROOT/scripts/harness-kit-version"
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      fail "Unknown command: $1"
      ;;
  esac
  ```

  Before dispatching, calculate `KIT_ROOT` from the launcher location, require
  the packaged installer, version file, CLI binary, and checksum file, then
  export `HARNESS_CLI_BINARY_PATH` and `HARNESS_CLI_CHECKSUM_PATH`. The launcher
  must use `exec`, quote every path, and never invoke a network command.

- [ ] **Step 4: Implement deterministic kit assembly**

  Create `scripts/build-harness-macos-kit.sh` with required arguments:

  ```text
  --platform <macos-arm64|macos-x64>
  --cli <path-to-harness-cli-binary>
  --out-dir <directory>
  ```

  It must reject another platform, copy only the files named by
  `scripts/harness-install-files.txt`, discover `scripts/schema/*.sql`, copy
  `.gitignore`, installer files required by `harness`, `harness-kit-version`,
  `harness-upstream-repository`, the launcher, the supplied CLI binary, and a
  SHA-256 file generated from that binary. It writes `harness-macos-<arch>.tar.gz` with a root containing
  `bin/harness` and `libexec/harness-kit`; use `tar -czf` only after validating
  every required source file exists.

- [ ] **Step 5: Run kit and payload validation**

  Run:

  ```bash
  bash -n scripts/harness scripts/build-harness-macos-kit.sh scripts/validate-harness-macos-kit.sh
  scripts/validate-install-payload.sh
  scripts/validate-harness-macos-kit.sh
  ```

  Expected: both validators pass; no target receives source-repository history.

- [ ] **Step 6: Commit kit assembly**

  ```bash
  git add scripts/harness scripts/build-harness-macos-kit.sh scripts/validate-harness-macos-kit.sh
  git commit -m "feat(packaging): build macos harness kits"
  ```

### Task 5: Publish immutable kit releases and update the tap

**Files:**
- Create: `.github/workflows/harness-kit-release.yml`
- Modify: `.github/workflows/post-merge-maintenance.yml:1-137`
- Create: `scripts/render-homebrew-formula.sh`
- Create: `packaging/homebrew/Formula/harness.rb.tmpl`

- [ ] **Step 1: Write offline renderer and workflow structure checks**

  Add a shell test section to `scripts/validate-harness-macos-kit.sh` that runs:

  ```bash
  scripts/render-homebrew-formula.sh \
    --kit-version 0.1.0 \
    --base-url "file://$TMP_DIR" \
    --arm-sha 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
    --intel-sha fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210 \
    --output "$TMP_DIR/harness.rb"

  ruby -c "$TMP_DIR/harness.rb"
  grep -Fq 'harness-macos-arm64.tar.gz' "$TMP_DIR/harness.rb"
  grep -Fq 'harness-macos-x64.tar.gz' "$TMP_DIR/harness.rb"
  ```

  The test must also parse both workflow YAML files with Ruby YAML and assert
  the kit workflow references both archive names and `HOMEBREW_TAP_TOKEN`.

- [ ] **Step 2: Run the checks and observe failure**

  Run:

  ```bash
  scripts/validate-harness-macos-kit.sh
  ```

  Expected: FAIL because the renderer and kit release workflow are absent.

- [ ] **Step 3: Implement formula rendering**

  Make the renderer replace these exact template variables:

  ```text
  @KIT_VERSION@
  @KIT_BASE_URL@
  @ARM_SHA256@
  @INTEL_SHA256@
  ```

  The rendered Formula must follow this shape, using the architecture blocks
  supported by Homebrew Formulae:

  ```ruby
  class Harness < Formula
    desc "Install and update the repository Harness operating kit"
    homepage "https://github.com/winterzxzz/repository-harness"
    version "@KIT_VERSION@"

    on_macos do
      on_arm do
        url "@KIT_BASE_URL@/harness-macos-arm64.tar.gz"
        sha256 "@ARM_SHA256@"
      end
      on_intel do
        url "@KIT_BASE_URL@/harness-macos-x64.tar.gz"
        sha256 "@INTEL_SHA256@"
      end
    end

    def install
      libexec.install Dir["*"]
      bin.install_symlink libexec/"bin/harness"
    end

    test do
      assert_match "Usage:", shell_output("#{bin}/harness --help")
    end
  end
  ```

  The renderer defaults `--base-url` to
  `https://github.com/winterzxzz/repository-harness/releases/download/harness-kit-v<version>`.
  It must accept only HTTPS GitHub release URLs or absolute `file://` URLs for
  local tests, reject an invalid version or checksum, and write atomically to
  the requested output path.

- [ ] **Step 4: Implement kit release workflow and ordered release dispatch**

  In `.github/workflows/harness-kit-release.yml`, verify the workspace, resolve
  the current CLI release named by `scripts/harness-cli-release-tag`, build both
  kit archives with the native macOS runners, verify them with
  `scripts/validate-harness-macos-kit.sh`, and publish the archives plus
  `.sha256` files to tag `harness-kit-v<version>`.

  In the publish job, only when `HOMEBREW_TAP_TOKEN` is non-empty, clone
  `https://x-access-token:${HOMEBREW_TAP_TOKEN}@github.com/winterzxzz/homebrew-tap.git`,
  render `Formula/harness.rb` from the uploaded checksum values, run:

  ```bash
  brew audit --strict --online Formula/harness.rb
  HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source Formula/harness.rb
  ```

  Then commit and push the formula update. When the credential is absent, log a
  single manual-bump instruction and leave the release successful.

  Extend post-merge maintenance to calculate `kit_changed` from the installer,
  payload manifest, templates, schemas, kit scripts, packaging template, and
  CLI release tag. Bump `scripts/harness-kit-version`, create
  `harness-kit-v<version>`, and invoke kit release only after any required CLI
  release completes.

- [ ] **Step 5: Run static release checks**

  Run:

  ```bash
  ruby -e 'require "yaml"; ARGV.each { |f| YAML.load_file(f); puts "ok #{f}" }' \
    .github/workflows/harness-cli-release.yml \
    .github/workflows/harness-kit-release.yml \
    .github/workflows/post-merge-maintenance.yml
  scripts/validate-harness-macos-kit.sh
  ```

  Expected: all three YAML files parse and the kit validator passes.

- [ ] **Step 6: Commit release automation**

  ```bash
  git add .github/workflows/harness-kit-release.yml .github/workflows/post-merge-maintenance.yml packaging/homebrew/Formula/harness.rb.tmpl scripts/render-homebrew-formula.sh scripts/validate-harness-macos-kit.sh
  git commit -m "ci: publish macos harness kits"
  ```

### Task 6: Create and verify the public Homebrew tap

**Files:**
- Create in `winterzxzz/homebrew-tap`: `Formula/harness.rb`
- Create in `winterzxzz/homebrew-tap`: `README.md`
- Create in `winterzxzz/homebrew-tap`: `.github/workflows/formula.yml`

**Precondition:** Complete this task only after the source-repository pull
request is merged and GitHub Actions has published the first immutable
`harness-kit-v<version>` release. A feature-branch formula must not point at a
nonexistent or mutable artifact.

- [ ] **Step 1: Verify GitHub authentication and repository availability**

  Run:

  ```bash
  gh auth status
  gh repo view winterzxzz/homebrew-tap
  ```

  Expected: authentication has permission to create public repositories; the
  second command reports that the tap does not yet exist.

- [ ] **Step 2: Create the public tap and clone it into a temporary directory**

  Run:

  ```bash
  gh repo create winterzxzz/homebrew-tap --public --description "Homebrew formulae for winterzxzz tools"
  TAP_DIR="$(mktemp -d)/homebrew-tap"
  git clone git@github.com:winterzxzz/homebrew-tap.git "$TAP_DIR"
  ```

  Expected: the repository is public and the local clone has an `origin` remote
  for `winterzxzz/homebrew-tap`.

- [ ] **Step 3: Install the rendered Formula and tap CI**

  Render `Formula/harness.rb` from the first published kit release checksums.
  Create a workflow that runs `brew audit --strict --online Formula/harness.rb`
  and `HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source
  Formula/harness.rb` on pull requests and pushes. The README must show:

  ```bash
  brew install winterzxzz/tap/harness
  harness init
  brew update && brew upgrade harness
  harness update
  ```

- [ ] **Step 4: Validate and publish the tap**

  Run:

  ```bash
  brew audit --strict --online Formula/harness.rb
  HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source Formula/harness.rb
  harness --version
  brew uninstall harness
  ```

  Expected: formula audit and local install pass; `harness --version` reports
  the kit version; uninstall removes only the Homebrew-managed kit.

- [ ] **Step 5: Commit and push the tap contents**

  ```bash
  git add Formula/harness.rb README.md .github/workflows/formula.yml
  git commit -m "feat: add harness formula"
  git push -u origin main
  ```

### Task 7: Document the multi-Mac workflow and close validation

**Files:**
- Modify: `README.md:63-158`
- Modify: `scripts/README.md:100-247`
- Modify: `docs/stories/US-086-macos-homebrew-bootstrap.md`
- Modify: `docs/TEST_MATRIX.md`

- [ ] **Step 1: Write documentation assertions**

  Add these checks to the end of `scripts/validate-harness-macos-kit.sh`:

  ```bash
  rg -Fq 'brew install winterzxzz/tap/harness' README.md
  rg -Fq 'brew bundle' README.md
  rg -Fq 'harness update' README.md
  rg -Fq 'scripts/bin/harness-cli' README.md
  ```

- [ ] **Step 2: Update public guides**

  Make Homebrew the first macOS path. Explain the `Brewfile` entry, the distinct
  `brew upgrade harness` and per-project `harness update` actions, the safe
  `--merge` and `--override` behavior, and the direct Bash/PowerShell fallback
  paths using the corrected `winterzxzz` URLs. State that agents continue to
  use the installed project-local `scripts/bin/harness-cli`.

- [ ] **Step 3: Run the complete local validation matrix**

  Run:

  ```bash
  cargo fmt --check
  cargo test --workspace
  cargo clippy --workspace -- -D warnings
  bash -n scripts/install-harness.sh scripts/harness scripts/build-harness-macos-kit.sh scripts/render-homebrew-formula.sh scripts/validate-harness-macos-kit.sh
  scripts/validate-install-payload.sh
  scripts/validate-harness-macos-kit.sh
  git diff --check
  ```

  Expected: every command exits zero. If an Apple Silicon or Intel kit cannot
  be built on the local host, record it in the story and rely on the matching
  GitHub Actions runner before marking platform proof complete.

- [ ] **Step 4: Record evidence and proof flags**

  Update `US-086` with every command and result. Then run:

  ```bash
  scripts/bin/harness-cli story update \
    --id US-086 \
    --status implemented \
    --unit 1 \
    --integration 1 \
    --e2e 1 \
    --platform 1 \
    --evidence "scripts/validate-install-payload.sh; scripts/validate-harness-macos-kit.sh; Homebrew formula install smoke"
  ```

- [ ] **Step 5: Commit documentation and validation evidence**

  ```bash
  git add README.md scripts/README.md docs/stories/US-086-macos-homebrew-bootstrap.md docs/TEST_MATRIX.md scripts/validate-harness-macos-kit.sh
  git commit -m "docs: document homebrew harness workflow"
  ```
