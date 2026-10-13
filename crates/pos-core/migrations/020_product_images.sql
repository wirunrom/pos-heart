-- Product pictures as small PNGs, apart from the product rows so catalog
-- queries never load them. `image_rev` changes whenever the picture does.
CREATE TABLE product_images (
    product_id TEXT PRIMARY KEY NOT NULL REFERENCES products(id),
    png BLOB NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
ALTER TABLE products ADD COLUMN image_rev INTEGER;
PRAGMA user_version=20;
