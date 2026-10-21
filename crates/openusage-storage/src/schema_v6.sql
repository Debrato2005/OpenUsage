-- Notification delivery is separate from the immutable accepted-reset evidence.
-- Migration never queues historical events. Notifications require explicit opt-in.
CREATE TABLE reset_alert_preferences (
 id INTEGER PRIMARY KEY NOT NULL CHECK(id=1),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 quiet_enabled INTEGER NOT NULL CHECK(quiet_enabled IN (0,1)),
 time_zone TEXT NOT NULL CHECK(length(time_zone) BETWEEN 1 AND 128),
 quiet_start_minute INTEGER NOT NULL CHECK(quiet_start_minute BETWEEN 0 AND 1439),
 quiet_end_minute INTEGER NOT NULL CHECK(quiet_end_minute BETWEEN 0 AND 1439),
 CHECK(quiet_start_minute!=quiet_end_minute)
) STRICT;
CREATE TABLE reset_alert_overrides (
 profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
) STRICT;
CREATE TABLE reset_notification_outbox (
 event_id TEXT PRIMARY KEY NOT NULL REFERENCES alert_events(id) ON DELETE RESTRICT,
 state TEXT NOT NULL CHECK(state IN ('queued','uncertain','submitted','failed','suppressed')),
 attempt_id TEXT,
 attempted_at_utc TEXT,
 completed_at_utc TEXT,
 CHECK((state IN ('queued','suppressed') AND attempt_id IS NULL AND attempted_at_utc IS NULL AND completed_at_utc IS NULL)
    OR (state='uncertain' AND attempt_id IS NOT NULL AND attempted_at_utc IS NOT NULL AND completed_at_utc IS NULL)
    OR (state IN ('submitted','failed') AND attempt_id IS NOT NULL AND attempted_at_utc IS NOT NULL AND completed_at_utc IS NOT NULL))
) STRICT;
-- This no-FK journal survives archive restore and Undo. A consumed external side
-- effect cannot be undone by replacing profiles or restoring an older database.
CREATE TABLE reset_notification_receipts (
 event_key TEXT PRIMARY KEY NOT NULL CHECK(length(event_key) BETWEEN 1 AND 8192),
 attempt_id TEXT NOT NULL UNIQUE,
 state TEXT NOT NULL CHECK(state IN ('uncertain','submitted','failed')),
 attempted_at_utc TEXT NOT NULL,
 completed_at_utc TEXT,
 CHECK((state='uncertain' AND completed_at_utc IS NULL) OR (state IN ('submitted','failed') AND completed_at_utc IS NOT NULL))
) STRICT;
