-- Grouped readings have their own scope. Legacy rows and zones stay unchanged (SC-024).
CREATE TABLE local_usage_scopes (
 tool TEXT PRIMARY KEY NOT NULL,
 period_kind TEXT NOT NULL CHECK(period_kind IN ('day','unplaced')),
 time_zone TEXT,
 CHECK((period_kind='day' AND time_zone IS NOT NULL) OR (period_kind='unplaced' AND time_zone IS NULL))
) STRICT;
CREATE TABLE local_usage_scans (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 tool TEXT NOT NULL REFERENCES local_usage_scopes(tool),
 scanner_version TEXT NOT NULL,
 normalization_version INTEGER NOT NULL CHECK(normalization_version=1),
 observed_at_utc TEXT NOT NULL,
 latest INTEGER NOT NULL CHECK(latest IN (0,1)),
 UNIQUE(id,tool)
) STRICT;
CREATE UNIQUE INDEX local_usage_latest_scan ON local_usage_scans(tool) WHERE latest=1;
CREATE TABLE local_usage_scan_roots (
 scan_id TEXT NOT NULL,
 tool TEXT NOT NULL,
 log_source_id TEXT NOT NULL,
 PRIMARY KEY(scan_id,log_source_id),
 FOREIGN KEY(scan_id,tool) REFERENCES local_usage_scans(id,tool) ON DELETE CASCADE,
 FOREIGN KEY(log_source_id,tool) REFERENCES log_sources(id,tool)
) STRICT;
-- Periods and sessions are separate kinds. Only day records enter dated queries.
CREATE TABLE local_usage_records (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 tool TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('day','unplaced','session')),
 period_key TEXT NOT NULL,
 scan_id TEXT NOT NULL,
 retained INTEGER NOT NULL CHECK(retained IN (0,1)),
 project TEXT,
 first_activity_utc TEXT,
 last_activity_utc TEXT,
 total_tokens TEXT CHECK(total_tokens IS NULL OR (length(total_tokens) BETWEEN 1 AND 16 AND total_tokens NOT GLOB '*[^0-9]*')),
 input_tokens TEXT CHECK(input_tokens IS NULL OR (length(input_tokens) BETWEEN 1 AND 16 AND input_tokens NOT GLOB '*[^0-9]*')),
 output_tokens TEXT CHECK(output_tokens IS NULL OR (length(output_tokens) BETWEEN 1 AND 16 AND output_tokens NOT GLOB '*[^0-9]*')),
 cache_write_tokens TEXT CHECK(cache_write_tokens IS NULL OR (length(cache_write_tokens) BETWEEN 1 AND 16 AND cache_write_tokens NOT GLOB '*[^0-9]*')),
 cache_read_tokens TEXT CHECK(cache_read_tokens IS NULL OR (length(cache_read_tokens) BETWEEN 1 AND 16 AND cache_read_tokens NOT GLOB '*[^0-9]*')),
 reasoning_tokens TEXT CHECK(reasoning_tokens IS NULL OR (length(reasoning_tokens) BETWEEN 1 AND 16 AND reasoning_tokens NOT GLOB '*[^0-9]*')),
 estimated_value_micros TEXT CHECK(estimated_value_micros IS NULL OR (length(estimated_value_micros) BETWEEN 1 AND 16 AND estimated_value_micros NOT GLOB '*[^0-9]*')),
 CHECK(total_tokens IS NOT NULL),
 CHECK((kind='unplaced' AND period_key='') OR (kind!='unplaced' AND length(period_key)>0)),
 CHECK(kind='session' OR (project IS NULL AND first_activity_utc IS NULL AND last_activity_utc IS NULL)),
 UNIQUE(tool,kind,period_key),
 UNIQUE(id,tool),
 FOREIGN KEY(scan_id,tool) REFERENCES local_usage_scans(id,tool)
) STRICT;
CREATE INDEX local_usage_records_by_period ON local_usage_records(kind,period_key);
CREATE TABLE local_usage_record_models (
 record_id TEXT NOT NULL REFERENCES local_usage_records(id) ON DELETE CASCADE,
 model TEXT NOT NULL,
 total_tokens TEXT CHECK(total_tokens IS NULL OR (length(total_tokens) BETWEEN 1 AND 16 AND total_tokens NOT GLOB '*[^0-9]*')),
 input_tokens TEXT CHECK(input_tokens IS NULL OR (length(input_tokens) BETWEEN 1 AND 16 AND input_tokens NOT GLOB '*[^0-9]*')),
 output_tokens TEXT CHECK(output_tokens IS NULL OR (length(output_tokens) BETWEEN 1 AND 16 AND output_tokens NOT GLOB '*[^0-9]*')),
 cache_write_tokens TEXT CHECK(cache_write_tokens IS NULL OR (length(cache_write_tokens) BETWEEN 1 AND 16 AND cache_write_tokens NOT GLOB '*[^0-9]*')),
 cache_read_tokens TEXT CHECK(cache_read_tokens IS NULL OR (length(cache_read_tokens) BETWEEN 1 AND 16 AND cache_read_tokens NOT GLOB '*[^0-9]*')),
 reasoning_tokens TEXT CHECK(reasoning_tokens IS NULL OR (length(reasoning_tokens) BETWEEN 1 AND 16 AND reasoning_tokens NOT GLOB '*[^0-9]*')),
 estimated_value_micros TEXT CHECK(estimated_value_micros IS NULL OR (length(estimated_value_micros) BETWEEN 1 AND 16 AND estimated_value_micros NOT GLOB '*[^0-9]*')),
 PRIMARY KEY(record_id,model)
) STRICT;
CREATE TABLE local_usage_scan_differences (
 record_id TEXT NOT NULL,
 tool TEXT NOT NULL,
 scan_id TEXT NOT NULL,
 saved_total_tokens TEXT NOT NULL,
 new_total_tokens TEXT NOT NULL,
 PRIMARY KEY(record_id,saved_total_tokens,new_total_tokens),
 FOREIGN KEY(record_id,tool) REFERENCES local_usage_records(id,tool),
 FOREIGN KEY(scan_id,tool) REFERENCES local_usage_scans(id,tool)
) STRICT;
