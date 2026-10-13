-- Shop-defined roles. Built-in "manager" and "cashier" rows are created by
-- the app on first use (the cashier keeps the old shop-wide discount limit);
-- "owner" is fixed in code and never stored.
CREATE TABLE roles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 40),
    role_json TEXT NOT NULL
) STRICT;
-- staff.role now holds any role ID, so rebuild the table without the old CHECK.
CREATE TABLE staff_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 60),
    role TEXT NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
    pin_hash TEXT NOT NULL,
    active INTEGER NOT NULL CHECK(active IN (0, 1)),
    created_at INTEGER NOT NULL
) STRICT;
INSERT INTO staff_new SELECT id, name, role, pin_hash, active, created_at FROM staff;
DROP TABLE staff;
ALTER TABLE staff_new RENAME TO staff;
PRAGMA user_version=12;
