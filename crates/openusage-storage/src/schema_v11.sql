-- Local sessions from the pinned ccusage session report (SRS Section 13, SC-021).
-- Counts and labels only: never a prompt, response, tool output or code (OU-LOG-003).
CREATE TABLE local_usage_sessions (
 id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=36),
 log_source_id TEXT NOT NULL,
 tool TEXT NOT NULL,
 -- The session ID that ccusage reports. It is a key, never a path to open (OU-SEC-021).
 session_key TEXT NOT NULL CHECK(length(session_key) BETWEEN 1 AND 256),
 -- Only Claude Code reports a project: the folder name that it writes (SC-021).
 project_name TEXT CHECK(project_name IS NULL OR length(project_name) BETWEEN 1 AND 512),
 -- Null until the owner settles OQ-1 (OU-LOG-015).
 log_account_key TEXT CHECK(log_account_key IS NULL OR length(log_account_key) BETWEEN 1 AND 256),
 -- Codex reports no start, so it stays null (SC-021).
 start_utc TEXT CHECK(start_utc IS NULL OR length(start_utc)=24),
 end_utc TEXT NOT NULL CHECK(length(end_utc)=24),
 input_tokens TEXT NOT NULL CHECK(length(input_tokens) BETWEEN 1 AND 19 AND input_tokens NOT GLOB '*[^0-9]*'),
 output_tokens TEXT NOT NULL CHECK(length(output_tokens) BETWEEN 1 AND 19 AND output_tokens NOT GLOB '*[^0-9]*'),
 cache_write_tokens TEXT NOT NULL CHECK(length(cache_write_tokens) BETWEEN 1 AND 19 AND cache_write_tokens NOT GLOB '*[^0-9]*'),
 cache_read_tokens TEXT NOT NULL CHECK(length(cache_read_tokens) BETWEEN 1 AND 19 AND cache_read_tokens NOT GLOB '*[^0-9]*'),
 -- A part of the output count. Null when the tool reports none: a missing count is not zero.
 reasoning_tokens TEXT CHECK(reasoning_tokens IS NULL OR (length(reasoning_tokens) BETWEEN 1 AND 19 AND reasoning_tokens NOT GLOB '*[^0-9]*')),
 total_tokens TEXT NOT NULL CHECK(length(total_tokens) BETWEEN 1 AND 19 AND total_tokens NOT GLOB '*[^0-9]*'),
 -- An estimate in millionths of a US dollar. It is not money (OU-LOG-014).
 estimated_value_usd_micros TEXT CHECK(estimated_value_usd_micros IS NULL OR (length(estimated_value_usd_micros) BETWEEN 1 AND 19 AND estimated_value_usd_micros NOT GLOB '*[^0-9]*')),
 scanner_version TEXT NOT NULL CHECK(length(scanner_version) BETWEEN 1 AND 32),
 created_at_utc TEXT NOT NULL,
 updated_at_utc TEXT NOT NULL,
 FOREIGN KEY(log_source_id,tool) REFERENCES log_sources(id,tool),
 UNIQUE(log_source_id,session_key)
) STRICT;
CREATE INDEX local_usage_sessions_by_end ON local_usage_sessions(end_utc);
