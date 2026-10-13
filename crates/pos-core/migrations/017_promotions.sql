-- Shop-defined promotions (JSON) and how many bills used each; checkout
-- recomputes promotions from this table inside its transaction.
CREATE TABLE promotions (
    id TEXT PRIMARY KEY NOT NULL,
    promo_json TEXT NOT NULL,
    uses INTEGER NOT NULL DEFAULT 0 CHECK(uses >= 0),
    updated_at INTEGER NOT NULL
) STRICT;
PRAGMA user_version=17;
