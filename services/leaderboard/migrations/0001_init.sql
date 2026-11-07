-- The opt-in leaderboard (SRS Section 14A, decision of 7 November 2026).
-- It stores no IP address, no GitHub token, and no free text from a client.
CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  github_id INTEGER NOT NULL UNIQUE,
  login TEXT NOT NULL CHECK (length(login) BETWEEN 1 AND 39),
  created_at TEXT NOT NULL,
  -- A maintainer sets this with the D1 command line. No web endpoint changes it.
  hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1))
) STRICT;
CREATE TABLE devices (
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL CHECK (length(device_id) = 36),
  -- SHA-256 of the 256-bit service token. The token itself is never stored.
  token_hash TEXT NOT NULL UNIQUE CHECK (length(token_hash) = 64),
  created_at TEXT NOT NULL,
  PRIMARY KEY (user_id, device_id)
) STRICT;
CREATE TABLE days (
  user_id INTEGER NOT NULL,
  device_id TEXT NOT NULL,
  date TEXT NOT NULL CHECK (length(date) = 10),
  tool TEXT NOT NULL CHECK (length(tool) BETWEEN 1 AND 32),
  model TEXT NOT NULL CHECK (length(model) BETWEEN 1 AND 64),
  input_tokens INTEGER NOT NULL CHECK (input_tokens BETWEEN 0 AND 1000000000000),
  output_tokens INTEGER NOT NULL CHECK (output_tokens BETWEEN 0 AND 1000000000000),
  cache_read_tokens INTEGER NOT NULL CHECK (cache_read_tokens BETWEEN 0 AND 1000000000000),
  cache_write_tokens INTEGER NOT NULL CHECK (cache_write_tokens BETWEEN 0 AND 1000000000000),
  total_tokens INTEGER NOT NULL CHECK (total_tokens = input_tokens + output_tokens + cache_read_tokens + cache_write_tokens),
  estimated_value_micros INTEGER CHECK (estimated_value_micros IS NULL OR estimated_value_micros BETWEEN 0 AND 1000000000000),
  PRIMARY KEY (user_id, device_id, date, tool, model),
  FOREIGN KEY (user_id, device_id) REFERENCES devices(user_id, device_id) ON DELETE CASCADE
) STRICT;
CREATE INDEX days_by_date ON days(date);
-- A user day above the plausibility ceiling stays out of the ranks until a maintainer clears it.
CREATE TABLE flags (
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  date TEXT NOT NULL,
  cleared INTEGER NOT NULL DEFAULT 0 CHECK (cleared IN (0, 1)),
  PRIMARY KEY (user_id, date)
) STRICT;
-- Hourly counts keyed by GitHub account or user, never by IP address.
CREATE TABLE limits (
  key TEXT NOT NULL,
  hour TEXT NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (key, hour)
) STRICT;
