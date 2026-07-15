-- Harness v0 schema - migration 009
-- Story-level independent E2E command (US-101).

ALTER TABLE story ADD COLUMN e2e_command TEXT;

INSERT INTO schema_version (version) VALUES (9);
