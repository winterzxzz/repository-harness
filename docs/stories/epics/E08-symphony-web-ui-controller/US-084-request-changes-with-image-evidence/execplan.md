# Exec Plan

## Goal

Allow a reviewer to request changes on Ready work with a required reason and
optional image evidence, immediately launching a replacement run for the same
story while preserving all prior evidence.

## Scope

In scope:

- Bounded HTTP request reading and multipart parsing.
- PNG/JPEG/WebP signature and size validation.
- Feedback-aware replacement run preparation and cleanup.
- Run contract and agent prompt feedback fields.
- Request-changes API and Ready review UI.
- Historical feedback presentation.
- Rust, Playwright, desktop, and workspace proof.

Out of scope:

- Done-task reopening.
- New board buckets or story statuses.
- Arbitrary attachments, cloud storage, or PR amendment semantics.
- Broad HTTP framework replacement.

## Risk Classification

Risk flags:

- Audit/security: untrusted binary upload and filesystem writes.
- Public contracts: new Web API and UI behavior.
- Existing behavior: replaces the current reject flow.
- Data retention: evidence follows local run cleanup.
- Cross-platform: browser and Electron share upload behavior.
- Weak proof: no existing multipart or image-upload tests.

Hard gates:

- Audit/security.
- Bounded upload and path traversal prevention.
- No source-run mutation before validated replacement preparation.

## Work Phases

1. Add failing Rust tests for bounded binary request reading and multipart
   validation.
2. Implement the minimal request reader and multipart parser.
3. Add failing run-contract/state tests for request changes and rollback.
4. Implement feedback-aware replacement run preparation and prompt wiring.
5. Add API tests for eligibility, validation, atomicity, and error responses.
6. Add failing Playwright coverage for the Ready request-changes flow.
7. Implement file picker, previews, submission, errors, and feedback history.
8. Run desktop/browser/workspace verification and update durable proof.

## Stop Conditions

Pause for human confirmation if:

- Supporting Done-task reopen becomes necessary.
- Evidence must leave the local machine or enter Git.
- An HTTP framework replacement becomes necessary rather than a bounded reader.
- Existing run-state storage cannot support rollback without a schema change.
- Validation limits must be weakened.
