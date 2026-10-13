-- Partial refunds: some units of some lines, possibly several times per sale.
-- Full voids stay in sale_voids; receipts are never rewritten.
CREATE TABLE sale_refunds (
    id TEXT PRIMARY KEY NOT NULL,
    sale_id TEXT NOT NULL REFERENCES sales(cart_id),
    created_at INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK(length(trim(reason)) BETWEEN 1 AND 240),
    refund_satang INTEGER NOT NULL CHECK(refund_satang >= 0),
    refund_method TEXT NOT NULL CHECK(refund_method IN ('cash', 'transfer', 'card')),
    refund_json TEXT NOT NULL
) STRICT;
CREATE INDEX sale_refunds_sale ON sale_refunds(sale_id, created_at);
CREATE INDEX sale_refunds_created_at ON sale_refunds(created_at);
PRAGMA user_version=8;
