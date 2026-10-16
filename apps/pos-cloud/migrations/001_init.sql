-- POS Heart cloud, schema 1. Applied at startup inside one transaction;
-- `cloud_meta.version` records it.
CREATE TABLE cloud_meta (version INTEGER NOT NULL);
INSERT INTO cloud_meta VALUES (1);

-- Shop owners (dashboard sign-in). Passwords are Argon2id hashes.
CREATE TABLE accounts (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
-- Dashboard sessions: only the SHA-256 of the bearer token is stored.
CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    expires_at BIGINT NOT NULL
);
CREATE TABLE shops (
    id UUID PRIMARY KEY,
    owner_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
-- One-time codes the owner creates to link the shop's main register.
CREATE TABLE link_codes (
    code TEXT PRIMARY KEY,
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    expires_at BIGINT NOT NULL
);
-- The shop's main register (hub). Linking a new one revokes the old.
CREATE TABLE hubs (
    id UUID PRIMARY KEY,
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    linked_at BIGINT NOT NULL,
    last_seen_at BIGINT,
    revoked_at BIGINT
);
-- The hub's records, one row per (kind, id): resends and restores
-- overwrite, a NULL `data` is a deleted record. `occurred_at` (unix seconds)
-- is copied from the record's `created_at` for period queries.
CREATE TABLE records (
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    data JSONB,
    occurred_at BIGINT,
    received_at BIGINT NOT NULL,
    PRIMARY KEY (shop_id, kind, record_id)
);
CREATE INDEX records_period ON records (shop_id, kind, occurred_at);
-- Owner edits waiting to be applied by the hub, in order.
CREATE TABLE changes (
    seq BIGSERIAL PRIMARY KEY,
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    patch JSONB NOT NULL,
    created_by UUID NOT NULL REFERENCES accounts(id),
    created_at BIGINT NOT NULL,
    applied_at BIGINT,
    outcome TEXT
);
CREATE INDEX changes_pending ON changes (shop_id, seq) WHERE applied_at IS NULL;
