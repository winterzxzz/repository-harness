# Agent Instructions

## Project Skills

Use `.codex/skills/harness-intake-griller/SKILL.md` when a request needs
discussion, feature intake, docs, or story shaping before Symphony execution.
The skill is project-scoped; do not use a global copy as the source of truth.

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

Before a step that could use an external tool, run
`scripts/bin/harness-cli query tools --capability <name> --status present` to
see what is equipped; an absent capability is a clean skip. Expand into
`README.md`, `docs/HARNESS.md`, `docs/ARCHITECTURE.md`,
`docs/CONTEXT_RULES.md`, `docs/TOOL_REGISTRY.md`, product docs, stories, or
decisions only when the context pack, risk lane, or code path points there.
<!-- HARNESS:END -->
