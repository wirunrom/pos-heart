-- Categories as their own rows: order (as shown in Sell and Products) and
-- empty categories. Products keep the category name; renames update both.
CREATE TABLE categories (
    name TEXT PRIMARY KEY NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 80),
    position INTEGER NOT NULL
) STRICT;
INSERT INTO categories(name, position)
    SELECT category, ROW_NUMBER() OVER (ORDER BY category)
    FROM (SELECT DISTINCT category FROM products WHERE category <> '');
PRAGMA user_version=21;
