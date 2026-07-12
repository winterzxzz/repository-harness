# Harness Audit

`scripts/bin/harness-cli audit` detects drift in durable Harness state and
prints an entropy score. Lower is better.

## Checks

| Category | Meaning | Weight |
| --- | --- | --- |
| Orphaned stories | Planned or in-progress stories with no linked trace. | 10 |
| Unverified stories | Active or implemented stories with `verify_command` but no recorded verification result. Retired stories are historical records and are not counted. | 5 |
| Unverified decisions | Decisions with `verify_command` but no recorded verification result. | 5 |
| Markdown decisions missing durable records | Numbered decision Markdown files that are not represented by a durable decision row. | 3 |
| Open backlog without outcomes | Implemented backlog items with predicted impact but no actual outcome. | 2 |
| Stale stories | Unimplemented stories whose latest linked trace is more than 30 days old. | 3 |
| Broken tools | Registered tools whose command is not found on disk or `PATH`. | 8 |
| Unresolved harness friction | Trace records with actionable friction that is not an already-resolved provider gap. | 2 |

## Score

```text
score = orphaned_stories * 10
      + unverified_stories * 5
      + unverified_decisions * 5
      + untracked_decisions * 3
      + backlog_without_outcomes * 2
      + stale_stories * 3
      + broken_tools * 8
      + unresolved_friction * 2
```

The score is capped at 100.

| Range | Interpretation |
| --- | --- |
| 0 | Perfect: records are traced, verified, and healthy. |
| 1-25 | Healthy: minor housekeeping remains. |
| 26-50 | Attention needed: drift is accumulating. |
| 51-100 | Action required: stale state undermines Harness value. |

Audit findings feed `scripts/bin/harness-cli propose`, which turns repeated
patterns and otherwise-unresolved friction into proposed backlog items. This
keeps one-off failures visible without pretending they have high confidence.
