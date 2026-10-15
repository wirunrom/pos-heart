-- Registers (multi-device phase 0c): each register has its own shift and
-- drawer. Existing shifts belong to the main register.
CREATE TABLE registers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 80),
    created_at INTEGER NOT NULL,
    retired_at INTEGER
) STRICT;
INSERT INTO registers (id, name, created_at) VALUES
    ('00000000-0000-0000-0000-000000000001', 'เครื่องหลัก', 0);
-- Checked in code (Pos::use_register); no REFERENCES so older-schema
-- fixtures can still drop the column.
ALTER TABLE shifts ADD COLUMN register_id TEXT NOT NULL
    DEFAULT '00000000-0000-0000-0000-000000000001';
DROP INDEX one_open_shift;
CREATE UNIQUE INDEX one_open_shift_per_register ON shifts(register_id) WHERE closed_at IS NULL;
PRAGMA user_version=24;
