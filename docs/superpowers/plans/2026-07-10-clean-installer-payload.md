# Clean Installer Payload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make fresh Harness installations copy only reusable operating-kit files and empty project scaffolds, never `repository-harness` task history or project identity.

**Architecture:** Keep `scripts/harness-install-files.txt` as the single allowlist consumed by both installers. Add one repository validation script that enforces required and forbidden payload classes, exercises a Bash dry run, performs a real fresh install with a local CLI fixture, and proves merge preservation.

**Tech Stack:** Bash, PowerShell manifest consumption, POSIX file utilities, Rust Harness CLI, Markdown documentation.

---

## File Map

- Create `scripts/validate-install-payload.sh`: executable contract test for manifest boundaries and installer behavior.
- Modify `scripts/harness-install-files.txt`: remove source-project identity and numbered decision history while retaining reusable policies and scaffolds.
- Modify `scripts/README.md`: document the operating-kit/project-data boundary and the validation command.
- Modify `README.md`: explain clean fresh-install behavior to users.
- Modify `docs/stories/US-083-clean-installer-payload.md`: record final validation evidence and implemented status.
- Modify the local Harness durable row for `US-083`: record proof status and trace through `scripts/bin/harness-cli`.

### Task 1: Add The Failing Payload Contract

**Files:**
- Create: `scripts/validate-install-payload.sh`
- Test: `scripts/validate-install-payload.sh`

- [ ] **Step 1: Create the payload validation script**

Create `scripts/validate-install-payload.sh` with this content:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
MANIFEST="$ROOT_DIR/scripts/harness-install-files.txt"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
  printf 'install payload validation failed: %s\n' "$1" >&2
  exit 1
}

require_entry() {
  grep -Fxq "$1" "$MANIFEST" || fail "required payload entry missing: $1"
}

reject_pattern() {
  if grep -Eq "$1" "$MANIFEST"; then
    fail "forbidden payload pattern present: $1"
  fi
}

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

for required in \
  AGENTS.md \
  docs/ARCHITECTURE.md \
  docs/CONTEXT_RULES.md \
  docs/FEATURE_INTAKE.md \
  docs/HARNESS.md \
  docs/HARNESS_BACKLOG.md \
  docs/TEST_MATRIX.md \
  docs/decisions/README.md \
  docs/product/README.md \
  docs/stories/README.md \
  docs/stories/backlog.md \
  docs/templates/decision.md \
  scripts/README.md \
  .gitignore
do
  require_entry "$required"
done

reject_pattern '^README\.md$'
reject_pattern '^docs/decisions/[0-9][0-9][0-9][0-9].*\.md$'
reject_pattern '^docs/stories/(US-|epics/)'
reject_pattern '^docs/superpowers/'
reject_pattern '^\.harness/'
reject_pattern '^\.symphony/'
reject_pattern '^\.worktrees/'
reject_pattern '^harness\.db(-wal|-shm)?$'

grep -Fq 'PAYLOAD_MANIFEST="scripts/harness-install-files.txt"' \
  "$ROOT_DIR/scripts/install-harness.sh" || fail "Bash installer does not use the shared manifest"
grep -Fq '$script:PayloadManifest = "scripts/harness-install-files.txt"' \
  "$ROOT_DIR/scripts/install-harness.ps1" || fail "PowerShell installer does not use the shared manifest"

bash -n "$ROOT_DIR/scripts/install-harness.sh"

DRY_TARGET="$TMP_DIR/dry-target"
DRY_OUTPUT="$TMP_DIR/dry-run.txt"
"$ROOT_DIR/scripts/install-harness.sh" \
  --directory "$DRY_TARGET" --yes --dry-run >"$DRY_OUTPUT"

grep -Fq 'create   docs/HARNESS.md' "$DRY_OUTPUT" || fail "dry run omitted core Harness policy"
grep -Fq 'create   docs/decisions/README.md' "$DRY_OUTPUT" || fail "dry run omitted decision scaffold"
if grep -Eq '^(create|update|skip|overwrite)[[:space:]]+README\.md([[:space:]]|$)' "$DRY_OUTPUT"; then
  fail "dry run includes source repository README"
fi
if grep -Eq 'docs/decisions/[0-9][0-9][0-9][0-9].*\.md' "$DRY_OUTPUT"; then
  fail "dry run includes numbered decision history"
fi

RELEASE_DIR="$TMP_DIR/release"
mkdir -p "$RELEASE_DIR"
CLI_SOURCE="$ROOT_DIR/scripts/bin/harness-cli"
if [ ! -x "$CLI_SOURCE" ]; then
  CLI_SOURCE="$ROOT_DIR/target/debug/harness-cli"
fi
test -x "$CLI_SOURCE" || fail "Harness CLI fixture is unavailable; run cargo build first"
cp "$CLI_SOURCE" "$RELEASE_DIR/harness-cli-test-platform"
sha256_file "$RELEASE_DIR/harness-cli-test-platform" > \
  "$RELEASE_DIR/harness-cli-test-platform.sha256"

FRESH_TARGET="$TMP_DIR/fresh-target"
HARNESS_CLI_PLATFORM=test-platform \
HARNESS_CLI_BASE_URL="file://$RELEASE_DIR" \
  "$ROOT_DIR/scripts/install-harness.sh" --directory "$FRESH_TARGET" --yes >/dev/null

test -f "$FRESH_TARGET/docs/HARNESS.md" || fail "fresh install omitted core policy"
test -f "$FRESH_TARGET/docs/decisions/README.md" || fail "fresh install omitted decision scaffold"
test ! -e "$FRESH_TARGET/README.md" || fail "fresh install copied source repository README"
for decision_file in "$FRESH_TARGET/docs/decisions"/[0-9][0-9][0-9][0-9]*.md; do
  test ! -f "$decision_file" || fail "fresh install copied numbered decision history"
done
test ! -e "$FRESH_TARGET/harness.db" || fail "fresh install created operational database"
test ! -e "$FRESH_TARGET/.harness" || fail "fresh install created runtime history"

MERGE_TARGET="$TMP_DIR/merge-target"
mkdir -p "$MERGE_TARGET/docs/decisions"
printf 'target readme\n' >"$MERGE_TARGET/README.md"
printf 'target decision\n' >"$MERGE_TARGET/docs/decisions/0001-target.md"
HARNESS_CLI_PLATFORM=test-platform \
HARNESS_CLI_BASE_URL="file://$RELEASE_DIR" \
  "$ROOT_DIR/scripts/install-harness.sh" \
  --directory "$MERGE_TARGET" --merge --yes >/dev/null

test "$(cat "$MERGE_TARGET/README.md")" = 'target readme' || fail "merge changed target README"
test "$(cat "$MERGE_TARGET/docs/decisions/0001-target.md")" = 'target decision' || \
  fail "merge changed target decision history"

printf 'install payload validation passed\n'
```

- [ ] **Step 2: Make the script executable**

Run:

```bash
chmod +x scripts/validate-install-payload.sh
```

Expected: the executable bit is set for the new validation script.

- [ ] **Step 3: Run the contract and verify the RED state**

Run:

```bash
scripts/validate-install-payload.sh
```

Expected: FAIL with `forbidden payload pattern present: ^README\.md$` because the current manifest still installs the source repository README.

### Task 2: Make The Installer Payload Clean

**Files:**
- Modify: `scripts/harness-install-files.txt:1`
- Modify: `scripts/README.md:100`
- Modify: `README.md:63`
- Test: `scripts/validate-install-payload.sh`

- [ ] **Step 1: Remove source-project history from the shared manifest**

Change the manifest header and opening entries to:

```text
# Harness installer payload.
# Install reusable operating files and empty project scaffolds only.
# Do not add repository-harness task history or project identity here.
# Bash and PowerShell installers both read this list.
# Schema migrations are discovered automatically from scripts/schema/*.sql.
AGENTS.md
docs/ARCHITECTURE.md
```

Delete these entries entirely:

```text
README.md
docs/decisions/0001-harness-first-development.md
docs/decisions/0002-post-spec-product-lifecycle.md
docs/decisions/0003-generic-spec-intake-harness.md
docs/decisions/0004-sqlite-durable-layer.md
docs/decisions/0005-prebuilt-rust-harness-cli.md
docs/decisions/0006-phase-4-benchmark-triage.md
docs/decisions/0007-improvement-proposal-rules.md
```

Keep `docs/decisions/README.md` and every reusable policy, scaffold, template, schema discovery rule, CLI doc, and `.gitignore` entry.

- [ ] **Step 2: Document the boundary in the installer guide**

In `scripts/README.md`, replace the paragraph beginning `The installer must stay limited` and the following manifest paragraph with:

```markdown
The installer must stay limited to the reusable Harness operating kit. It
installs agent instructions, policy docs, empty project scaffolds, templates,
schema migrations, and the platform CLI. It does not install the source
repository's root `README.md`, numbered decision history, story packets,
changesets, run artifacts, database, application source folders, package
scripts, CI, tests, platform shells, or fake validation commands.

Installed projects create their own product docs, stories, decisions, traces,
changesets, and run artifacts as work happens. Existing target-owned files are
preserved by `--merge`; removing a path from the source payload is not a cleanup
instruction for already-installed projects.

The stable file payload is declared once in
`scripts/harness-install-files.txt` and is read by both the Bash and PowerShell
installers. Keep reusable Harness policies, empty scaffolds, and templates in
that allowlist. Validate changes with `scripts/validate-install-payload.sh`.
Schema migrations are different: both installers discover
`scripts/schema/*.sql` automatically from the source repository, so adding a
new migration only requires committing the SQL file.
```

- [ ] **Step 3: Document clean installs in the root README**

After the introductory install commands in `README.md`, add:

```markdown
A fresh install copies the reusable Harness operating kit, not this source
repository's identity or task history. The target keeps its own `README.md` and
starts with empty product, story, decision, backlog, test-matrix, database, and
run-history surfaces. Those records are created only from work performed in the
target project.
```

- [ ] **Step 4: Run the contract and verify the GREEN state**

Run:

```bash
scripts/validate-install-payload.sh
```

Expected: PASS with `install payload validation passed`.

- [ ] **Step 5: Check formatting and diff hygiene**

Run:

```bash
git diff --check
```

Expected: exit code 0 with no output.

- [ ] **Step 6: Commit the clean payload implementation**

```bash
git add scripts/validate-install-payload.sh scripts/harness-install-files.txt scripts/README.md README.md
git commit -m "fix(installer): exclude repository task history"
```

Expected: one commit containing the executable contract, manifest cleanup, and installer documentation.

### Task 3: Close US-083 With Durable Proof

**Files:**
- Modify: `docs/stories/US-083-clean-installer-payload.md`
- Test: `scripts/validate-install-payload.sh`

- [ ] **Step 1: Run story and repository validation**

Run:

```bash
scripts/bin/harness-cli story verify US-083
scripts/validate-install-payload.sh
bash -n scripts/install-harness.sh
git diff --check
```

Expected: story verification reports pass, payload validation prints `install payload validation passed`, Bash syntax succeeds, and diff check is clean.

- [ ] **Step 2: Update the story packet**

Change `## Status` from `planned` to `implemented` and replace the evidence section with:

```markdown
## Evidence

- `scripts/bin/harness-cli story verify US-083`
- `scripts/validate-install-payload.sh`
- `bash -n scripts/install-harness.sh`
- `git diff --check`
- Payload validation proved required policy/scaffold retention, rejected source
  README and numbered decision history, performed a fresh local install without
  database or run artifacts, and preserved target-owned README and decision
  files during `--merge`.
- Both Bash and PowerShell installers remain bound to
  `scripts/harness-install-files.txt`; PowerShell runtime execution was
  unavailable on the local macOS validation host.
```

- [ ] **Step 3: Record durable proof**

Run:

```bash
scripts/bin/harness-cli story update --id US-083 --status implemented \
  --unit 1 --integration 1 --e2e 1 --platform 1 \
  --evidence "Clean installer payload validated: required policies and empty scaffolds remain; source README, numbered decisions, database, changesets, and run artifacts are absent from fresh installs; merge preserves target-owned files; Bash and PowerShell share the guarded manifest." \
  --verify scripts/validate-install-payload.sh
```

Expected: `Story US-083 updated.`

- [ ] **Step 4: Record the Harness trace**

Run:

```bash
scripts/bin/harness-cli trace \
  --summary "Implemented US-083 clean installer payload" \
  --intake 23 \
  --story US-083 \
  --agent codex \
  --actions '["added payload-boundary validation","removed source README and numbered decisions from installer manifest","documented clean install boundary","validated fresh install and merge preservation"]' \
  --read '["AGENTS.md","README.md","docs/HARNESS.md","docs/FEATURE_INTAKE.md","docs/CONTEXT_RULES.md","scripts/harness-install-files.txt","scripts/install-harness.sh","scripts/install-harness.ps1","scripts/README.md","docs/superpowers/specs/2026-07-10-clean-installer-payload-design.md","docs/stories/US-083-clean-installer-payload.md"]' \
  --changed '["README.md","scripts/harness-install-files.txt","scripts/README.md","scripts/validate-install-payload.sh","docs/stories/US-083-clean-installer-payload.md"]' \
  --decisions '["kept reusable policies and empty scaffolds","excluded repository-harness identity and task history","preserved existing merge targets","used one guarded manifest for Bash and PowerShell"]' \
  --outcome completed \
  --notes "Validation passed: story verify, installer payload contract, Bash syntax, fresh install, merge preservation, and git diff check. PowerShell runtime unavailable locally; shared manifest binding verified structurally."
```

Expected: a new trace is recorded and scored.

- [ ] **Step 5: Commit the completed story evidence**

```bash
git add docs/stories/US-083-clean-installer-payload.md
git commit -m "docs: close clean installer payload story"
```

Expected: a separate documentation commit records final proof without mixing it into the implementation commit.

- [ ] **Step 6: Final verification**

Run:

```bash
scripts/bin/harness-cli story verify US-083
scripts/validate-install-payload.sh
git status --short
```

Expected: both validation commands pass and `git status --short` is empty.
