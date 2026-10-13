-- Scanner barcodes separate from the shop's product code, and a reversible
-- "not for sale" flag distinct from archiving. Existing rows stay on sale.
ALTER TABLE products ADD COLUMN barcode TEXT CHECK(barcode IS NULL OR length(trim(barcode)) BETWEEN 1 AND 64);
ALTER TABLE products ADD COLUMN on_sale INTEGER NOT NULL DEFAULT 1 CHECK(on_sale IN (0, 1));
CREATE UNIQUE INDEX products_barcode ON products(barcode) WHERE barcode IS NOT NULL;
CREATE INDEX products_category ON products(category, name);
PRAGMA user_version=5;
