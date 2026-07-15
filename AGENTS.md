# Agent Instructions

## Template Review Boundary

Before reviewing workflow, reliability, or cost, check whether
`scripts/harness-install-files.txt` exists. Its presence identifies the Harness
source template: assess its fresh installer payload, not source-only stories,
decisions, ignored databases, changesets, or run history. Label each finding as
`template`, `fresh-install`, or `source-repo`; call it a template defect only
when it reproduces in the template or a fresh install. This guard does not
apply when the user explicitly asks to audit the source repository's own state.

<!-- HARNESS:BEGIN -->
## Harness

This repo uses Harness. Use the Rust Harness CLI at `scripts/bin/harness-cli`
on macOS/Linux or `.\scripts\bin\harness-cli.exe` on Windows as the main
operational tool.

Start with the smallest useful pack:

- Known story: `scripts/bin/harness-cli context --story <story-id>`
- No story yet: classify with `docs/FEATURE_INTAKE.md`, then run
  `scripts/bin/harness-cli query matrix` or
  `scripts/bin/harness-cli context --lane <tiny|normal|high-risk>` as needed.

For an explicitly approved, execution-ready story, hand implementation to
Symphony with `harness-symphony run <story-id>` so the user can monitor the run
in the local Web UI. Do not pass `--no-web`; report the controller URL printed
by Symphony to the user. Keep intake, investigation, planning, and direct tiny
edits in the current agent session. If `HARNESS_RUN_ID` is already set, continue
inside the current Symphony run instead of starting a nested run.

For an approved external-executor story, the main agent runs `run --prepare-only`,
`runs start` (Symphony auto-names the subagent Winter1–Winter5 when
`--executor` is omitted), periodic `runs heartbeat`, and `runs complete` from
the source repository. Pipe the subagent's stdout through
`runs output <run_id>` so its work streams live into the Web UI In Progress
console; the stream also refreshes the lease. The subagent edits only inside
the printed worktree and never invokes root lifecycle commands.

Before a step that could use an external tool, run
`scripts/bin/harness-cli query tools --capability <name> --status present` to
see what is equipped; an absent capability is a clean skip. Expand into
`README.md`, `docs/HARNESS.md`, `docs/ARCHITECTURE.md`,
`docs/CONTEXT_RULES.md`, `docs/TOOL_REGISTRY.md`, product docs, stories, or
decisions only when the context pack, risk lane, or code path points there.
<!-- HARNESS:END -->
