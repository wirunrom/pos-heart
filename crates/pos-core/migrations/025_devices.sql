-- Paired devices (multi-device phase 1b). Tokens are stored as SHA-256
-- hashes; a revoked device keeps its row for the audit trail.
CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 80),
    kind TEXT NOT NULL CHECK(kind IN ('register', 'kitchen', 'ordering')),
    register_id TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    paired_at INTEGER NOT NULL,
    revoked_at INTEGER,
    last_seen_at INTEGER
) STRICT;
PRAGMA user_version=25;
