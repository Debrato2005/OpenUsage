-- Existing observations retain an explicit evidence gap. No response evidence is invented.
CREATE TABLE reading_evidence_legacy (
 collection_run_id TEXT PRIMARY KEY NOT NULL REFERENCES collection_runs(id) ON DELETE CASCADE
) STRICT;
INSERT INTO reading_evidence_legacy(collection_run_id) SELECT id FROM collection_runs;

CREATE TABLE reading_evidence (
 collection_run_id TEXT PRIMARY KEY NOT NULL REFERENCES collection_runs(id) ON DELETE CASCADE,
 adapter_id TEXT NOT NULL CHECK(adapter_id='codex_app_server'),
 adapter_revision INTEGER NOT NULL CHECK(adapter_revision BETWEEN 1 AND 65535),
 projection_version INTEGER NOT NULL CHECK(projection_version=1),
 source_level TEXT NOT NULL CHECK(source_level='official'),
 reading_status TEXT NOT NULL CHECK(reading_status='official'),
 check_results TEXT NOT NULL CHECK(length(check_results) BETWEEN 2 AND 512),
 comparison_result TEXT NOT NULL CHECK(comparison_result='not_performed'),
 redacted_response_hash TEXT NOT NULL CHECK(length(redacted_response_hash)=64 AND redacted_response_hash NOT GLOB '*[^0-9a-f]*'),
 format_fingerprint TEXT NOT NULL CHECK(length(format_fingerprint)=64 AND format_fingerprint NOT GLOB '*[^0-9a-f]*'),
 unused_fields TEXT NOT NULL CHECK(length(unused_fields) BETWEEN 2 AND 2048)
) STRICT;
CREATE TRIGGER reading_evidence_immutable BEFORE UPDATE ON reading_evidence
BEGIN SELECT RAISE(ABORT,'immutable reading evidence'); END;
CREATE TRIGGER reading_evidence_legacy_immutable BEFORE UPDATE ON reading_evidence_legacy
BEGIN SELECT RAISE(ABORT,'immutable evidence availability'); END;
CREATE TRIGGER reading_evidence_no_replacement BEFORE INSERT ON reading_evidence
WHEN EXISTS(SELECT 1 FROM reading_evidence WHERE collection_run_id=NEW.collection_run_id)
BEGIN SELECT RAISE(ABORT,'immutable reading evidence'); END;
CREATE TRIGGER reading_evidence_legacy_no_replacement BEFORE INSERT ON reading_evidence_legacy
WHEN EXISTS(SELECT 1 FROM reading_evidence_legacy WHERE collection_run_id=NEW.collection_run_id)
BEGIN SELECT RAISE(ABORT,'immutable evidence availability'); END;
CREATE TRIGGER reading_evidence_excludes_legacy BEFORE INSERT ON reading_evidence
WHEN EXISTS(SELECT 1 FROM reading_evidence_legacy WHERE collection_run_id=NEW.collection_run_id)
BEGIN SELECT RAISE(ABORT,'conflicting evidence availability'); END;
CREATE TRIGGER reading_evidence_legacy_excludes_complete BEFORE INSERT ON reading_evidence_legacy
WHEN EXISTS(SELECT 1 FROM reading_evidence WHERE collection_run_id=NEW.collection_run_id)
BEGIN SELECT RAISE(ABORT,'conflicting evidence availability'); END;
