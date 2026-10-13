-- Ingredients with stock in thousandths of their unit (may go negative),
-- their movement history (manual changes, sale use, void returns), and a
-- recipe per product. Existing products have no recipe.
CREATE TABLE ingredients (
    id TEXT PRIMARY KEY NOT NULL,
    ingredient_json TEXT NOT NULL,
    stock_milli INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE ingredient_movements (
    id TEXT PRIMARY KEY NOT NULL,
    ingredient_id TEXT NOT NULL REFERENCES ingredients(id),
    created_at INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('initial', 'receive', 'remove', 'count', 'sale', 'void')),
    sale_id TEXT,
    movement_json TEXT NOT NULL
) STRICT;
CREATE INDEX ingredient_movements_ingredient ON ingredient_movements(ingredient_id, created_at DESC);
CREATE INDEX ingredient_movements_sale ON ingredient_movements(sale_id);
ALTER TABLE products ADD COLUMN recipe_json TEXT NOT NULL DEFAULT '[]'
    CHECK(json_valid(recipe_json) AND json_type(recipe_json) = 'array');
PRAGMA user_version=18;
