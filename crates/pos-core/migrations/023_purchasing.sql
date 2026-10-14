CREATE TABLE suppliers (
 id TEXT PRIMARY KEY NOT NULL,
 name TEXT NOT NULL,
 active INTEGER NOT NULL CHECK(active IN (0,1)),
 supplier_json TEXT NOT NULL CHECK(json_valid(supplier_json)),
 updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE purchase_orders (
 id TEXT PRIMARY KEY NOT NULL,
 supplier_id TEXT NOT NULL REFERENCES suppliers(id),
 created_at INTEGER NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>=0),
 order_json TEXT NOT NULL CHECK(json_valid(order_json)),
 request_json TEXT NOT NULL CHECK(json_valid(request_json))
) STRICT;
CREATE INDEX purchase_orders_created ON purchase_orders(created_at DESC,id);
CREATE TABLE purchase_receipts (
 id TEXT PRIMARY KEY NOT NULL,
 order_id TEXT NOT NULL REFERENCES purchase_orders(id),
 created_at INTEGER NOT NULL,
 receipt_json TEXT NOT NULL CHECK(json_valid(receipt_json))
) STRICT;
CREATE INDEX purchase_receipts_order ON purchase_receipts(order_id,created_at DESC,id);
CREATE TABLE purchase_cancellations (
 id TEXT PRIMARY KEY NOT NULL,
 order_id TEXT NOT NULL UNIQUE REFERENCES purchase_orders(id),
 cancellation_json TEXT NOT NULL CHECK(json_valid(cancellation_json))
) STRICT;
PRAGMA user_version=23;
