# Agent Symphony Execution Guidance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure AI agents in fresh Harness installs route execution-ready stories through Symphony so users can monitor runs in the automatically started Web UI.

**Architecture:** Add a stable execution rule to the installer-managed `AGENTS.md`, relying on the existing `harness-symphony run` behavior that starts or reuses the local Web UI. Extend the fresh-install payload validator so release validation fails if this rule is absent or loses its recursion and opt-out boundaries.

**Tech Stack:** Markdown agent instructions, Bash installer validation, Rust Harness CLI durable records.

---

### Task 1: Lock the fresh-install contract with a failing validator

**Files:**
- Modify: `scripts/validate-install-payload.sh`

- [x] **Step 1: Add assertions against the installed `AGENTS.md`**

Add fixed-string checks requiring the fresh install to mention `harness-symphony run <story-id>`, prohibit `--no-web` for observable runs, and avoid starting a nested Symphony run when `HARNESS_RUN_ID` is already set.

- [x] **Step 2: Run the validator and confirm RED**

Run: `scripts/validate-install-payload.sh`

Expected: FAIL because the installed `AGENTS.md` does not yet contain the Symphony execution rule.

### Task 2: Add the minimal installer-managed agent guidance

**Files:**
- Modify: `AGENTS.md`

- [x] **Step 1: Add the execution-ready story rule**

Within the managed Harness block, instruct agents to use `harness-symphony run <story-id>` for explicitly approved execution-ready stories, keep the default Web UI behavior, report the printed controller URL, and continue directly inside an existing Symphony run.

- [x] **Step 2: Run the validator and confirm GREEN**

Run: `scripts/validate-install-payload.sh`

Expected: PASS with `install payload validation passed`.

### Task 3: Record and verify Harness proof

**Files:**
- Modify: `docs/stories/US-089-agent-symphony-execution-guidance.md`
- Modify: `.harness/changesets/<generated>.changeset.jsonl`

- [x] **Step 1: Update the story evidence and durable proof**

Record the fresh-install validator as unit, integration, and platform proof; keep E2E false because no browser UI behavior changes.

- [x] **Step 2: Run final checks**

Run: `scripts/bin/harness-cli story verify US-089 && git diff --check`

Expected: both commands exit successfully.

- [x] **Step 3: Record a standard trace**

Record intake, story, actions, read/changed files, validation, and friction through `scripts/bin/harness-cli trace`.

### Review follow-up: Preserve guidance during shim refresh

- [x] Add a failing fresh-install regression proving `--refresh-agent-shim`
  must retain the Symphony execution guidance.
- [x] Update `agent_shim_block()` with the same Web UI and nested-run boundaries.
- [x] Run `scripts/validate-install-payload.sh` and confirm the full validator passes.
