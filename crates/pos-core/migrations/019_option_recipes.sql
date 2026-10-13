-- Recipe changes by option choice ("+ egg", "no egg"), per product.
ALTER TABLE products ADD COLUMN option_recipe_json TEXT NOT NULL DEFAULT '[]'
    CHECK(json_valid(option_recipe_json) AND json_type(option_recipe_json) = 'array');
PRAGMA user_version=19;
