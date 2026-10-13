CREATE TABLE sale_voids (
    sale_id TEXT PRIMARY KEY NOT NULL REFERENCES sales(cart_id),
    id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK(length(trim(reason)) BETWEEN 1 AND 240),
    refund_satang INTEGER NOT NULL CHECK(refund_satang >= 0),
    void_json TEXT NOT NULL
) STRICT;
CREATE INDEX sale_voids_created_at ON sale_voids(created_at);
PRAGMA user_version = 2;
