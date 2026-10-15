use crate::{
    Error, Result,
    schema::{held_carts, sale_refunds, sale_voids, sales},
};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_types::{Integer, Text},
};
use pos_domain::{Cart, SaleRecord};
use std::path::Path;
use uuid::Uuid;

/// Highest supported business schema. Backups and recovery use the same gate.
pub const SCHEMA_VERSION: i32 = 24;

/// Owned by the database worker. Diesel and raw SQL share this one connection.
pub struct Pos {
    pub(crate) conn: SqliteConnection,
    /// Signed-in staff and a one-shot approval (see staff.rs).
    pub(crate) session: crate::staff::Session,
    /// The register this connection sells for: its shift and drawer.
    pub(crate) register: uuid::Uuid,
}
#[derive(QueryableByName)]
struct SchemaVersion {
    #[diesel(sql_type = Integer)]
    user_version: i32,
}
/// One paired device's sign-in, approval and register, kept by the hub
/// between that device's requests (multi-device phase 1). PIN lockout
/// counts per device.
#[derive(Debug, Clone)]
pub struct DeviceContext {
    session: crate::staff::Session,
    register: uuid::Uuid,
}
impl DeviceContext {
    pub fn new(register: uuid::Uuid) -> Self {
        Self {
            session: crate::staff::Session::default(),
            register,
        }
    }
    pub fn register(&self) -> uuid::Uuid {
        self.register
    }
}
impl Pos {
    /// Runs `f` as the device: its session and register replace this
    /// connection's for the call and are written back afterwards, so the
    /// counter's own sign-in is untouched.
    pub fn as_device<R>(&mut self, device: &mut DeviceContext, f: impl FnOnce(&mut Pos) -> R) -> R {
        std::mem::swap(&mut self.session, &mut device.session);
        std::mem::swap(&mut self.register, &mut device.register);
        let out = f(self);
        std::mem::swap(&mut self.session, &mut device.session);
        std::mem::swap(&mut self.register, &mut device.register);
        out
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_str().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Database path must be valid Unicode",
            ))
        })?;
        let mut conn = SqliteConnection::establish(path)?;
        for pragma in [
            "PRAGMA busy_timeout=5000;",
            "PRAGMA foreign_keys=ON;",
            "PRAGMA journal_mode=WAL;",
            "PRAGMA synchronous=FULL;",
            "PRAGMA cache_size=-8192;",
        ] {
            conn.batch_execute(pragma)?;
        }
        conn.immediate_transaction::<_, Error, _>(|conn| {
            let version = diesel::sql_query("PRAGMA user_version")
                .get_result::<SchemaVersion>(conn)?
                .user_version;
            if !(0..=SCHEMA_VERSION).contains(&version) {
                return Err(Error::UnsupportedSchema(i64::from(version)));
            }
            if version == 0 {
                conn.batch_execute(include_str!("../migrations/001_initial.sql"))?;
            }
            if version < 2 {
                conn.batch_execute(include_str!("../migrations/002_sale_voids.sql"))?;
            }
            if version < 3 {
                conn.batch_execute(include_str!("../migrations/003_shifts.sql"))?;
            }
            if version < 4 {
                conn.batch_execute(include_str!("../migrations/004_payment_methods.sql"))?;
            }
            if version < 5 {
                conn.batch_execute(include_str!("../migrations/005_catalog.sql"))?;
            }
            if version < 6 {
                conn.batch_execute(include_str!("../migrations/006_product_options.sql"))?;
            }
            if version < 7 {
                conn.batch_execute(include_str!("../migrations/007_stock_movements.sql"))?;
            }
            if version < 8 {
                conn.batch_execute(include_str!("../migrations/008_sale_refunds.sql"))?;
            }
            if version < 9 {
                conn.batch_execute(include_str!("../migrations/009_staff.sql"))?;
            }
            if version < 10 {
                conn.batch_execute(include_str!("../migrations/010_tax_invoices.sql"))?;
            }
            if version < 11 {
                conn.batch_execute(include_str!("../migrations/011_pricing.sql"))?;
            }
            if version < 12 {
                conn.batch_execute(include_str!("../migrations/012_roles.sql"))?;
            }
            if version < 13 {
                conn.batch_execute(include_str!("../migrations/013_cost.sql"))?;
            }
            if version < 14 {
                conn.batch_execute(include_str!("../migrations/014_audit.sql"))?;
            }
            if version < 15 {
                conn.batch_execute(include_str!("../migrations/015_dining.sql"))?;
            }
            if version < 16 {
                conn.batch_execute(include_str!("../migrations/016_credit_notes.sql"))?;
            }
            if version < 17 {
                conn.batch_execute(include_str!("../migrations/017_promotions.sql"))?;
            }
            if version < 18 {
                conn.batch_execute(include_str!("../migrations/018_ingredients.sql"))?;
            }
            if version < 19 {
                conn.batch_execute(include_str!("../migrations/019_option_recipes.sql"))?;
            }
            if version < 20 {
                conn.batch_execute(include_str!("../migrations/020_product_images.sql"))?;
            }
            if version < 21 {
                conn.batch_execute(include_str!("../migrations/021_categories.sql"))?;
            }
            if version < 22 {
                conn.batch_execute(include_str!("../migrations/022_option_sets.sql"))?;
            }
            if version < 23 {
                conn.batch_execute(include_str!("../migrations/023_purchasing.sql"))?;
            }
            if version < 24 {
                conn.batch_execute(include_str!("../migrations/024_registers.sql"))?;
            }
            crate::staff::ensure_roles(conn)?;
            Ok(())
        })?;
        Ok(Self {
            conn,
            session: crate::staff::Session::default(),
            register: crate::MAIN_REGISTER,
        })
    }
    pub fn hold(&mut self, cart: &Cart, now: i64) -> Result<()> {
        cart.validate()?;
        self.conn.immediate_transaction(|conn| {
            let paid = diesel::select(diesel::dsl::exists(
                sales::table.filter(sales::cart_id.eq(cart.id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            if paid {
                return Err(Error::AlreadyPaid);
            }
            crate::dining::check_table(conn, cart)?;
            // Only `send_to_kitchen` changes what the kitchen has: an autosave
            // queued before a send must not roll that state back.
            let mut cart = cart.clone();
            if let Some(stored) = held_carts::table
                .find(cart.id.to_string())
                .select(held_carts::cart_json)
                .first::<String>(conn)
                .optional()?
            {
                let stored: Cart = serde_json::from_str(&stored)?;
                cart.sent = stored.sent;
            }
            let json = serde_json::to_string(&cart)?;
            diesel::insert_into(held_carts::table)
                .values((
                    held_carts::id.eq(cart.id.to_string()),
                    held_carts::updated_at.eq(now),
                    held_carts::cart_json.eq(&json),
                ))
                .on_conflict(held_carts::id)
                .do_update()
                .set((
                    held_carts::updated_at.eq(now),
                    held_carts::cart_json.eq(&json),
                ))
                .execute(conn)?;
            Ok(())
        })
    }
    /// Loading never deletes a held bill; checkout removes it only after payment commits.
    pub fn held(&mut self) -> Result<Vec<Cart>> {
        let rows = held_carts::table
            .order(held_carts::updated_at.desc())
            .limit(100)
            .select(held_carts::cart_json)
            .load::<String>(&mut self.conn)?;
        rows.into_iter()
            .map(|json| Ok(serde_json::from_str(&json)?))
            .collect()
    }
    pub fn remove_held(&mut self, id: Uuid) -> Result<()> {
        if diesel::delete(held_carts::table.filter(held_carts::id.eq(id.to_string())))
            .execute(&mut self.conn)?
            == 0
        {
            return Err(Error::CartNotFound);
        }
        Ok(())
    }
    pub fn history(&mut self, offset: u32) -> Result<Vec<SaleRecord>> {
        self.conn.transaction(|conn| {
            let rows = sales::table
                .left_join(sale_voids::table.on(sale_voids::sale_id.eq(sales::cart_id)))
                .order(sales::number.desc())
                .limit(50)
                .offset(i64::from(offset))
                .select((
                    sales::cart_id,
                    sales::receipt_json,
                    sale_voids::void_json.nullable(),
                ))
                .load::<(String, String, Option<String>)>(conn)?;
            let ids: Vec<&String> = rows.iter().map(|(id, ..)| id).collect();
            let mut refunds: std::collections::HashMap<String, Vec<pos_domain::SaleRefund>> =
                std::collections::HashMap::new();
            for (sale_id, json) in sale_refunds::table
                .filter(sale_refunds::sale_id.eq_any(&ids))
                .order((sale_refunds::created_at, sale_refunds::id))
                .select((sale_refunds::sale_id, sale_refunds::refund_json))
                .load::<(String, String)>(conn)?
            {
                refunds
                    .entry(sale_id)
                    .or_default()
                    .push(serde_json::from_str(&json)?);
            }
            let mut invoices = std::collections::HashMap::new();
            for (sale_id, json) in crate::schema::tax_invoices::table
                .filter(crate::schema::tax_invoices::sale_id.eq_any(&ids))
                .select((
                    crate::schema::tax_invoices::sale_id,
                    crate::schema::tax_invoices::invoice_json,
                ))
                .load::<(String, String)>(conn)?
            {
                invoices.insert(sale_id, serde_json::from_str(&json)?);
            }
            let mut notes: std::collections::HashMap<String, Vec<pos_domain::CreditNote>> =
                std::collections::HashMap::new();
            for (sale_id, json) in crate::schema::credit_notes::table
                .filter(crate::schema::credit_notes::sale_id.eq_any(&ids))
                .order(crate::schema::credit_notes::number)
                .select((
                    crate::schema::credit_notes::sale_id,
                    crate::schema::credit_notes::note_json,
                ))
                .load::<(String, String)>(conn)?
            {
                notes
                    .entry(sale_id)
                    .or_default()
                    .push(serde_json::from_str(&json)?);
            }
            rows.into_iter()
                .map(|(id, sale, cancellation)| {
                    Ok(SaleRecord {
                        credit_notes: notes.remove(&id).unwrap_or_default(),
                        tax_invoice: invoices.remove(&id),
                        sale: serde_json::from_str(&sale)?,
                        cancellation: cancellation
                            .map(|json| serde_json::from_str(&json))
                            .transpose()?,
                        refunds: refunds.remove(&id).unwrap_or_default(),
                    })
                })
                .collect()
        })
    }
    /// SQLite creates a consistent standalone snapshot; never copy live DB/WAL files.
    pub fn backup(&mut self, destination: &Path) -> Result<()> {
        if destination.exists() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Choose a new backup filename",
            )));
        }
        let destination = destination.to_str().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Backup path must be valid Unicode",
            ))
        })?;
        diesel::sql_query("VACUUM main INTO ?")
            .bind::<Text, _>(destination)
            .execute(&mut self.conn)?;
        Ok(())
    }
}
