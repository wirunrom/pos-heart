-- Option groups (sweetness, toppings) per product as validated JSON.
-- Existing products have none; receipts keep their own option snapshots.
ALTER TABLE products ADD COLUMN options_json TEXT NOT NULL DEFAULT '[]'
    CHECK(json_valid(options_json) AND json_type(options_json) = 'array');
PRAGMA user_version=6;
