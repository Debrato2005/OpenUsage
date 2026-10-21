-- Historical packets start without reset proof. New packets link their immediate predecessor.
ALTER TABLE quota_snapshots ADD COLUMN previous_quota_run_id TEXT REFERENCES quota_snapshots(id) ON DELETE RESTRICT;
CREATE UNIQUE INDEX quota_previous_run ON quota_snapshots(previous_quota_run_id) WHERE previous_quota_run_id IS NOT NULL;
CREATE TABLE quota_reset_assessments (
 collection_run_id TEXT NOT NULL,
 profile_id TEXT NOT NULL,
 binding_id TEXT NOT NULL,
 limit_id TEXT NOT NULL,
 window_role TEXT NOT NULL CHECK(window_role IN ('primary','secondary')),
 state TEXT NOT NULL CHECK(state IN ('not_confirmed','confirmed')),
 pending_reset_at_utc TEXT,
 generation_run_id TEXT REFERENCES quota_snapshots(id) ON DELETE RESTRICT,
 PRIMARY KEY(collection_run_id,limit_id,window_role),
 FOREIGN KEY(collection_run_id,profile_id,binding_id,limit_id) REFERENCES quota_limits(collection_run_id,profile_id,binding_id,limit_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO quota_reset_assessments
 SELECT collection_run_id,profile_id,binding_id,limit_id,'primary','not_confirmed',primary_resets_at_utc,NULL
 FROM quota_limits WHERE primary_used_percent IS NOT NULL;
INSERT INTO quota_reset_assessments
 SELECT collection_run_id,profile_id,binding_id,limit_id,'secondary','not_confirmed',secondary_resets_at_utc,NULL
 FROM quota_limits WHERE secondary_used_percent IS NOT NULL;
