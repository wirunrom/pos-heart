-- Schema 2: encrypted database backups from the hub. The server cannot
-- read them (sealed on the main register with the owner's password).
CREATE TABLE backups (
    id UUID PRIMARY KEY,
    shop_id UUID NOT NULL REFERENCES shops(id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    size BIGINT NOT NULL,
    data BYTEA NOT NULL
);
CREATE INDEX backups_shop ON backups (shop_id, created_at DESC);
UPDATE cloud_meta SET version = 2;
