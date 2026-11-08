-- Hourly local totals from the pinned ccusage blocks report (OU-LOG-017, SC-021).
-- Only Claude Code reports hours. Each row covers one whole UTC hour.
CREATE TABLE local_usage_hours (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 log_source_id TEXT NOT NULL,
 tool TEXT NOT NULL,
 hour_start_utc TEXT NOT NULL CHECK(length(hour_start_utc)=24 AND substr(hour_start_utc,15)='00:00.000Z'),
 input_tokens TEXT NOT NULL CHECK(length(input_tokens) BETWEEN 1 AND 19 AND input_tokens NOT GLOB '*[^0-9]*'),
 output_tokens TEXT NOT NULL CHECK(length(output_tokens) BETWEEN 1 AND 19 AND output_tokens NOT GLOB '*[^0-9]*'),
 cache_write_tokens TEXT NOT NULL CHECK(length(cache_write_tokens) BETWEEN 1 AND 19 AND cache_write_tokens NOT GLOB '*[^0-9]*'),
 cache_read_tokens TEXT NOT NULL CHECK(length(cache_read_tokens) BETWEEN 1 AND 19 AND cache_read_tokens NOT GLOB '*[^0-9]*'),
 total_tokens TEXT NOT NULL CHECK(length(total_tokens) BETWEEN 1 AND 19 AND total_tokens NOT GLOB '*[^0-9]*'),
 scanner_version TEXT NOT NULL CHECK(length(scanner_version) BETWEEN 1 AND 32),
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 FOREIGN KEY(log_source_id,tool) REFERENCES log_sources(id,tool),
 UNIQUE(log_source_id,hour_start_utc)
) STRICT;
CREATE INDEX local_usage_hours_by_start ON local_usage_hours(hour_start_utc);
