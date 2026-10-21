-- Record only newly accepted reset proofs. Existing history is not replayed.
CREATE TABLE alert_events (
 id TEXT PRIMARY KEY NOT NULL,
 event_key TEXT NOT NULL UNIQUE CHECK(length(event_key) BETWEEN 1 AND 8192),
 profile_id TEXT NOT NULL,
 binding_id TEXT NOT NULL,
 verified_identity_key TEXT,
 limit_id TEXT NOT NULL,
 window_role TEXT NOT NULL CHECK(window_role IN ('primary','secondary')),
 window_duration_mins INTEGER NOT NULL CHECK(window_duration_mins BETWEEN 1 AND 9007199254740991),
 generation_run_id TEXT NOT NULL,
 evidence_at_utc TEXT NOT NULL,
 source_kind TEXT NOT NULL CHECK(source_kind IN ('official_artifact','test_fixture')),
 delivery_state TEXT NOT NULL CHECK(delivery_state='recorded'),
 FOREIGN KEY(generation_run_id,profile_id,binding_id,limit_id) REFERENCES quota_limits(collection_run_id,profile_id,binding_id,limit_id) ON DELETE RESTRICT,
 FOREIGN KEY(generation_run_id,limit_id,window_role) REFERENCES quota_reset_assessments(collection_run_id,limit_id,window_role) ON DELETE RESTRICT
) STRICT;
CREATE INDEX alert_history_order ON alert_events(evidence_at_utc DESC,id DESC);
