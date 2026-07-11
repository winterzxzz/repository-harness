# Clean Template Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure a fresh Harness install contains only generic, internally consistent project guidance with no repository-harness history or missing local references.

**Architecture:** Keep the existing manifest-driven installer and add semantic assertions to its fresh-install validator. Make the distributed Markdown files generic rather than adding new workflow gates or runtime requirements.

**Tech Stack:** Bash validation, Markdown templates, Rust Harness CLI smoke tests.

---

### Task 1: Add semantic fresh-install regression checks

**Files:**
- Modify: `scripts/validate-install-payload.sh`

- [x] Assert the installed AGENTS file does not reference an absent project skill.
- [x] Assert the fresh test matrix contains no source story rows.
- [x] Assert installed documentation does not name `repository-harness` or reference omitted Symphony documents.
- [x] Run `scripts/validate-install-payload.sh` and confirm it fails on current payload content.

### Task 2: Make distributed documentation project-neutral

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/README.md`
- Modify: `docs/product/README.md`
- Modify: `docs/TEST_MATRIX.md`
- Modify: `docs/HARNESS_COMPONENTS.md`
- Modify: `docs/HARNESS_MATURITY.md`

- [x] Remove the source-only intake skill instruction from the installed agent shim.
- [x] Remove references to Symphony documents and contracts that are not in the payload.
- [x] Reset the legacy test matrix to an empty scaffold.
- [x] Convert maturity and component descriptions from source-repository claims to project-local capability guidance.
- [x] Preserve flexible discovery, optional tools, and lane-based ceremony.

### Task 3: Verify fresh-install behavior

**Files:**
- Test: `scripts/validate-install-payload.sh`

- [x] Run the full installer payload validator and require exit code 0.
- [x] Run `git diff --check`.
- [x] Review the final diff for source history, dead references, and accidental workflow hardening.
