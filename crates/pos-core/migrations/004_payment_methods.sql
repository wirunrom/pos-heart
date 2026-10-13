-- Rebuild both sides of the receipt/refund foreign key with foreign_keys still ON.
-- Preserve receipt numbers, sequence high-water mark, and immutable JSON verbatim.
CREATE TABLE sales_v4 (
    number INTEGER PRIMARY KEY AUTOINCREMENT,
    cart_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    label TEXT NOT NULL,
    total_satang INTEGER NOT NULL CHECK(total_satang >= 0),
    cash_satang INTEGER NOT NULL CHECK(cash_satang >= 0),
    change_satang INTEGER NOT NULL CHECK(change_satang >= 0),
    receipt_json TEXT NOT NULL,
    payment_method TEXT NOT NULL DEFAULT 'cash' CHECK(payment_method IN ('cash','transfer','card')),
    CHECK((payment_method = 'cash' AND cash_satang >= total_satang AND change_satang = cash_satang - total_satang)
       OR (payment_method IN ('transfer','card') AND cash_satang = 0 AND change_satang = 0))
) STRICT;
INSERT INTO sales_v4(number,cart_id,created_at,label,total_satang,cash_satang,change_satang,receipt_json)
    SELECT number,cart_id,created_at,label,total_satang,cash_satang,change_satang,receipt_json FROM sales;
INSERT INTO sqlite_sequence(name,seq) SELECT 'sales_v4',seq FROM sqlite_sequence WHERE name='sales'
    AND NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='sales_v4');
UPDATE sqlite_sequence SET seq=max(seq,coalesce((SELECT seq FROM sqlite_sequence WHERE name='sales'),0)) WHERE name='sales_v4';
CREATE TABLE sale_voids_v4 (
    sale_id TEXT PRIMARY KEY NOT NULL REFERENCES sales_v4(cart_id),
    id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK(length(trim(reason)) BETWEEN 1 AND 240),
    refund_satang INTEGER NOT NULL CHECK(refund_satang >= 0),
    void_json TEXT NOT NULL,
    refund_method TEXT NOT NULL DEFAULT 'cash' CHECK(refund_method IN ('cash','transfer','card'))
) STRICT;
INSERT INTO sale_voids_v4(sale_id,id,created_at,reason,refund_satang,void_json)
    SELECT sale_id,id,created_at,reason,refund_satang,void_json FROM sale_voids;
DROP TABLE sale_voids;
DROP TABLE sales;
ALTER TABLE sales_v4 RENAME TO sales;
ALTER TABLE sale_voids_v4 RENAME TO sale_voids;
CREATE INDEX sales_created_at ON sales(created_at);
CREATE INDEX sale_voids_created_at ON sale_voids(created_at);
ALTER TABLE cash_movements ADD COLUMN payment_method TEXT NOT NULL DEFAULT 'cash'
    CHECK(payment_method IN ('cash','transfer','card'));
PRAGMA user_version=4;
