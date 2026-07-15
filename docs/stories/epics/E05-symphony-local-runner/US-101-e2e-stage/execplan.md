# Exec Plan

## Goal

Symphony independently executes each story's declared E2E command in the run
worktree as a visible `e2e` stage between `agent` and `validation`, streaming
its output live to the Web UI and failing the run on a non-zero exit.

## Scope

In scope:

- `scripts/schema/009-story-e2e.sql` migration (`story.e2e_command`).
- `harness-cli story add|update --e2e` flag.
- Contract embedding at prepare time; `finalize_prepared_run` e2e execution
  with timeout, streaming, skip, and failure paths.
- `e2e.timeout_minutes` config (default 15).
- Web backend stage list + web-ui task flow node and tests.
- Docs: SYMPHONY_QUICKSTART lifecycle section.

Out of scope:

- Review approval gate (US-102).
- Re-running unit/integration tests independently.
- Retry policy for flaky E2E runs.

## Risk Classification

Risk flags:

- Data model (schema migration).
- Existing behavior (run lifecycle changes for every run).
- Public contracts (RUN_CONTRACT.json field, web flow payload).
- Multi-domain (harness-cli, symphony runner, web UI).

Hard gates:

- Data migration → high-risk lane.

## Work Phases

1. Discovery — done (validation stage confirmed evidence-only; flow stage
   lists located in `run.rs:627`, `web.rs:2915`, `task-flow-model.ts:14`).
2. Design — `design.md` in this folder, approved direction: option A
   (dedicated stage), per-story command, clean skip.
3. Validation planning — `validation.md` in this folder.
4. Implementation order:
   a. Migration + `harness-cli story --e2e` + tests.
   b. Contract field + prepare embedding + tests.
   c. `finalize_prepared_run` e2e execution (skip/pass/fail/timeout,
      streaming) + tests.
   d. Config key + doctor/config show surface.
   e. Web backend stage list + web-ui flow model + tests.
   f. Docs + binary smoke.
5. Verification — full `cargo test`, web-ui model tests, real-binary smoke
   with a fixture story (pass, fail, and skip cases).
6. Harness update — record decision for the lifecycle change; story proof
   status via `harness-cli story update`.

## Stop Conditions

Pause for human confirmation if:

- The contract change turns out to break existing RUN_CONTRACT consumers.
- The e2e stage needs to mutate anything outside the worktree.
- Validation requirements would need to be weakened to make runs pass.
- Windows shell semantics force a different interface than designed.
