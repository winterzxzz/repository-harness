# Decisions

Decision records preserve lasting product, architecture, data ownership,
security, compatibility, and validation choices that future work must inherit.

Use `docs/templates/decision.md`. Task-local implementation choices remain in
the active execution plan and do not require a separate decision.

An installed consumer begins with no fabricated decisions. Add local decision
documents here as real choices are accepted, then index them in this file.

## Upstream Current Decisions

These source-repository decisions explain Harness itself; they are not installed
consumer product choices.

| Decision | Status | Title |
| --- | --- | --- |
| [0019](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0019-repository-centered-default-workflow.md) | Active | Repository-Centered Default Workflow |
| [0020](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0020-installation-profiles-and-knowledge-boundaries.md) | Active | Installation Profiles And Knowledge Boundaries |

## Compatibility Decisions

These decisions remain relevant only when maintaining the optional CLI,
SQLite, or orchestration surface.

| Decision | Status | Title |
| --- | --- | --- |
| [0004](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0004-sqlite-durable-layer.md) | Compatibility | SQLite Durable Layer |
| [0005](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0005-prebuilt-rust-harness-cli.md) | Compatibility | Prebuilt Rust Harness CLI |
| [0006](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0006-phase-4-benchmark-triage.md) | Compatibility | Phase 4 Benchmark Triage |
| [0007](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0007-improvement-proposal-rules.md) | Compatibility | Improvement Proposal Rules |
| [0011](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0011-reproducible-core-state.md) | Compatibility | Reproducible Core State |

## Historical Decisions

These records explain superseded default behavior and remain available for
provenance rather than current instruction.

| Decision | Status | Title |
| --- | --- | --- |
| [0001](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0001-harness-first-development.md) | Amended by 0019 | Harness-First Development |
| [0002](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0002-post-spec-product-lifecycle.md) | Superseded by 0003 | Seed Specification Product Lifecycle |
| [0003](https://github.com/hoangnb24/repository-harness/blob/main/docs/decisions/0003-generic-spec-intake-harness.md) | Amended by 0019 | Generic Spec Intake Harness |

## Add A Decision When

- A locked technical choice changes.
- Product behavior changes meaningfully and alternatives have different
  consequences.
- Data ownership, authorization, privacy, security, or public compatibility is
  decided.
- A validation requirement is added, removed, or weakened.
- The source-of-truth hierarchy or default workflow changes.

Do not add a decision merely because a task mentions a sensitive domain or uses
a durable execution plan.
