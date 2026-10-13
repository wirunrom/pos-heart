CREATE TABLE shifts (
    id TEXT PRIMARY KEY NOT NULL,
    opened_at INTEGER NOT NULL,
    closed_at INTEGER,
    shift_json TEXT NOT NULL
) STRICT;
CREATE UNIQUE INDEX one_open_shift ON shifts ((1)) WHERE closed_at IS NULL;
CREATE INDEX shifts_closed ON shifts(closed_at DESC);
CREATE TABLE cash_movements (
    id TEXT PRIMARY KEY NOT NULL,
    shift_id TEXT NOT NULL REFERENCES shifts(id),
    created_at INTEGER NOT NULL,
    movement_json TEXT NOT NULL
) STRICT;
CREATE INDEX movements_shift ON cash_movements(shift_id, created_at DESC);
PRAGMA user_version=3;
