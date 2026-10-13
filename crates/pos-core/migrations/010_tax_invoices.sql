-- Full tax invoices (ใบกำกับภาษีเต็มรูป): own running number, one per sale.
-- Abbreviated tax invoices are the receipts themselves (sales.receipt_json).
CREATE TABLE tax_invoices (
    number INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    sale_id TEXT NOT NULL UNIQUE REFERENCES sales(cart_id),
    created_at INTEGER NOT NULL,
    invoice_json TEXT NOT NULL
) STRICT;
PRAGMA user_version=10;
