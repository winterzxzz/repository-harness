# Symphony Runtime Cleanup Design

## Context

Fresh Harness installations can accumulate a large `.symphony/worktrees/`
tree because Symphony creates one Git worktree per normal or high-risk run but
does not remove terminal worktrees. The source repository already ignores its
runtime paths, but the Bash and PowerShell installers only merge database and
downloaded-binary rules into a target project's `.gitignore`. The cleanup
configuration is parsed and displayed, yet it is not connected to run or sync
lifecycle behavior.

This is a `fresh-install` defect. Source-repository state is useful evidence of
the impact, but it is not the boundary for the fix.

## Outcome

Freshly installed projects ignore all local Symphony runtime state, and
Symphony safely reclaims terminal worktrees without deleting durable
changesets or branches that may still contain unmerged code.

## Non-Goals

- Deleting `.harness/changesets/`.
- Automatically deleting Symphony branches.
- Moving worktrees into the operating system temporary directory.
- Cleaning arbitrary directories outside the configured Symphony worktree
  root.
- Adding a Web UI cleanup control in this slice.

## Considered Approaches

### Manual cleanup only

Add ignore rules and a command that users run when disk usage becomes visible.
This is simple, but it preserves the current failure mode for users who do not
know the command exists.

### Lifecycle cleanup with an explicit command

Clean safe worktrees after a run reaches Done, sweep expired failed and orphan
worktrees when Symphony starts, and expose the same engine through a command.
This is the selected approach because it keeps normal operation bounded while
retaining an inspectable and recoverable manual path.

### Operating-system temporary worktrees

Create worktrees outside the repository. This reduces visual clutter but does
not define retention, complicates repository discovery and cross-platform
behavior, and makes crash recovery less predictable.

## Fresh-Install Ignore Contract

Both installers must idempotently merge these rules into the target
`.gitignore`:

```gitignore
.symphony/
.worktrees/
.harness/*
!.harness/changesets/
!.harness/changesets/*.changeset.jsonl
```

Existing database and downloaded-binary rules remain unchanged. Re-running an
installer must not duplicate rules or replace target-owned `.gitignore`
content. The changeset exceptions preserve the existing durable-history
contract.

## Cleanup Policy

Cleanup distinguishes durable history, review evidence, Git branches, and
disposable worktrees:

| Resource | Policy |
| --- | --- |
| `.harness/changesets/` | Never remove. |
| `.harness/runs/<run_id>/` | Keep the newest configured number of terminal runs; never compact active-run evidence. |
| `.symphony/worktrees/<run_id>/` for Done runs | Remove immediately after successful sync when cleanup is enabled. |
| Failed or interrupted worktrees | Keep for seven days by default, then remove. |
| Orphan worktrees | Remove after seven days when they are under the configured worktree root and are not active. |
| `symphony/<run_id>` branches | Preserve in this slice. |

A Done cleanup removes the registered Git worktree and its directory. It does
not delete the branch. This is deliberate: sync applies Harness changesets, but
a local-review workflow may still depend on a branch that has not become
reachable from the base branch.

Failed and interrupted worktrees remain available for debugging during the
retention window. A zero-day retention explicitly requests immediate cleanup;
it is not the default.

## Configuration

The cleanup configuration becomes operational:

```yaml
cleanup:
  cleanup_after_sync: true
  keep_failed_worktrees: true
  failed_worktree_retention_days: 7
```

`cleanup_after_sync` defaults to `true` for new and configuration-free
installs. `keep_failed_worktrees: false` means failed and interrupted
worktrees are eligible immediately. When it is `true`, the retention-day value
applies. Existing explicit configuration remains valid.

Run artifact compaction continues to use `runs.compact_keep_last`, whose
default remains 50. Automatic worktree cleanup must not silently reinterpret
that value as a changeset-retention limit.

## CLI

Add one command over the shared cleanup engine:

```bash
harness-symphony runs cleanup
harness-symphony runs cleanup --dry-run
```

The report lists each candidate, its reason, whether it was removed, and the
best-effort number of bytes reclaimed. `--dry-run` performs discovery and
safety checks without filesystem or Git mutations. Re-running cleanup is
idempotent.

## Lifecycle Integration

The cleanup engine runs at three points:

1. Symphony startup performs a sweep for expired terminal and orphan
   worktrees, then compacts terminal run evidence to the configured limit.
2. A run that successfully reaches Done through sync attempts immediate
   worktree cleanup when `cleanup_after_sync` is enabled, then applies the same
   terminal-evidence limit.
3. The CLI command allows explicit inspection or recovery at any time.

Only terminal records are candidates for worktree or evidence cleanup.
Prepared or running worktrees and their run folders are never removed. If
state and filesystem observations disagree, the engine chooses the
non-destructive result and reports why the candidate was skipped.

## Orphan Detection And Safety

An orphan candidate must satisfy every condition below:

- its canonical path is inside the configured worktree directory;
- it is not the repository root;
- it is not referenced by a prepared or running run;
- it is older than the failed-worktree retention window;
- it is either registered by `git worktree list` or matches a Symphony run
  directory that has no live state record.

Registered worktrees are removed with `git worktree remove --force`, followed
by `git worktree prune`. A remaining directory may be removed only after the
containment checks succeed. Symlinks must not permit traversal outside the
configured root.

## Failure Handling

Cleanup is best effort when invoked from startup or sync. A cleanup failure is
reported as a warning and retained for the next sweep; it must not turn a
successfully completed run into a failed run. The explicit CLI command exits
non-zero if any requested deletion fails and reports successful and failed
candidates separately.

State records and run evidence remain readable after their worktree is gone.
The stored worktree path may therefore refer to a cleaned path; run display
must make that state clear rather than treating it as corruption.

## Validation

- Unit tests cover eligibility by run status, age, configuration, containment,
  active-run protection, and idempotence.
- Integration tests use temporary Git repositories to prove registered
  worktree removal, branch preservation, orphan handling, dry-run behavior,
  and retry after partial failure.
- Sync tests prove Done cleanup is attempted only after successful sync and
  that cleanup failure does not reverse sync success.
- Installer tests prove Bash and PowerShell fresh installs merge every runtime
  ignore rule without duplicating or overwriting target content.
- A fresh-install smoke proves `.symphony/` and local `.harness/runs/` are
  ignored while `.harness/changesets/*.changeset.jsonl` remains visible to Git.

## Delivery Boundary

The source template, both installers, Symphony configuration and cleanup
engine, CLI documentation, and retention product contract are in scope. No
existing worktrees are removed merely by installing an update; cleanup occurs
when the updated Symphony starts or when the operator invokes the command.
