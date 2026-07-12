# Context Discovery Rules

Context rules help agents choose where to start, when to expand, and what to
skip. They are advisory retrieval rules, not a script for exploring the repo.

The goal is to put enough task-shaped information in context without fighting
the agent's normal loop: search, open nearby files, follow imports, edit, and
run proof.

## Start Pack

Prefer a generated pack over rebuilding the reading list by hand:

```bash
scripts/bin/harness-cli context --story US-XXX --out context.md
scripts/bin/harness-cli context --lane normal
```

Treat the pack as a starting index. After that, follow imports, search results,
story links, changed files, and nearby tests.

## Retrieval Levels

Use three levels:

| Level | Meaning |
| --- | --- |
| Start here | Small seed context for the current phase. |
| Expand when | Add this only when the trigger is present. |
| Skip unless | Leave it out unless the task clearly crosses that boundary. |

## Intake Phase

Use this phase to classify the request, identify surface, and choose lane.

| Level | Sources |
| --- | --- |
| Start here | `docs/FEATURE_INTAKE.md`; `scripts/bin/harness-cli query matrix`; user prompt or story id |
| Expand when | `README.md` for project thesis or install flow; `docs/HARNESS.md` for process changes; relevant `docs/product/*` for product behavior |
| Skip unless | `docs/ARCHITECTURE.md`, `docs/decisions/*`, and high-risk templates unless risk, boundaries, or durable policy changes are in scope |

## Planning Phase

Use this phase to choose the smallest safe approach and proof.

| Level | Sources |
| --- | --- |
| Start here | Current files to edit; relevant story packet when one exists; generated context pack |
| Expand when | Adjacent files with the same pattern; `docs/templates/story.md` when creating a normal story; relevant product docs when behavior changes |
| Skip unless | `docs/templates/high-risk-story/*`, `docs/HARNESS_MATURITY.md`, and broad decision history unless the lane or trigger asks for them |

## Implementation Phase

Use this phase to edit only the selected slice.

| Level | Sources |
| --- | --- |
| Start here | Files being changed; nearest tests; adjacent implementation pattern |
| Expand when | Relevant product docs or story packet when code changes accepted behavior; `docs/ARCHITECTURE.md` when structure or boundaries move |
| Skip unless | Provider/API/security docs, historical traces, and unrelated stories unless the code path crosses those boundaries |

## Validation Phase

Use this phase to prove the change without inventing new ceremony.

| Level | Sources |
| --- | --- |
| Start here | Story validation section or `scripts/bin/harness-cli query matrix`; verification command in the story row; relevant package README when needed to run proof |
| Expand when | `docs/templates/validation-report.md` for notable proof; benchmark protocol when benchmark claims are made |
| Skip unless | Full platform or release validation unless the lane, story, or touched files call for it |

## Trace Phase

Use this phase to leave evidence for the next agent.

| Level | Sources |
| --- | --- |
| Start here | `git status --short`; validation output; changed-file list; story packet when one exists |
| Expand when | `docs/TRACE_SPEC.md` for normal or high-risk traces; `scripts/bin/harness-cli query backlog` when friction occurred |
| Skip unless | `docs/HARNESS_COMPONENTS.md` unless attributing failure to Harness components |

## Expansion Triggers

| Trigger | Expand to |
| --- | --- |
| Database schema, durable records, or migrations | `docs/decisions/` when a matching record exists, `scripts/schema/`, and relevant CLI code |
| CLI behavior or installer distribution | `docs/decisions/` when a matching record exists, `scripts/README.md`, relevant `crates/harness-cli/*` code, CLI help, and installer docs when present |
| Auth, authorization, data loss, audit/security, or external providers | High-risk story template, relevant decisions, and owner confirmation when direction is ambiguous |
| Public API, product behavior, or user-visible workflow | Relevant `docs/product/*`, story packets, validation expectations, and UI/API tests |
| Harness policy, source hierarchy, risk classification, or validation rules | `docs/HARNESS.md`, `docs/FEATURE_INTAKE.md`, `docs/ARCHITECTURE.md`, and relevant decisions |
| Repeated confusion, stale docs, or missing proof | `docs/HARNESS_BACKLOG.md`; record `harness_friction`; add backlog when fix is out of scope |
| Maturity, observability, trace quality, or benchmark claim | `docs/HARNESS_COMPONENTS.md`, `docs/HARNESS_MATURITY.md`, and `docs/TRACE_SPEC.md` |

## Budget Guidance

| Lane | Target | Shape |
| --- | --- | --- |
| Tiny | About 2K Harness-context tokens | `AGENTS.md` or generated pack, `docs/FEATURE_INTAKE.md`, matrix, and exact files |
| Normal | About 5K Harness-context tokens | Generated pack, relevant story/product docs, touched files, nearby tests, proof command |
| High-risk | About 10K Harness-context tokens | Generated pack plus decisions, boundary docs, high-risk template, product docs, and validation docs |

## Review Checklist

Before implementation:

- Lane chosen from `docs/FEATURE_INTAKE.md` or existing story.
- Generated pack or equivalent small start context reviewed.
- Expansion triggers checked against touched files and risk.

Before final response:

- Validation output reviewed.
- `git status --short` reviewed.
- Trace records actions, files read, files changed, outcome, and friction when
  useful.
