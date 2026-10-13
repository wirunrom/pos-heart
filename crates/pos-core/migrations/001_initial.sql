CREATE TABLE products (
    id TEXT PRIMARY KEY NOT NULL,
    sku TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(trim(sku)) BETWEEN 1 AND 64),
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 160),
    category TEXT NOT NULL CHECK(length(category) <= 80),
    price_satang INTEGER NOT NULL CHECK(price_satang >= 0),
    stock INTEGER CHECK(stock IS NULL OR stock BETWEEN 0 AND 4294967295),
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1))
) STRICT;
CREATE TABLE sales (
    number INTEGER PRIMARY KEY AUTOINCREMENT,
    cart_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    label TEXT NOT NULL,
    total_satang INTEGER NOT NULL CHECK(total_satang >= 0),
    cash_satang INTEGER NOT NULL CHECK(cash_satang >= total_satang),
    change_satang INTEGER NOT NULL CHECK(change_satang = cash_satang - total_satang),
    receipt_json TEXT NOT NULL
) STRICT;
CREATE INDEX sales_created_at ON sales(created_at);
CREATE TABLE held_carts (
    id TEXT PRIMARY KEY NOT NULL,
    updated_at INTEGER NOT NULL,
    cart_json TEXT NOT NULL
) STRICT;
PRAGMA user_version = 1;
