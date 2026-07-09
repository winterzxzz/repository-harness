# Harness Symphony Quick Start

Harness Symphony is the local runner for Harness stories. It prepares a safe
workspace, gives the agent an explicit contract, collects the result, and keeps
durable Harness state reviewable through committed changesets.

Use this guide when you want to run a story, not when you want to understand the
full design. The full design lives in `docs/SYMPHONY_SCOPE.md`.

## What Symphony Does

Symphony turns this:

```text
Harness story
```

into this:

```text
isolated run workspace
  + copied harness.db
  + RUN_CONTRACT.json
  + agent execution
  + SUMMARY.md
  + RESULT.json
  + .harness/changesets/<run_id>.changeset.jsonl
```

The important rule: the root `harness.db` is not the source of truth for a run.
The run writes to a copied database, and durable Harness changes are preserved
as semantic changesets.

## Visual Map

Use this as the quick mental picture:

```text
+---------------+
| Harness story |
+-------+-------+
        | work list chooses a runnable story
        v
+-------------------------------------------------------------+
| Normal / high-risk run                                      |
|                                                             |
|  .symphony/worktrees/<run_id>/                              |
|    +- harness.db                copied DB for this run       |
|    +- AGENTS.md                 Symphony shim for the agent  |
|    +- .harness/runs/<run_id>/RUN_CONTRACT.json              |
+-------+-----------------------------------------------------+
        | agent works only inside the assigned run contract
        v
+-------------------------------------------------------------+
| Required outputs                                            |
|                                                             |
|  .harness/runs/<run_id>/SUMMARY.md                          |
|  .harness/runs/<run_id>/RESULT.json                         |
|  .harness/changesets/<run_id>.changeset.jsonl               |
+-------+-----------------------------------------------------+
        | review, merge, then replay committed changesets
        v
+-----------------------+
| harness-symphony sync |
+-----------------------+
```

Tiny stories take a shorter path:

```text
+------------+
| tiny story |
+-----+------+
        | harness-symphony run <story-id> --here
        v
+-------------------------------------------------------------+
| current checkout                                            |
|   + copied DB under .symphony/runs/                         |
|   + SUMMARY.md, RESULT.json, and changeset still required   |
+-------------------------------------------------------------+
```

## Mermaid Flow

```mermaid
flowchart TD
    A[Harness story] --> B{Lane}
    B -->|tiny| C[run story-id --here]
    B -->|normal or high-risk| D[run story-id --prepare-only]
    D --> E[Isolated worktree]
    E --> F[RUN_CONTRACT.json and AGENTS.md shim]
    F --> G[run story-id]
    C --> H[Agent writes required outputs]
    G --> H
    H --> I[SUMMARY.md]
    H --> J[RESULT.json]
    H --> K[Semantic changeset JSONL]
    I --> L[Review or PR]
    J --> L
    K --> L
    L --> M[Merge]
    M --> N[harness-symphony sync]
    N --> O[Local harness.db updated]
```

```mermaid
flowchart LR
    subgraph Root[Root checkout]
        RDB[(root harness.db)]
        RCODE[repo files]
    end

    subgraph Run[Symphony run workspace]
        W[.symphony/worktrees/run_id]
        CDB[(copied harness.db)]
        CONTRACT[RUN_CONTRACT.json]
        AGENTS[AGENTS.md shim]
        OUT[SUMMARY.md + RESULT.json]
        CHANGESET[changeset JSONL]
    end

    RDB -. copied, not mutated .-> CDB
    RCODE --> W
    W --> CONTRACT
    W --> AGENTS
    CDB --> CHANGESET
    CONTRACT --> OUT
    OUT --> Review[human review]
    CHANGESET --> Review
```

## First Run Checklist

Run these from the repository root.

### 1. Build The CLI

```bash
cargo build -p harness-symphony
```

During development, the command path is:

```bash
target/debug/harness-symphony
```

### 2. Check Readiness

```bash
target/debug/harness-symphony doctor
```

Fix any `fail` rows before running a normal story. Warnings are usually
actionable configuration gaps; read the message before deciding to continue.

### 3. See Runnable Work

```bash
target/debug/harness-symphony work list
```

Look for a story with `Runnable` set to `yes` or `warn`.

- `yes`: ready to run.
- `warn`: runnable, but something important is missing, often a verification
  command.
- `no`: do not run it through Symphony yet.

### 4. Prepare A Normal Or High-Risk Story

Use `--prepare-only` when you want to inspect what Symphony will give the agent
before actually launching one:

```bash
target/debug/harness-symphony run <story-id> --prepare-only
```

This creates an isolated worktree under:

```text
.symphony/worktrees/<run_id>/
```

It also creates a run contract at:

```text
.harness/runs/<run_id>/RUN_CONTRACT.json
```

The worktree `AGENTS.md` contains a Symphony block that points the agent to the
contract and repeats the assigned story, copied database path, required outputs,
and forbidden paths.

### 5. Execute A Normal Or High-Risk Story

When the repo is configured with an agent adapter, run:

```bash
target/debug/harness-symphony run <story-id>
```

The agent must produce:

```text
.harness/runs/<run_id>/SUMMARY.md
.harness/runs/<run_id>/RESULT.json
```

If the run writes durable Harness records, it must also produce:

```text
.harness/changesets/<run_id>.changeset.jsonl
```

Symphony validates the result before accepting the run.

## Tiny Story Shortcut

Tiny-lane stories can use a lighter path:

```bash
target/debug/harness-symphony run <story-id> --here
```

Use `--here` only for tiny stories. It skips the separate worktree, but it still
uses a copied database and still requires the same run artifacts.

Do not use `--here` for normal or high-risk work. Symphony refuses those lanes
because they need the full isolation loop.

## After A Run

Check status:

```bash
target/debug/harness-symphony status
```

List runs:

```bash
target/debug/harness-symphony runs list
```

Inspect one run:

```bash
target/debug/harness-symphony runs show <run_id>
```

Review these local files before opening a PR:

```text
.harness/runs/<run_id>/SUMMARY.md
.harness/runs/<run_id>/RESULT.json
.harness/changesets/<run_id>.changeset.jsonl
```

`SUMMARY.md` is the human review surface. It should include a readable Harness
changes table so reviewers do not have to inspect raw JSONL first. PR creation
uses the summary as the PR body and commits only the durable changeset artifact.
If PR creation is disabled, review these artifacts locally before syncing.

## Optional PR Flow

If PR creation is configured, create a PR for a finished run:

```bash
target/debug/harness-symphony pr create <run_id>
```

Retry PR creation after fixing configuration or provider issues:

```bash
target/debug/harness-symphony pr retry <run_id>
```

PRs should include the run summary, result file, changeset, and any product or
docs changes made by the agent.

When `pull_request.create` is `disabled`, completed runs stay in Review as a
local artifact-review step instead of becoming Needs Attention for missing PR
creation.

## After Merge

After pulling merged changes, apply committed changesets to your local
`harness.db`:

```bash
target/debug/harness-symphony sync
```

`sync` is idempotent. Running it twice is safe; already applied changesets are
skipped.

On a fresh clone, rebuild the local Harness database from committed changesets:

```bash
scripts/bin/harness-cli db rebuild --from .harness/changesets
```

## Minimal Configuration

Symphony reads optional configuration from:

```text
.harness/symphony.yml
```

Defaults are usable for local development. To use the first-class Codex adapter:

```yaml
version: 1
agent:
  adapter: codex
```

For the first-class OpenCode adapter (headless `opencode run --auto`):

```yaml
version: 1
agent:
  adapter: opencode
```

The web UI can also pick the agent per run: the board Run button is a split
button (Codex/OpenCode), the chosen agent is remembered as the default in the
Symphony state DB, and the Settings tab manages the same default via
`GET/PUT /api/settings`.

For a custom one-shot command adapter:

```yaml
version: 1
agent:
  adapter: custom
  command:
    - ./scripts/run-agent.sh
```

Inspect the resolved configuration:

```bash
target/debug/harness-symphony config show
```

## The Mental Model

| If you want to... | Run... |
| --- | --- |
| Check whether Symphony can run here | `target/debug/harness-symphony doctor` |
| Find runnable stories | `target/debug/harness-symphony work list` |
| Create an isolated workspace only | `target/debug/harness-symphony run <story-id> --prepare-only` |
| Run a normal or high-risk story | `target/debug/harness-symphony run <story-id>` |
| Run a tiny story in the current checkout | `target/debug/harness-symphony run <story-id> --here` |
| See what happened locally | `target/debug/harness-symphony status` |
| Inspect a run | `target/debug/harness-symphony runs show <run_id>` |
| Create a PR for a finished run | `target/debug/harness-symphony pr create <run_id>` |
| Apply merged changesets | `target/debug/harness-symphony sync` |

## Common Mistakes

- Do not send normal or high-risk stories through `--here`.
- Do not edit root `harness.db` to represent run results.
- Do not commit `.symphony/`; it is local runtime state.
- Do not review only raw changeset JSONL; read `SUMMARY.md` first.
- Do not forget `sync` after merging a Symphony PR.
- Do not treat Symphony as a second intake system. Harness chooses story scope
  and lane; Symphony runs the selected story safely.
