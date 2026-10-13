-- Staff who sign in with a PIN. A shop without active staff needs no sign-in.
-- Records keep JSON snapshots of who acted; this table is only the roster.
CREATE TABLE staff (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 60),
    role TEXT NOT NULL CHECK(role IN ('owner', 'manager', 'cashier')),
    pin_hash TEXT NOT NULL,
    active INTEGER NOT NULL CHECK(active IN (0, 1)),
    created_at INTEGER NOT NULL
) STRICT;
-- Small shop-wide settings enforced by the core (e.g. the cashier discount limit).
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;
-- Who made a manual stock change (JSON attribution, NULL before staff).
ALTER TABLE stock_movements ADD COLUMN staff_json TEXT;
PRAGMA user_version=9;
