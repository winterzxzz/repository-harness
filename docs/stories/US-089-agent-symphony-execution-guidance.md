# US-089 Agent Symphony Execution Guidance

## Status

implemented

## Lane

normal

## Product Contract

An AI agent operating in a fresh Harness install routes an explicitly approved,
execution-ready story through Symphony so the user can monitor the run in the
automatically started local Web UI.

## Relevant Product Docs

- `docs/HARNESS.md`
- `docs/FEATURE_INTAKE.md`

## Acceptance Criteria

- Fresh-installed `AGENTS.md` directs execution-ready stories through
  `harness-symphony run <story-id>`.
- Agents retain Symphony's default Web UI startup and report the controller URL
  instead of passing `--no-web`.
- Intake, planning, investigation, direct tiny edits, and work already running
  under `HARNESS_RUN_ID` do not recursively start Symphony.
- Installer payload validation fails if the durable agent guidance is absent.

## Design Notes

- Commands: `harness-symphony run <story-id>` and
  `scripts/validate-install-payload.sh`.
- Domain rules: only explicitly approved, execution-ready stories are routed;
  Symphony remains the owner of isolated execution and observable run state.
- UI surfaces: existing local Symphony Web UI; no UI implementation changes.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | Fixed-string assertions reject missing agent guidance. |
| Integration | Installer creates a fresh target whose `AGENTS.md` contains the rule. |
| E2E | Not required; existing UI behavior is unchanged. |
| Platform | Bash fresh-install payload validation passes on macOS/Linux. |
| Release | `scripts/validate-install-payload.sh` passes. |

## Harness Delta

The reusable agent entrypoint gains an operational handoff rule connecting
approved story execution to Symphony's observable UI-backed run path.

## Evidence

- TDD RED: `scripts/validate-install-payload.sh` failed with
  `fresh install AGENTS.md does not route approved story execution through Symphony`.
- TDD GREEN: `scripts/validate-install-payload.sh` passed after installing and
  checking the managed agent guidance in a fresh target.
- Review-fix RED: the validator failed because `--refresh-agent-shim` replaced
  the managed block without the Symphony guidance.
- Review-fix GREEN: the generated shim now preserves the same Symphony handoff,
  Web UI, and nested-run boundaries, and the full payload validator passes.
