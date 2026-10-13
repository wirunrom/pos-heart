//! Read-only structural validation before a snapshot is offered for recovery.
use crate::{Error, Result};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Text},
};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupInfo {
    pub schema: i64,
    pub sales: i64,
    pub products: i64,
    pub held: i64,
}
#[derive(QueryableByName)]
struct Check {
    #[diesel(sql_type = Text)]
    integrity_check: String,
}
#[derive(QueryableByName)]
struct Number {
    #[diesel(sql_type = BigInt)]
    n: i64,
}
pub fn inspect_database(path: &Path) -> Result<BackupInfo> {
    if !std::fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(Error::CorruptData);
    }
    let path = path.to_str().ok_or(Error::CorruptData)?;
    let mut conn = SqliteConnection::establish(path)?;
    diesel::sql_query("PRAGMA query_only=ON").execute(&mut conn)?;
    let checks = diesel::sql_query("PRAGMA integrity_check").load::<Check>(&mut conn)?;
    if checks.len() != 1 || checks[0].integrity_check != "ok" {
        return Err(Error::CorruptData);
    }
    let schema = diesel::sql_query("SELECT user_version AS n FROM pragma_user_version")
        .get_result::<Number>(&mut conn)?
        .n;
    if !(1..=i64::from(crate::SCHEMA_VERSION)).contains(&schema) {
        return Err(Error::UnsupportedSchema(schema));
    }
    let foreign = diesel::sql_query("SELECT count(*) AS n FROM pragma_foreign_key_check")
        .get_result::<Number>(&mut conn)?
        .n;
    if foreign != 0 {
        return Err(Error::CorruptData);
    }
    let mut count = |sql: &str| -> Result<i64> {
        Ok(diesel::sql_query(sql).get_result::<Number>(&mut conn)?.n)
    };
    Ok(BackupInfo {
        schema,
        sales: count("SELECT count(*) AS n FROM sales")?,
        products: count("SELECT count(*) AS n FROM products")?,
        held: count("SELECT count(*) AS n FROM held_carts")?,
    })
}

impl crate::Pos {
    /// A sidecar marker must never resurrect a paid/deleted bill in a snapshot.
    pub fn has_held_cart(&mut self, id: uuid::Uuid) -> Result<bool> {
        use crate::schema::held_carts;
        Ok(diesel::select(diesel::dsl::exists(
            held_carts::table.filter(held_carts::id.eq(id.to_string())),
        ))
        .get_result(&mut self.conn)?)
    }
    /// Device operations recheck Settings against the current database session.
    pub fn authorize_recovery(&mut self) -> Result<()> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Settings))?;
            Ok(())
        })
    }
}

#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type = Text)]
    value: String,
}
fn validate_json<T: serde::de::DeserializeOwned>(
    conn: &mut SqliteConnection,
    table: &str,
    field: &str,
    validate: impl Fn(T) -> Result<()>,
) -> Result<()> {
    let mut offset = 0_i64;
    loop {
        // Table and field are fixed source-code identifiers, never external input.
        let rows = diesel::sql_query(format!(
            "SELECT {field} AS value FROM {table} ORDER BY rowid LIMIT 200 OFFSET ?"
        ))
        .bind::<BigInt, _>(offset)
        .load::<JsonRow>(conn)?;
        let count = rows.len();
        for row in rows {
            validate(serde_json::from_str(&row.value)?)?;
        }
        if count < 200 {
            return Ok(());
        }
        offset = offset.checked_add(200).ok_or(Error::CorruptData)?;
    }
}
impl crate::Pos {
    /// Validate every row of a staged/migrated snapshot, not only the first UI page.
    pub fn validate_recovery_data(&mut self) -> Result<()> {
        use crate::schema::products;
        use pos_domain::*;
        let mut offset = 0_i64;
        loop {
            let rows = products::table
                .order(products::id)
                .offset(offset)
                .limit(200)
                .select(crate::catalog::ProductRow::as_select())
                .load::<crate::catalog::ProductRow>(&mut self.conn)?;
            let count = rows.len();
            for row in rows {
                let p: Product = row.try_into()?;
                ProductInput {
                    id: Some(p.id),
                    sku: p.sku,
                    name: p.name,
                    category: p.category,
                    price: p.price,
                    cost: p.cost,
                    stock: p.stock,
                    barcode: p.barcode,
                    on_sale: p.on_sale,
                    options: p.options,
                    service_charge: p.service_charge,
                    kitchen: p.kitchen,
                    recipe: p.recipe,
                    keep_tracked_stock: true,
                }
                .validate()?;
            }
            if count < 200 {
                break;
            }
            offset = offset.checked_add(200).ok_or(Error::CorruptData)?;
        }
        validate_json::<Cart>(&mut self.conn, "held_carts", "cart_json", |c| {
            c.validate()?;
            Ok(())
        })?;
        validate_json::<Sale>(&mut self.conn, "sales", "receipt_json", |sale| {
            if sale.lines.is_empty() {
                return Err(Error::CorruptData);
            }
            let mut gross = Money::ZERO;
            let mut paid = Money::ZERO;
            for line in &sale.lines {
                if line.quantity == 0 || line.unit_price.checked_mul(line.quantity)? != line.total {
                    return Err(Error::CorruptData);
                }
                gross = gross.checked_add(line.total)?;
                let value = i128::from(line.total.satang())
                    - i128::from(line.discount.satang())
                    - i128::from(line.bill_discount.satang())
                    + i128::from(line.service.satang())
                    + i128::from(line.vat_added.satang())
                    + i128::from(line.rounding);
                let value =
                    Money::from_satang(i64::try_from(value).map_err(|_| Error::CorruptData)?)?;
                if line.vat_share > value {
                    return Err(Error::CorruptData);
                }
                paid = paid.checked_add(value)?;
            }
            if paid != sale.total {
                return Err(Error::CorruptData);
            }
            Ok(())
        })?;
        validate_json::<SaleVoid>(&mut self.conn, "sale_voids", "void_json", |_| Ok(()))?;
        validate_json::<SaleRefund>(&mut self.conn, "sale_refunds", "refund_json", |_| Ok(()))?;
        validate_json::<Shift>(&mut self.conn, "shifts", "shift_json", |s| {
            s.expected()?;
            Ok(())
        })?;
        validate_json::<CashMovement>(&mut self.conn, "cash_movements", "movement_json", |_| {
            Ok(())
        })?;
        validate_json::<TaxInvoice>(&mut self.conn, "tax_invoices", "invoice_json", |_| Ok(()))?;
        validate_json::<pos_domain::CreditNote>(
            &mut self.conn,
            "credit_notes",
            "note_json",
            |_| Ok(()),
        )?;
        validate_json::<RoleDef>(&mut self.conn, "roles", "role_json", |r| {
            r.validate()?;
            Ok(())
        })?;
        validate_json::<crate::ActivityEvent>(
            &mut self.conn,
            "activity_events",
            "event_json",
            |_| Ok(()),
        )?;
        validate_json::<pos_domain::KitchenTicket>(
            &mut self.conn,
            "kitchen_tickets",
            "ticket_json",
            |_| Ok(()),
        )?;
        for ingredient in self.ingredients()? {
            ingredient.validate()?;
        }
        validate_json::<pos_domain::IngredientMovement>(
            &mut self.conn,
            "ingredient_movements",
            "movement_json",
            |_| Ok(()),
        )?;
        for promo in self.promotions()? {
            promo.validate()?;
        }
        for table in self.dining_tables()? {
            table.validate()?;
        }
        self.staff()?;
        self.tax_settings()?.validate()?;
        self.pricing_settings()?.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;
    #[test]
    fn refuses_future_schema_and_bad_snapshots_beyond_first_page() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("db.sqlite3");
        let mut pos = crate::Pos::open(&path)?;
        assert_eq!(
            inspect_database(&path)?.schema,
            i64::from(crate::SCHEMA_VERSION)
        );
        pos.conn.batch_execute("PRAGMA user_version=999")?;
        assert!(matches!(
            inspect_database(&path),
            Err(Error::UnsupportedSchema(999))
        ));
        pos.conn
            .batch_execute(&format!("PRAGMA user_version={}", crate::SCHEMA_VERSION))?;
        for timestamp in 1..=101 {
            let mut cart = pos_domain::Cart::new();
            cart.lines.push(pos_domain::CartLine {
                product_id: uuid::Uuid::new_v4(),
                quantity: 1,
                ..Default::default()
            });
            pos.hold(&cart, timestamp)?;
        }
        // A malformed old row must be refused even when held() would not read it.
        diesel::sql_query("INSERT INTO held_carts(id,updated_at,cart_json) VALUES ('bad',0,'{}')")
            .execute(&mut pos.conn)?;
        assert_eq!(pos.held()?.len(), 100);
        assert!(pos.validate_recovery_data().is_err());
        Ok(())
    }
}
