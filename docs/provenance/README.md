# Historical And Provenance Index

This material explains how the current repository-centered workflow and
compatibility boundary evolved. It is source evidence, not default task
instruction, and is not installed into consumers.

## Superseded Workflow Decisions

- [Harness-first development](../decisions/0001-harness-first-development.md)
- [Post-spec lifecycle](../decisions/0002-post-spec-product-lifecycle.md)
- [Generic intake harness](../decisions/0003-generic-spec-intake-harness.md)

## Compatibility Provenance

- [SQLite durable layer](../decisions/0004-sqlite-durable-layer.md)
- [Prebuilt CLI](../decisions/0005-prebuilt-rust-harness-cli.md)
- [Benchmark triage](../decisions/0006-phase-4-benchmark-triage.md)
- [Improvement proposal rules](../decisions/0007-improvement-proposal-rules.md)
- [Reproducible core state](../decisions/0011-reproducible-core-state.md)

## Execution And Review Evidence

- `docs/stories/epics/`: legacy story-era implementation and validation packets.
- `docs/reviews/`: retained review findings.
- `docs/plans/completed/`: consequential completed execution plans.
- `docs/provenance/*.json`: machine-readable migration and ownership evidence.
- Git history: authoritative file evolution and commit grouping.

Use the [CLI compatibility index](../compatibility/README.md) for supported
current compatibility behavior rather than reconstructing commands from old
story packets.
