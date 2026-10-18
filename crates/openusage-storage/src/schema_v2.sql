-- Provisional profile activity only. Local binding UUIDs are not provider identity keys.
CREATE TABLE profile_identity_events (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    home_reference TEXT NOT NULL,
    previous_binding_id TEXT,
    returned_email TEXT NOT NULL CHECK(length(returned_email) BETWEEN 1 AND 254),
    returned_plan TEXT,
    identity_confidence TEXT NOT NULL CHECK(identity_confidence='provisional'),
    verified_identity_key TEXT CHECK(verified_identity_key IS NULL),
    observed_at_utc TEXT NOT NULL,
    UNIQUE(id,profile_id),
    FOREIGN KEY(previous_binding_id,profile_id) REFERENCES profile_identity_events(id,profile_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE collection_runs (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    binding_id TEXT NOT NULL,
    started_at_utc TEXT NOT NULL,
    completed_at_utc TEXT NOT NULL,
    observed_at_utc TEXT NOT NULL,
    source_method TEXT NOT NULL CHECK(source_method='account/usage/read'),
    provider_version TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('official_artifact','test_fixture')),
    source_scope TEXT NOT NULL CHECK(source_scope='account_activity_with_unknown_application_scope'),
    raw_scope TEXT CHECK(raw_scope IS NULL),
    normalized_unit TEXT NOT NULL CHECK(normalized_unit='tokens'),
    coverage TEXT NOT NULL CHECK(coverage='unknown'),
    identity_confidence TEXT NOT NULL CHECK(identity_confidence='provisional'),
    verified_identity_key TEXT CHECK(verified_identity_key IS NULL),
    outcome TEXT NOT NULL CHECK(outcome='accepted'),
    UNIQUE(id,profile_id,binding_id),
    FOREIGN KEY(binding_id,profile_id) REFERENCES profile_identity_events(id,profile_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE activity_summaries (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    binding_id TEXT NOT NULL,
    collection_run_id TEXT NOT NULL UNIQUE,
    lifetime_tokens TEXT CHECK(lifetime_tokens IS NULL OR (length(lifetime_tokens) BETWEEN 1 AND 19 AND lifetime_tokens NOT GLOB '*[^0-9]*' AND (lifetime_tokens='0' OR substr(lifetime_tokens,1,1)!='0') AND (length(lifetime_tokens)<19 OR lifetime_tokens<='9223372036854775807'))),
    peak_daily_tokens TEXT CHECK(peak_daily_tokens IS NULL OR (length(peak_daily_tokens) BETWEEN 1 AND 19 AND peak_daily_tokens NOT GLOB '*[^0-9]*' AND (peak_daily_tokens='0' OR substr(peak_daily_tokens,1,1)!='0') AND (length(peak_daily_tokens)<19 OR peak_daily_tokens<='9223372036854775807'))),
    longest_running_turn_sec TEXT CHECK(longest_running_turn_sec IS NULL OR (length(longest_running_turn_sec) BETWEEN 1 AND 19 AND longest_running_turn_sec NOT GLOB '*[^0-9]*' AND (longest_running_turn_sec='0' OR substr(longest_running_turn_sec,1,1)!='0') AND (length(longest_running_turn_sec)<19 OR longest_running_turn_sec<='9223372036854775807'))),
    current_streak_days TEXT CHECK(current_streak_days IS NULL OR (length(current_streak_days) BETWEEN 1 AND 19 AND current_streak_days NOT GLOB '*[^0-9]*' AND (current_streak_days='0' OR substr(current_streak_days,1,1)!='0') AND (length(current_streak_days)<19 OR current_streak_days<='9223372036854775807'))),
    longest_streak_days TEXT CHECK(longest_streak_days IS NULL OR (length(longest_streak_days) BETWEEN 1 AND 19 AND longest_streak_days NOT GLOB '*[^0-9]*' AND (longest_streak_days='0' OR substr(longest_streak_days,1,1)!='0') AND (length(longest_streak_days)<19 OR longest_streak_days<='9223372036854775807'))),
    daily_state TEXT NOT NULL CHECK(daily_state IN ('unavailable','empty','returned')),
    UNIQUE(id,profile_id,binding_id),
    CHECK(id=collection_run_id),
    FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES collection_runs(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE current_activity (
    profile_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    binding_id TEXT NOT NULL,
    summary_id TEXT,
    collection_run_id TEXT,
    created_at_utc TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    CHECK(summary_id IS collection_run_id),
    FOREIGN KEY(binding_id,profile_id) REFERENCES profile_identity_events(id,profile_id) ON DELETE RESTRICT,
    FOREIGN KEY(summary_id,profile_id,binding_id) REFERENCES activity_summaries(id,profile_id,binding_id) ON DELETE RESTRICT,
    FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES collection_runs(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE current_activity_days (
    profile_id TEXT NOT NULL,
    provider_date TEXT NOT NULL,
    source_scope TEXT NOT NULL CHECK(source_scope='account_activity_with_unknown_application_scope'),
    binding_id TEXT NOT NULL,
    collection_run_id TEXT NOT NULL,
    tokens TEXT NOT NULL CHECK(tokens IS NULL OR (length(tokens) BETWEEN 1 AND 19 AND tokens NOT GLOB '*[^0-9]*' AND (tokens='0' OR substr(tokens,1,1)!='0') AND (length(tokens)<19 OR tokens<='9223372036854775807'))),
    observed_at_utc TEXT NOT NULL,
    created_at_utc TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    PRIMARY KEY(profile_id,provider_date,source_scope),
    FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES collection_runs(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE TABLE activity_day_history (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    provider_date TEXT NOT NULL,
    source_scope TEXT NOT NULL CHECK(source_scope='account_activity_with_unknown_application_scope'),
    binding_id TEXT NOT NULL,
    collection_run_id TEXT NOT NULL,
    prior_tokens TEXT CHECK(prior_tokens IS NULL OR (length(prior_tokens) BETWEEN 1 AND 19 AND prior_tokens NOT GLOB '*[^0-9]*' AND (prior_tokens='0' OR substr(prior_tokens,1,1)!='0') AND (length(prior_tokens)<19 OR prior_tokens<='9223372036854775807'))),
    accepted_tokens TEXT NOT NULL CHECK(accepted_tokens IS NULL OR (length(accepted_tokens) BETWEEN 1 AND 19 AND accepted_tokens NOT GLOB '*[^0-9]*' AND (accepted_tokens='0' OR substr(accepted_tokens,1,1)!='0') AND (length(accepted_tokens)<19 OR accepted_tokens<='9223372036854775807'))),
    observed_at_utc TEXT NOT NULL,
    UNIQUE(collection_run_id,provider_date,source_scope),
    FOREIGN KEY(collection_run_id,profile_id,binding_id) REFERENCES collection_runs(id,profile_id,binding_id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX activity_summary_profile_time ON collection_runs(profile_id,observed_at_utc,id);
CREATE INDEX activity_history_profile_date ON activity_day_history(profile_id,provider_date,source_scope,observed_at_utc,id);
