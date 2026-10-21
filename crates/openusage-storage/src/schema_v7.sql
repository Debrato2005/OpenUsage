-- Crossing evidence references the actual immutable observation, separately from
-- the earlier reset proof that established its window generation.
CREATE TABLE threshold_alert_events (
 event_id TEXT PRIMARY KEY NOT NULL REFERENCES alert_events(id) ON DELETE RESTRICT,
 threshold_percent INTEGER NOT NULL CHECK(threshold_percent IN (80,90,100)),
 evidence_run_id TEXT NOT NULL REFERENCES quota_snapshots(id) ON DELETE RESTRICT,
 accepted_at_utc TEXT NOT NULL
) STRICT;
CREATE TABLE threshold_alert_preferences (
 id INTEGER PRIMARY KEY NOT NULL CHECK(id=1),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
) STRICT;
CREATE TABLE threshold_alert_overrides (
 profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
) STRICT;
-- A covered threshold was consumed by a higher crossing; it was never submitted
-- to the OS. This journal deliberately has no foreign keys and survives restore.
ALTER TABLE reset_notification_receipts RENAME TO old_reset_notification_receipts;
CREATE TABLE reset_notification_receipts (
 event_key TEXT PRIMARY KEY NOT NULL CHECK(length(event_key) BETWEEN 1 AND 8192),
 attempt_id TEXT NOT NULL UNIQUE,
 state TEXT NOT NULL CHECK(state IN ('uncertain','submitted','failed','covered')),
 attempted_at_utc TEXT NOT NULL,
 completed_at_utc TEXT,
 window_key TEXT UNIQUE,
 CHECK((state='uncertain' AND completed_at_utc IS NULL) OR (state IN ('submitted','failed','covered') AND completed_at_utc IS NOT NULL))
) STRICT;
INSERT INTO reset_notification_receipts(event_key,attempt_id,state,attempted_at_utc,completed_at_utc) SELECT * FROM old_reset_notification_receipts;
DROP TABLE old_reset_notification_receipts;
