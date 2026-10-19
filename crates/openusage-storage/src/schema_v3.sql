-- Trusted rebuild; migration runner disables FK enforcement only on its private connection.
CREATE TABLE new_collection_runs (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    binding_id TEXT NOT NULL,
    started_at_utc TEXT NOT NULL,
    completed_at_utc TEXT NOT NULL,
    observed_at_utc TEXT NOT NULL,
    source_method TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('official_artifact','test_fixture')),
    source_scope TEXT NOT NULL,
    raw_scope TEXT CHECK(raw_scope IS NULL),
    normalized_unit TEXT NOT NULL,
    coverage TEXT NOT NULL CHECK(coverage='unknown'),
    identity_confidence TEXT NOT NULL CHECK(identity_confidence='provisional'),
    verified_identity_key TEXT CHECK(verified_identity_key IS NULL),
    outcome TEXT NOT NULL CHECK(outcome='accepted'),
    CHECK((source_method='account/usage/read' AND normalized_unit='tokens' AND source_scope='account_activity_with_unknown_application_scope') OR (source_method='account/rateLimits/read' AND normalized_unit='percent' AND source_scope='account_quota_with_unknown_application_scope')),
    UNIQUE(id,profile_id,binding_id),
    FOREIGN KEY(binding_id,profile_id) REFERENCES profile_identity_events(id,profile_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO new_collection_runs(id,profile_id,binding_id,started_at_utc,completed_at_utc,observed_at_utc,source_method,provider_version,source_kind,source_scope,raw_scope,normalized_unit,coverage,identity_confidence,verified_identity_key,outcome) SELECT id,profile_id,binding_id,started_at_utc,completed_at_utc,observed_at_utc,source_method,provider_version,source_kind,source_scope,raw_scope,normalized_unit,coverage,identity_confidence,verified_identity_key,outcome FROM collection_runs;
DROP TABLE collection_runs;
ALTER TABLE new_collection_runs RENAME TO collection_runs;
CREATE INDEX activity_summary_profile_time ON collection_runs(profile_id,observed_at_utc,id);
CREATE TABLE profile_bindings (
 profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 binding_id TEXT NOT NULL,
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 UNIQUE(profile_id,binding_id),
 FOREIGN KEY(binding_id,profile_id) REFERENCES profile_identity_events(id,profile_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO profile_bindings SELECT profile_id,binding_id,created_at_utc,updated_at_utc FROM current_activity;
CREATE TABLE quota_snapshots (
 id TEXT PRIMARY KEY NOT NULL,
 profile_id TEXT NOT NULL,
 binding_id TEXT NOT NULL,
 available_reset_credits TEXT,
 credit_details_state TEXT NOT NULL CHECK(credit_details_state IN ('unavailable','empty','returned')),
 UNIQUE(id,profile_id,binding_id),
 FOREIGN KEY(id,profile_id,binding_id) REFERENCES collection_runs(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE quota_limits (
 collection_run_id TEXT NOT NULL,
 profile_id TEXT NOT NULL,
 binding_id TEXT NOT NULL,
 limit_id TEXT NOT NULL,
 limit_name TEXT,
 reached_type TEXT,
 monthly_credit_limit TEXT,
 spend_control_reached INTEGER CHECK(spend_control_reached IN (0,1)),
 primary_used_percent INTEGER CHECK(primary_used_percent BETWEEN 0 AND 100),
 primary_duration_mins INTEGER CHECK(primary_duration_mins BETWEEN 1 AND 9007199254740991),
 primary_resets_at_utc TEXT CHECK(primary_resets_at_utc BETWEEN '0001-01-01T00:00:00.000Z' AND '9999-12-31T23:59:59.000Z'),
 secondary_used_percent INTEGER CHECK(secondary_used_percent BETWEEN 0 AND 100),
 secondary_duration_mins INTEGER CHECK(secondary_duration_mins BETWEEN 1 AND 9007199254740991),
 secondary_resets_at_utc TEXT CHECK(secondary_resets_at_utc BETWEEN '0001-01-01T00:00:00.000Z' AND '9999-12-31T23:59:59.000Z'),
 CHECK(primary_used_percent IS NOT NULL OR (primary_duration_mins IS NULL AND primary_resets_at_utc IS NULL)),
 CHECK(secondary_used_percent IS NOT NULL OR (secondary_duration_mins IS NULL AND secondary_resets_at_utc IS NULL)),
 PRIMARY KEY(collection_run_id,limit_id),
 UNIQUE(collection_run_id,profile_id,binding_id,limit_id),
 FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES quota_snapshots(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE credit_snapshots (
 collection_run_id TEXT NOT NULL,
 profile_id TEXT NOT NULL,
 binding_id TEXT NOT NULL,
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 31),
 title TEXT,
 status TEXT NOT NULL CHECK(status IN ('available','redeeming','redeemed','unknown')),
 granted_at_unix INTEGER NOT NULL CHECK(granted_at_unix BETWEEN -62135596800 AND 253402300799),
 expires_at_unix INTEGER CHECK(expires_at_unix BETWEEN -62135596800 AND 253402300799),
 PRIMARY KEY(collection_run_id,ordinal),
 FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES quota_snapshots(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE current_quota_readings (
 profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
 binding_id TEXT NOT NULL,
 collection_run_id TEXT NOT NULL,
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 UNIQUE(profile_id,binding_id,collection_run_id),
 FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES quota_snapshots(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE current_quota (
 profile_id TEXT NOT NULL,
 limit_id TEXT NOT NULL,
 window_role TEXT NOT NULL CHECK(window_role IN ('primary','secondary')),
 binding_id TEXT NOT NULL,
 collection_run_id TEXT NOT NULL,
 used_percent INTEGER NOT NULL CHECK(used_percent BETWEEN 0 AND 100),
 window_duration_mins INTEGER CHECK(window_duration_mins BETWEEN 1 AND 9007199254740991),
 resets_at_utc TEXT CHECK(resets_at_utc BETWEEN '0001-01-01T00:00:00.000Z' AND '9999-12-31T23:59:59.000Z'),
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 PRIMARY KEY(profile_id,limit_id,window_role),
 FOREIGN KEY(profile_id,binding_id,collection_run_id) REFERENCES current_quota_readings(profile_id,binding_id,collection_run_id) ON DELETE RESTRICT,
 FOREIGN KEY(collection_run_id,profile_id,binding_id,limit_id) REFERENCES quota_limits(collection_run_id,profile_id,binding_id,limit_id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX quota_profile_time ON collection_runs(profile_id,source_method,observed_at_utc,id);
