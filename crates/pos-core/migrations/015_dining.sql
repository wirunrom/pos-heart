-- Tables and zones for sit-down service, and kitchen tickets.
-- A table's open order is a held bill whose cart_json names the table.
CREATE TABLE dining_tables (
    id TEXT PRIMARY KEY NOT NULL,
    zone TEXT NOT NULL CHECK(length(zone) <= 160),
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    seats INTEGER NOT NULL CHECK(seats BETWEEN 0 AND 99),
    active INTEGER NOT NULL CHECK(active IN (0, 1)),
    sort INTEGER NOT NULL
) STRICT;
CREATE TABLE kitchen_tickets (
    number INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    cart_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    bumped_at INTEGER,
    ticket_json TEXT NOT NULL
) STRICT;
CREATE INDEX kitchen_tickets_open ON kitchen_tickets(bumped_at, number);
-- Which products the kitchen prepares (bottled drinks and retail do not).
ALTER TABLE products ADD COLUMN kitchen INTEGER NOT NULL DEFAULT 1 CHECK(kitchen IN (0, 1));
PRAGMA user_version=15;
