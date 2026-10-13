ALTER TABLE products ADD COLUMN cost_satang INTEGER CHECK(cost_satang IS NULL OR cost_satang >= 0);
PRAGMA user_version = 13;
