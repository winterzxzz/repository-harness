# Harness Components

This taxonomy helps a project identify which parts of its agent environment are
available, partial, or missing. It is a discovery aid, not a claim that a fresh
project already implements every responsibility.

Use these status values:

- **Covered**: the project has an explicit, working mechanism with evidence.
- **Partial**: support exists but is incomplete, manual, or unmeasured.
- **Missing**: the project has no meaningful support yet.
- **Not applicable**: the responsibility does not apply to this project.

## Responsibility Map

| Responsibility | Fresh-install starting point | Evidence to look for |
| --- | --- | --- |
| Task specification | Partial | Product contracts, story packets, acceptance criteria |
| Context selection | Partial | `AGENTS.md`, generated context packs, project-specific retrieval rules |
| Tool access | Partial | Harness CLI plus registered project capabilities |
| Project memory | Partial | Decisions, product docs, stories, and durable records |
| Task state | Partial | Story rows, lifecycle state, dependencies, and proof status |
| Observability | Partial | Agent traces; add runtime logs, metrics, or traces when the product needs them |
| Failure attribution | Partial | Errors and friction tied to a task, component, or verification result |
| Verification | Partial | Project-owned verification commands and evidence |
| Permissions | Partial | Risk lanes, repository instructions, and enforced platform controls |
| Entropy auditing | Partial | Drift audit, repeated-friction review, and measured improvement outcomes |
| Intervention recording | Partial | Human, reviewer, CI, or agent corrections recorded separately from execution traces |

The installed Harness provides scaffolding and durable commands for these
responsibilities. A project should upgrade a status only after project-local
evidence exists. Do not treat the presence of a template file as proof that the
underlying responsibility is covered.

## Extension Surfaces

Harness deliberately leaves product-specific capabilities open. Register tools
by purpose rather than hard-coding a provider:

```bash
scripts/bin/harness-cli tool register \
  --name <provider> \
  --kind <cli|binary|mcp|skill|http> \
  --capability <purpose> \
  --command <command> \
  --responsibility <responsibility> \
  --description <description>
```

Examples include impact analysis, browser validation, security scanning,
coverage, deployment verification, and performance measurement. Missing
optional capabilities should degrade proof explicitly rather than block
unrelated work.

## Review Questions

Use this file when evaluating the project environment:

1. Which responsibilities matter for the current product and risk lane?
2. What project-local evidence supports each claimed status?
3. Which missing capability repeatedly consumes human attention?
4. Can a small instruction fix the gap, or does it require an executable guard?
5. How will the project measure whether an improvement helped?

Record repeated gaps as friction or backlog proposals instead of expanding the
default instructions for every project.
