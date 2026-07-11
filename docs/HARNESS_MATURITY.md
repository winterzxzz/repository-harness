# Harness Maturity

This ladder helps a project assess how reliably agents can turn intent into
verified changes. A fresh install starts with reusable scaffolding, not a
pre-certified maturity level. Claim a level only from project-local evidence.

The ladder is cumulative: higher levels include the useful properties of lower
levels. It is a diagnostic model, not a mandatory roadmap.

## H0 — Unstructured

Work depends mainly on prompts and individual judgment.

Typical signals:

- No stable project instructions or product contracts.
- Task state and decisions live primarily in conversations.
- Completion relies on an agent's narrative rather than executable proof.

## H1 — Legible

The repository gives agents a small, reliable map.

Evidence may include:

- A concise `AGENTS.md` that points to deeper sources of truth.
- Product, architecture, and validation documents that reflect the project.
- Clear risk lanes and lightweight intake guidance.
- No broken local references or inherited template history.

## H2 — Durable

Important task state and learning survive across agent runs.

Evidence may include:

- Stories, decisions, traces, and backlog items in durable records.
- Generated context packs that start small and expand by need.
- Project capabilities discoverable through the tool registry.
- Friction recorded when agents repeatedly lack context or tools.

## H3 — Measured

The project can evaluate the quality of its agent workflow.

Evidence may include:

- Trace and context quality scored consistently.
- Failures attributed to product code, validation, context, tools, or policy.
- Repeated friction grouped into reviewable improvement candidates.
- Benchmarks or representative tasks provide a baseline.

## H4 — Verified

Behavioral proof is part of normal task completion.

Evidence may include:

- Stories own exact verification commands when mechanical proof is possible.
- Verification results are recorded and false-done states are surfaced.
- The proof type matches the behavior: unit, integration, E2E, platform, or
  explicit unavailable evidence.
- High-risk changes have stronger boundaries and review evidence than tiny work.

## H5 — Improving

The harness changes itself using measured outcomes rather than accumulated
ceremony.

Evidence may include:

- Audit, friction, and intervention data generate bounded proposals.
- Improvements state a predicted impact before implementation.
- Actual outcomes are compared with a baseline after implementation.
- Ineffective rules are revised or removed.
- Stable, repeated principles become executable guards; local implementation
  choices remain flexible.

## Assessment Method

Use the smallest evidence set that answers the question:

```bash
scripts/bin/harness-cli query matrix
scripts/bin/harness-cli query friction
scripts/bin/harness-cli query interventions
scripts/bin/harness-cli query tools --summary
scripts/bin/harness-cli audit
```

Then review the relevant project docs and verification output. Do not award a
level because the CLI contains a command or the installer contains a template;
the project must be using the capability successfully.

## Design Principle

Keep the harness rigid only at consequential boundaries: safety, durable state,
valid artifacts, and honest verification. Let capable models choose how to
explore the repository and implement changes inside those boundaries.
