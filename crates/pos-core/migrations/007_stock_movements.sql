-- Manual stock history: receiving, removals, counts and the start of tracking.
-- Sales and refunds stay recorded on receipts; existing data is unchanged.
CREATE TABLE stock_movements (
    id TEXT PRIMARY KEY NOT NULL,
    product_id TEXT NOT NULL REFERENCES products(id),
    created_at INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('initial', 'receive', 'remove', 'count')),
    before_qty INTEGER NOT NULL CHECK(before_qty BETWEEN 0 AND 4294967295),
    after_qty INTEGER NOT NULL CHECK(after_qty BETWEEN 0 AND 4294967295),
    reason TEXT NOT NULL CHECK(length(reason) <= 240)
) STRICT;
CREATE INDEX stock_movements_product ON stock_movements(product_id, created_at DESC);
PRAGMA user_version=7;
