-- Shared option sets (one option group each, e.g. sweetness) that many
-- products use; products list the sets they use beside their own groups.
CREATE TABLE option_sets (
    id TEXT PRIMARY KEY NOT NULL,
    set_json TEXT NOT NULL,
    position INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
ALTER TABLE products ADD COLUMN option_sets_json TEXT NOT NULL DEFAULT '[]'
    CHECK(json_valid(option_sets_json) AND json_type(option_sets_json) = 'array');
PRAGMA user_version=22;
