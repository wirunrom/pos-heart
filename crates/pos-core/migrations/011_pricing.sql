-- Service charge applies per product (off for retail goods in a restaurant).
-- Pricing settings (service charge rate, cash rounding) live in app_settings.
ALTER TABLE products ADD COLUMN service_charge INTEGER NOT NULL DEFAULT 1 CHECK(service_charge IN (0, 1));
PRAGMA user_version=11;
