-- Cloud outbox (phase 2): one pointer row per record the cloud has not
-- acknowledged yet, written by triggers in the same transaction as the
-- business write. The uploader reads each record's current JSON from its own
-- table (records are immutable or sent as their latest state) and deletes
-- the rows the server acknowledged. Mutable records keep one pending row.
CREATE TABLE outbox (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    queued_at INTEGER NOT NULL
);
CREATE INDEX outbox_record ON outbox(kind, record_id);

CREATE TRIGGER outbox_sales_insert AFTER INSERT ON sales BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('sale', CAST(NEW.cart_id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_sale_voids_insert AFTER INSERT ON sale_voids BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('void', CAST(NEW.sale_id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_sale_refunds_insert AFTER INSERT ON sale_refunds BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('refund', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_cash_movements_insert AFTER INSERT ON cash_movements BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('cash_movement', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_stock_movements_insert AFTER INSERT ON stock_movements BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('stock_movement', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_ingredient_movements_insert AFTER INSERT ON ingredient_movements BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('ingredient_movement', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_tax_invoices_insert AFTER INSERT ON tax_invoices BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('tax_invoice', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_credit_notes_insert AFTER INSERT ON credit_notes BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('credit_note', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_activity_events_insert AFTER INSERT ON activity_events BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('activity', CAST(NEW.number AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_purchase_receipts_insert AFTER INSERT ON purchase_receipts BEGIN
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('purchase_receipt', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_categories_insert AFTER INSERT ON categories BEGIN
    DELETE FROM outbox WHERE kind = 'category' AND record_id = CAST(NEW.name AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('category', CAST(NEW.name AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_categories_update AFTER UPDATE ON categories BEGIN
    DELETE FROM outbox WHERE kind = 'category' AND record_id = CAST(NEW.name AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('category', CAST(NEW.name AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_categories_delete AFTER DELETE ON categories BEGIN
    DELETE FROM outbox WHERE kind = 'category' AND record_id = CAST(OLD.name AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('category', CAST(OLD.name AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_categories_rename AFTER UPDATE OF name ON categories WHEN OLD.name <> NEW.name BEGIN
    DELETE FROM outbox WHERE kind = 'category' AND record_id = OLD.name;
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('category', OLD.name, CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_products_insert AFTER INSERT ON products BEGIN
    DELETE FROM outbox WHERE kind = 'product' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('product', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_products_update AFTER UPDATE ON products BEGIN
    DELETE FROM outbox WHERE kind = 'product' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('product', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_products_delete AFTER DELETE ON products BEGIN
    DELETE FROM outbox WHERE kind = 'product' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('product', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_option_sets_insert AFTER INSERT ON option_sets BEGIN
    DELETE FROM outbox WHERE kind = 'option_set' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('option_set', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_option_sets_update AFTER UPDATE ON option_sets BEGIN
    DELETE FROM outbox WHERE kind = 'option_set' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('option_set', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_option_sets_delete AFTER DELETE ON option_sets BEGIN
    DELETE FROM outbox WHERE kind = 'option_set' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('option_set', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_promotions_insert AFTER INSERT ON promotions BEGIN
    DELETE FROM outbox WHERE kind = 'promotion' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('promotion', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_promotions_update AFTER UPDATE ON promotions BEGIN
    DELETE FROM outbox WHERE kind = 'promotion' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('promotion', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_promotions_delete AFTER DELETE ON promotions BEGIN
    DELETE FROM outbox WHERE kind = 'promotion' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('promotion', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_ingredients_insert AFTER INSERT ON ingredients BEGIN
    DELETE FROM outbox WHERE kind = 'ingredient' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('ingredient', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_ingredients_update AFTER UPDATE ON ingredients BEGIN
    DELETE FROM outbox WHERE kind = 'ingredient' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('ingredient', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_ingredients_delete AFTER DELETE ON ingredients BEGIN
    DELETE FROM outbox WHERE kind = 'ingredient' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('ingredient', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_dining_tables_insert AFTER INSERT ON dining_tables BEGIN
    DELETE FROM outbox WHERE kind = 'table' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('table', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_dining_tables_update AFTER UPDATE ON dining_tables BEGIN
    DELETE FROM outbox WHERE kind = 'table' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('table', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_dining_tables_delete AFTER DELETE ON dining_tables BEGIN
    DELETE FROM outbox WHERE kind = 'table' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('table', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_staff_insert AFTER INSERT ON staff BEGIN
    DELETE FROM outbox WHERE kind = 'staff' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('staff', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_staff_update AFTER UPDATE ON staff BEGIN
    DELETE FROM outbox WHERE kind = 'staff' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('staff', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_staff_delete AFTER DELETE ON staff BEGIN
    DELETE FROM outbox WHERE kind = 'staff' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('staff', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_suppliers_insert AFTER INSERT ON suppliers BEGIN
    DELETE FROM outbox WHERE kind = 'supplier' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('supplier', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_suppliers_update AFTER UPDATE ON suppliers BEGIN
    DELETE FROM outbox WHERE kind = 'supplier' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('supplier', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_suppliers_delete AFTER DELETE ON suppliers BEGIN
    DELETE FROM outbox WHERE kind = 'supplier' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('supplier', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_purchase_orders_insert AFTER INSERT ON purchase_orders BEGIN
    DELETE FROM outbox WHERE kind = 'purchase_order' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('purchase_order', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_purchase_orders_update AFTER UPDATE ON purchase_orders BEGIN
    DELETE FROM outbox WHERE kind = 'purchase_order' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('purchase_order', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_purchase_orders_delete AFTER DELETE ON purchase_orders BEGIN
    DELETE FROM outbox WHERE kind = 'purchase_order' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('purchase_order', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_shifts_insert AFTER INSERT ON shifts BEGIN
    DELETE FROM outbox WHERE kind = 'shift' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('shift', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_shifts_update AFTER UPDATE ON shifts BEGIN
    DELETE FROM outbox WHERE kind = 'shift' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('shift', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_shifts_delete AFTER DELETE ON shifts BEGIN
    DELETE FROM outbox WHERE kind = 'shift' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('shift', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_registers_insert AFTER INSERT ON registers BEGIN
    DELETE FROM outbox WHERE kind = 'register' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('register', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_registers_update AFTER UPDATE ON registers BEGIN
    DELETE FROM outbox WHERE kind = 'register' AND record_id = CAST(NEW.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('register', CAST(NEW.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
CREATE TRIGGER outbox_registers_delete AFTER DELETE ON registers BEGIN
    DELETE FROM outbox WHERE kind = 'register' AND record_id = CAST(OLD.id AS TEXT);
    INSERT INTO outbox(kind, record_id, queued_at) VALUES ('register', CAST(OLD.id AS TEXT), CAST(strftime('%s','now') AS INTEGER));
END;
PRAGMA user_version=26;
