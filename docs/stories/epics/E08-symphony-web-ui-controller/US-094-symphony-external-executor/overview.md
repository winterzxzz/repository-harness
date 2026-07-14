# Overview

## Current Behavior

`run --prepare-only` creates an isolated Symphony run but no supported command
can let a main agent drive that run while its own subagent implements inside
the worktree.

## Target Behavior

A main agent can start, heartbeat, and complete an externally executed run.
Symphony preserves the active lock, expires abandoned leases, validates the
normal artifacts and changeset, and renders the executor and progress through
the existing CLI and Web UI surfaces.

## Prerequisite

`US-093` supplies the durable managed-runtime heartbeat, lifecycle stage, and
normalized event foundation. If it has not landed before implementation,
US-094 must incorporate only the prerequisite portions without changing their
approved managed-adapter behavior.

## Affected Users

- Main agents orchestrating internal subagents.
- Humans monitoring the run through Symphony CLI or Web UI.
- Maintainers diagnosing recovery and validation failures.

## Affected Product Docs

- `docs/SYMPHONY_QUICKSTART.md`
- `AGENTS.md`
- `docs/superpowers/specs/2026-07-14-symphony-external-executor-design.md`
- `docs/decisions/0010-external-executor-main-agent-lease.md`

## Non-Goals

- Multiple active runs, remote execution, queues, or scheduling.
- Symphony spawning or terminating the external subagent.
- Changing built-in adapter launch behavior.
- Granting the subagent access to root Symphony state.
