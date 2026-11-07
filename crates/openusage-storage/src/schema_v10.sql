-- Local log totals from the pinned ccusage binary (SRS Sections 5C and 13).
-- Counts only: never a prompt, response, tool output or code (OU-LOG-003).
-- Token counts are exact decimal text. Rust checks every value before a write and after a read.
CREATE TABLE log_sources (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 tool TEXT NOT NULL CHECK(length(tool) BETWEEN 1 AND 32 AND tool NOT GLOB '*[^a-z0-9-]*'),
 -- The resolved folder that a scan reads. One folder is scanned once, whatever links lead to it (OU-DUP-008).
 folder TEXT NOT NULL CHECK(length(folder) BETWEEN 1 AND 4096),
 consent TEXT NOT NULL CHECK(consent IN ('granted','withdrawn')),
 -- The first saved scan pins the zone for day boundaries (OU-LOG-010).
 time_zone TEXT CHECK(time_zone IS NULL OR length(time_zone) BETWEEN 1 AND 128),
 last_scan_at_utc TEXT,
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 UNIQUE(tool,folder),
 UNIQUE(id,tool)
) STRICT;
CREATE TABLE local_usage_days (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 log_source_id TEXT NOT NULL,
 tool TEXT NOT NULL,
 model TEXT NOT NULL CHECK(length(model) BETWEEN 1 AND 128),
 local_date TEXT NOT NULL CHECK(length(local_date)=10),
 input_tokens TEXT NOT NULL CHECK(length(input_tokens) BETWEEN 1 AND 19 AND input_tokens NOT GLOB '*[^0-9]*'),
 output_tokens TEXT NOT NULL CHECK(length(output_tokens) BETWEEN 1 AND 19 AND output_tokens NOT GLOB '*[^0-9]*'),
 cache_write_tokens TEXT NOT NULL CHECK(length(cache_write_tokens) BETWEEN 1 AND 19 AND cache_write_tokens NOT GLOB '*[^0-9]*'),
 cache_read_tokens TEXT NOT NULL CHECK(length(cache_read_tokens) BETWEEN 1 AND 19 AND cache_read_tokens NOT GLOB '*[^0-9]*'),
 total_tokens TEXT NOT NULL CHECK(length(total_tokens) BETWEEN 1 AND 19 AND total_tokens NOT GLOB '*[^0-9]*'),
 -- An estimate in millionths of a US dollar. It is not money and is never added to money (OU-LOG-014).
 estimated_value_usd_micros TEXT CHECK(estimated_value_usd_micros IS NULL OR (length(estimated_value_usd_micros) BETWEEN 1 AND 19 AND estimated_value_usd_micros NOT GLOB '*[^0-9]*')),
 -- The ccusage version of the scan that gave the saved counts (OU-LOG-007).
 scanner_version TEXT NOT NULL CHECK(length(scanner_version) BETWEEN 1 AND 32),
 -- logs_gone: a later scan read less, so the saved counts stay and the difference is saved (OU-LOG-011).
 coverage TEXT NOT NULL CHECK(coverage IN ('complete','logs_gone')),
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 FOREIGN KEY(log_source_id,tool) REFERENCES log_sources(id,tool),
 UNIQUE(log_source_id,local_date,model)
) STRICT;
CREATE INDEX local_usage_days_by_date ON local_usage_days(local_date);
CREATE TABLE local_usage_differences (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 log_source_id TEXT NOT NULL,
 local_date TEXT NOT NULL,
 model TEXT NOT NULL,
 saved_total_tokens TEXT NOT NULL CHECK(length(saved_total_tokens) BETWEEN 1 AND 19 AND saved_total_tokens NOT GLOB '*[^0-9]*'),
 new_total_tokens TEXT NOT NULL CHECK(length(new_total_tokens) BETWEEN 1 AND 19 AND new_total_tokens NOT GLOB '*[^0-9]*'),
 scan_at_utc TEXT NOT NULL,
 FOREIGN KEY(log_source_id,local_date,model) REFERENCES local_usage_days(log_source_id,local_date,model),
 -- A repeated lower scan saves no second copy of the same difference.
 UNIQUE(log_source_id,local_date,model,saved_total_tokens,new_total_tokens)
) STRICT;
