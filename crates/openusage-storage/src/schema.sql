CREATE TABLE accounts (
    id TEXT PRIMARY KEY NOT NULL,
    verified_identity_key TEXT,
    identity_confidence TEXT NOT NULL CHECK(identity_confidence IN ('verified','provisional','unknown')),
    returned_email TEXT,
    returned_plan TEXT,
    first_observed_at TEXT NOT NULL,
    last_observed_at TEXT NOT NULL,
    CHECK(verified_identity_key IS NULL OR identity_confidence = 'verified'),
    CHECK(identity_confidence != 'verified' OR verified_identity_key IS NOT NULL)
) STRICT;

CREATE TABLE profiles (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36 AND id GLOB '????????-????-????-????-????????????' AND replace(id,'-','') NOT GLOB '*[^0-9a-fA-F]*'),
    account_id TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
    environment TEXT NOT NULL CHECK(environment IN ('native','wsl')),
    approved_home_reference TEXT NOT NULL UNIQUE,
    connection_state TEXT NOT NULL CHECK(connection_state IN ('connected','authentication_required','unauthorized','offline','unsupported','error')),
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TRIGGER active_profiles_limit BEFORE INSERT ON profiles
WHEN NEW.archived = 0 AND (SELECT count(*) FROM profiles WHERE archived = 0) >= 10
BEGIN SELECT RAISE(ABORT, 'active profile limit'); END;

CREATE TRIGGER unarchive_profiles_limit BEFORE UPDATE OF archived ON profiles
WHEN OLD.archived != 0 AND NEW.archived = 0 AND (SELECT count(*) FROM profiles WHERE archived = 0) >= 10
BEGIN SELECT RAISE(ABORT, 'active profile limit'); END;

CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    timezone TEXT NOT NULL,
    locale TEXT NOT NULL,
    theme TEXT NOT NULL CHECK(theme IN ('system','light','dark')),
    monitoring TEXT NOT NULL CHECK(monitoring IN ('stopped','paused','active')),
    retention_policy TEXT NOT NULL CHECK(retention_policy = 'unlimited'),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

INSERT INTO settings VALUES (
    1, 'UTC', 'en-US', 'system', 'stopped', 'unlimited',
    strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now')
);
