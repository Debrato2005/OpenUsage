-- One shared delivery identity, with quota and operational evidence kept in
-- separate constrained children. Existing quota proof constraints stay intact.
CREATE TABLE notification_event_ids (id TEXT PRIMARY KEY NOT NULL) STRICT;
INSERT INTO notification_event_ids SELECT id FROM alert_events;
CREATE TABLE new_alert_events (
 id TEXT PRIMARY KEY NOT NULL REFERENCES notification_event_ids(id) ON DELETE RESTRICT,
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
INSERT INTO new_alert_events SELECT * FROM alert_events;
DROP TABLE alert_events;
ALTER TABLE new_alert_events RENAME TO alert_events;
CREATE INDEX alert_history_order ON alert_events(evidence_at_utc DESC,id DESC);
CREATE TABLE new_reset_notification_outbox (
 event_id TEXT PRIMARY KEY NOT NULL REFERENCES notification_event_ids(id) ON DELETE RESTRICT,
 state TEXT NOT NULL CHECK(state IN ('queued','uncertain','submitted','failed','suppressed')),
 attempt_id TEXT,
 attempted_at_utc TEXT,
 completed_at_utc TEXT,
 CHECK((state IN ('queued','suppressed') AND attempt_id IS NULL AND attempted_at_utc IS NULL AND completed_at_utc IS NULL)
    OR (state='uncertain' AND attempt_id IS NOT NULL AND attempted_at_utc IS NOT NULL AND completed_at_utc IS NULL)
    OR (state IN ('submitted','failed') AND attempt_id IS NOT NULL AND attempted_at_utc IS NOT NULL AND completed_at_utc IS NOT NULL))
) STRICT;
INSERT INTO new_reset_notification_outbox SELECT * FROM reset_notification_outbox;
DROP TABLE reset_notification_outbox;
ALTER TABLE new_reset_notification_outbox RENAME TO reset_notification_outbox;
CREATE TABLE operational_alert_outcomes (
 id TEXT PRIMARY KEY NOT NULL,
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 home_reference TEXT NOT NULL,
 binding_id TEXT,
 source_kind TEXT NOT NULL CHECK(source_kind IN ('official_artifact','test_fixture')),
 provider_version TEXT NOT NULL CHECK(length(provider_version) BETWEEN 1 AND 128),
 observed_at_utc TEXT NOT NULL,
 accepted_at_utc TEXT NOT NULL,
 outcome TEXT NOT NULL CHECK(outcome IN ('authentication_required','transient_collection_failure','accepted_success')),
 incident_id TEXT,
 accepted_run_id TEXT REFERENCES collection_runs(id) ON DELETE RESTRICT,
 transient_failures INTEGER NOT NULL CHECK(transient_failures BETWEEN 0 AND 3),
 accepted_order INTEGER NOT NULL UNIQUE CHECK(accepted_order>0),
 FOREIGN KEY(binding_id,profile_id) REFERENCES profile_identity_events(id,profile_id) ON DELETE RESTRICT,
 CHECK((outcome='accepted_success' AND accepted_run_id IS NOT NULL AND incident_id IS NULL)
    OR (outcome!='accepted_success' AND accepted_run_id IS NULL AND incident_id IS NOT NULL))
) STRICT;
CREATE INDEX operational_outcomes_scope ON operational_alert_outcomes(profile_id,home_reference,binding_id,source_kind,observed_at_utc,id);
-- Like the submission journal, this local incident state is not rewound by
-- restore or Undo. It contains no credentials and has no replaceable-history FK.
CREATE TABLE operational_alert_state (
 scope_key TEXT PRIMARY KEY NOT NULL,
 profile_id TEXT NOT NULL,
 home_reference TEXT NOT NULL,
 binding_id TEXT,
 source_kind TEXT NOT NULL CHECK(source_kind IN ('official_artifact','test_fixture')),
 authentication_incident_id TEXT,
 collection_incident_id TEXT,
 transient_failures INTEGER NOT NULL CHECK(transient_failures BETWEEN 0 AND 3),
 last_request_id TEXT NOT NULL,
 last_evidence_at_utc TEXT NOT NULL,
 CHECK((transient_failures=0 AND collection_incident_id IS NULL) OR (transient_failures>0 AND collection_incident_id IS NOT NULL))
) STRICT;
CREATE TABLE operational_alert_events (
 id TEXT PRIMARY KEY NOT NULL REFERENCES notification_event_ids(id) ON DELETE RESTRICT,
 event_key TEXT NOT NULL UNIQUE CHECK(length(event_key) BETWEEN 1 AND 8192),
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 scope_key TEXT NOT NULL,
 incident_id TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('authentication_required','collection_failure')),
 evidence_outcome_id TEXT NOT NULL REFERENCES operational_alert_outcomes(id) ON DELETE RESTRICT,
 evidence_at_utc TEXT NOT NULL,
 source_kind TEXT NOT NULL CHECK(source_kind IN ('official_artifact','test_fixture'))
) STRICT;
CREATE INDEX operational_alert_history ON operational_alert_events(evidence_at_utc DESC,id DESC);
CREATE TABLE operational_alert_preferences (
 kind TEXT PRIMARY KEY NOT NULL CHECK(kind IN ('authentication_required','collection_failure')),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1))
) STRICT;
CREATE TABLE operational_alert_overrides (
 profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 kind TEXT NOT NULL CHECK(kind IN ('authentication_required','collection_failure')),
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 PRIMARY KEY(profile_id,kind)
) STRICT;
