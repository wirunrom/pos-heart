-- Credit notes (ใบลดหนี้, Revenue Code s.86/10): one per refund or void of a
-- sale that has a full tax invoice, in their own running number sequence.
-- `source_id` is the void or partial refund the note documents.
CREATE TABLE credit_notes (
    number INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    sale_id TEXT NOT NULL REFERENCES sales(cart_id),
    source_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    note_json TEXT NOT NULL
) STRICT;
CREATE INDEX credit_notes_sale ON credit_notes(sale_id, number);
PRAGMA user_version=16;
