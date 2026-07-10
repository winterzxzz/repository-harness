# Clean Harness Installer Payload Design

## Problem

Fresh Harness installations currently copy a small amount of content that
belongs to the development history of `repository-harness`, not to the target
project. In particular, the shared installer manifest includes the source
repository's root `README.md` and numbered decision records created while
building Harness.

The installer already excludes local databases, Symphony run artifacts,
changesets, and project story history. The remaining boundary should be made
explicit and mechanically checked.

## Desired Outcome

A newly installed project receives the reusable Harness operating kit but no
task history or project identity from `repository-harness`.

The installed operating kit includes:

- the stable agent shim;
- reusable Harness policies and context rules;
- empty product, story, decision, backlog, and test-matrix scaffolds;
- templates;
- schema migrations; and
- the platform Harness CLI.

The installed project then creates and retains only its own product docs,
stories, decisions, traces, changesets, and run artifacts.

## Scope

### Included

- Remove the source repository root `README.md` from the installer payload.
- Remove numbered `docs/decisions/*.md` records from the installer payload.
- Keep `docs/decisions/README.md` as the empty decision-log scaffold.
- Keep reusable policy documents, templates, schema migrations, and empty
  project-data scaffolds.
- Add an automated payload-boundary check shared by local validation and the
  story verification command.
- Update installer documentation to explain the operating-kit versus
  project-generated-data boundary.

### Excluded

- Deleting or rewriting files in this source repository.
- Removing numbered decisions from an already-installed project.
- Changing `--merge` or `--override` conflict behavior.
- Disabling future project-local docs, decisions, traces, changesets, or run
  artifacts.
- Changing database schema or `harness-cli init`; initialization already
  creates an empty operational database.

## Approaches Considered

### 1. Tighten the shared manifest

Keep one allowlist for both Bash and PowerShell installers, remove historical
entries, and validate the boundary directly.

This is the selected approach because it is small, explicit, and preserves the
existing single-source payload model.

### 2. Maintain a separate installer template tree

Copy reusable files into a dedicated template directory and install from that
tree. This creates a clean conceptual boundary but duplicates policy content
and introduces synchronization risk.

### 3. Install everything and scrub afterward

Copy the current payload and delete historical files at the end. This makes
merge behavior harder to reason about and risks deleting target-owned data.

## Design

### Payload Boundary

`scripts/harness-install-files.txt` remains the source of truth for stable,
non-schema files used by both installers.

Allowed categories are:

- agent entrypoints;
- reusable policy and reference docs;
- empty project-data scaffolds;
- templates;
- installer-facing CLI documentation; and
- `.gitignore` rules for local runtime state.

Forbidden categories are:

- the source repository root `README.md`;
- numbered decision records;
- story packets created for `repository-harness` work;
- `.harness/changesets` and `.harness/runs`;
- `harness.db` and SQLite sidecar files; and
- source-repository planning or review artifacts.

Schema migrations remain discovered independently from `scripts/schema/*.sql`.

### Fresh Install Flow

1. The installer reads the shared payload manifest.
2. It copies only reusable operating-kit files.
3. It discovers and copies schema migrations.
4. It installs the platform CLI.
5. No operational database or project history is created by installation.
6. The target project creates its own durable records when Harness commands
   are used later.

### Existing Install Flow

`--merge` continues to preserve all existing target files and only creates
missing allowlisted files. Removing an entry from the source manifest does not
delete that path from existing projects.

`--override` replaces only currently declared protected payload content; it
does not add a cleanup pass for historical files that may already exist.

## Validation

Add a repository validation script that fails when:

- the payload includes root `README.md`;
- the payload includes a numbered decision record;
- the payload includes runtime state or history paths;
- required core policies are missing;
- required empty scaffolds are missing; or
- the Bash installer has invalid shell syntax.

Also run a dry-run fresh-install smoke and assert that reusable policies are
reported while historical files are absent. PowerShell continues to consume
the same manifest, so manifest assertions cover the cross-platform payload
boundary even when a PowerShell runtime is unavailable locally.

## Risks And Mitigations

- A useful policy might be mistaken for history. Mitigation: required-policy
  assertions name the files that must remain installed.
- Future contributors might re-add source history. Mitigation: the validation
  script rejects forbidden path classes.
- Existing installations might expect the copied root README. Mitigation:
  existing files are untouched; only fresh-install payload behavior changes.

