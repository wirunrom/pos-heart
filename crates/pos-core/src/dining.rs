//! Tables and kitchen tickets. A table's order is a held bill tagged with the
//! table (one open bill per table); sending to the kitchen stores a ticket
//! and the bill's new kitchen state in one transaction, so a retry or a
//! repeated send never fires the same dishes twice.
use crate::{
    Error, Pos, Result,
    schema::{dining_tables, held_carts, kitchen_tickets, products, sales},
};
use diesel::prelude::*;
use pos_domain::{Cart, DiningTable, KitchenTicket, TicketLine};
use std::collections::HashMap;
use uuid::Uuid;

/// Bumped tickets kept on the kitchen screen for recall.
const RECALL: i64 = 8;

type TableRow = (String, String, String, i32, i32, i64);
fn table_from((id, zone, name, seats, active, _): TableRow) -> Result<DiningTable> {
    Ok(DiningTable {
        id: Uuid::parse_str(&id).map_err(|_| Error::CorruptData)?,
        zone,
        name,
        seats: u32::try_from(seats).map_err(|_| Error::CorruptData)?,
        active: active == 1,
    })
}
fn find_table(conn: &mut SqliteConnection, id: Uuid) -> Result<Option<DiningTable>> {
    dining_tables::table
        .find(id.to_string())
        .select((
            dining_tables::id,
            dining_tables::zone,
            dining_tables::name,
            dining_tables::seats,
            dining_tables::active,
            dining_tables::sort,
        ))
        .first::<TableRow>(conn)
        .optional()?
        .map(table_from)
        .transpose()
}
/// The held bill open on `table`, other than `except`.
fn bill_on(conn: &mut SqliteConnection, table: Uuid, except: Uuid) -> Result<Option<Uuid>> {
    // Held bills are few; their JSON is scanned rather than indexed.
    for json in held_carts::table
        .select(held_carts::cart_json)
        .load::<String>(conn)?
    {
        let cart: Cart = serde_json::from_str(&json)?;
        if cart.table == Some(table) && cart.id != except {
            return Ok(Some(cart.id));
        }
    }
    Ok(None)
}
/// Checks a bill's table before it is held: the table exists, is active,
/// and holds no other open bill.
fn stored_cart(conn: &mut SqliteConnection, id: Uuid) -> Result<Option<Cart>> {
    held_carts::table
        .find(id.to_string())
        .select(held_carts::cart_json)
        .first::<String>(conn)
        .optional()?
        .map(|json| Ok(serde_json::from_str(&json)?))
        .transpose()
}
fn store_cart(conn: &mut SqliteConnection, cart: &Cart, now: i64) -> Result<()> {
    let json = serde_json::to_string(cart)?;
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
}
impl Pos {
    /// Moves some units of a bill to a new held bill (no table) in one
    /// IMMEDIATE transaction: "these two pay separately". The kitchen state
    /// is split from the stored bill, never the UI copy, so neither bill
    /// fires or cancels dishes because of the split. A retry with the same
    /// `new_id` returns the stored pair.
    pub fn split_bill(
        &mut self,
        cart: &Cart,
        take: &[(usize, u32)],
        new_id: Uuid,
        now: i64,
    ) -> Result<(Cart, Cart)> {
        cart.validate()?;
        self.conn.immediate_transaction(|conn| {
            if let Some(moved) = stored_cart(conn, new_id)? {
                let rest = stored_cart(conn, cart.id)?.ok_or(Error::CartNotFound)?;
                return Ok((rest, moved));
            }
            for id in [cart.id, new_id] {
                let paid = diesel::select(diesel::dsl::exists(
                    sales::table.filter(sales::cart_id.eq(id.to_string())),
                ))
                .get_result::<bool>(conn)?;
                if paid {
                    return Err(Error::AlreadyPaid);
                }
            }
            let mut cart = cart.clone();
            if let Some(stored) = stored_cart(conn, cart.id)? {
                cart.sent = stored.sent;
            }
            let (rest, moved) = pos_domain::split_cart(&cart, take, new_id)?;
            rest.validate()?;
            moved.validate()?;
            check_table(conn, &rest)?;
            store_cart(conn, &rest, now)?;
            store_cart(conn, &moved, now)?;
            Ok((rest, moved))
        })
    }
}
pub(crate) fn check_table(conn: &mut SqliteConnection, cart: &Cart) -> Result<()> {
    let Some(table) = cart.table else {
        return Ok(());
    };
    match find_table(conn, table)? {
        Some(t) if t.active => {}
        _ => return Err(Error::TableNotFound),
    }
    if bill_on(conn, table, cart.id)?.is_some() {
        return Err(Error::TableBusy);
    }
    Ok(())
}
fn save_held(conn: &mut SqliteConnection, cart: &Cart, now: i64) -> Result<()> {
    let json = serde_json::to_string(cart)?;
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
}
fn load_ticket(json: &str, number: i64, bumped: Option<i64>) -> Result<KitchenTicket> {
    let mut t: KitchenTicket = serde_json::from_str(json)?;
    t.number = u64::try_from(number).map_err(|_| Error::CorruptData)?;
    t.bumped_at = bumped;
    Ok(t)
}

impl Pos {
    /// Every table, in the shop's order (zone, then the order added).
    pub fn dining_tables(&mut self) -> Result<Vec<DiningTable>> {
        dining_tables::table
            .order((dining_tables::sort, dining_tables::name))
            .select((
                dining_tables::id,
                dining_tables::zone,
                dining_tables::name,
                dining_tables::seats,
                dining_tables::active,
                dining_tables::sort,
            ))
            .load::<TableRow>(&mut self.conn)?
            .into_iter()
            .map(table_from)
            .collect()
    }
    /// Adds or edits a table (Settings permission). Switching a table off
    /// is refused while it holds an open bill.
    pub fn save_table(&mut self, table: &DiningTable) -> Result<DiningTable> {
        table.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Settings))?;
            if !table.active && bill_on(conn, table.id, Uuid::nil())?.is_some() {
                return Err(Error::TableBusy);
            }
            let duplicate = diesel::select(diesel::dsl::exists(
                dining_tables::table
                    .filter(dining_tables::name.eq(table.name.trim()))
                    .filter(dining_tables::zone.eq(table.zone.trim()))
                    .filter(dining_tables::active.eq(1))
                    .filter(dining_tables::id.ne(table.id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            if duplicate && table.active {
                return Err(Error::DuplicateTable);
            }
            let saved = DiningTable {
                zone: table.zone.trim().to_owned(),
                name: table.name.trim().to_owned(),
                ..table.clone()
            };
            let exists = find_table(conn, table.id)?.is_some();
            if exists {
                diesel::update(dining_tables::table.find(table.id.to_string()))
                    .set((
                        dining_tables::zone.eq(&saved.zone),
                        dining_tables::name.eq(&saved.name),
                        dining_tables::seats.eq(i32::try_from(saved.seats).unwrap_or(0)),
                        dining_tables::active.eq(i32::from(saved.active)),
                    ))
                    .execute(conn)?;
            } else {
                let sort = dining_tables::table
                    .select(diesel::dsl::max(dining_tables::sort))
                    .first::<Option<i64>>(conn)?
                    .unwrap_or(0)
                    + 1;
                diesel::insert_into(dining_tables::table)
                    .values((
                        dining_tables::id.eq(saved.id.to_string()),
                        dining_tables::zone.eq(&saved.zone),
                        dining_tables::name.eq(&saved.name),
                        dining_tables::seats.eq(i32::try_from(saved.seats).unwrap_or(0)),
                        dining_tables::active.eq(i32::from(saved.active)),
                        dining_tables::sort.eq(sort),
                    ))
                    .execute(conn)?;
            }
            Ok(saved)
        })
    }
    /// Sends what changed since the last send: new dishes become a ticket,
    /// removed ones a cancellation ticket. The bill is held with its new
    /// kitchen state in the same transaction; an emptied bill is discarded.
    /// Returns the bill as stored and the ticket, if anything changed.
    pub fn send_to_kitchen(
        &mut self,
        cart: &Cart,
        now: i64,
    ) -> Result<(Cart, Option<KitchenTicket>)> {
        if !cart.lines.is_empty() {
            cart.validate()?;
        }
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let paid = diesel::select(diesel::dsl::exists(
                sales::table.filter(sales::cart_id.eq(cart.id.to_string())),
            ))
            .get_result::<bool>(conn)?;
            if paid {
                return Err(Error::AlreadyPaid);
            }
            check_table(conn, cart)?;
            let staff = crate::staff::authorize(conn, &session, None)?;
            // Diff against what is stored, never the screen's copy: a send
            // whose reply was lost must not fire the same dishes again.
            let mut cart = cart.clone();
            if let Some(json) = held_carts::table
                .find(cart.id.to_string())
                .select(held_carts::cart_json)
                .first::<String>(conn)
                .optional()?
            {
                cart.sent = serde_json::from_str::<Cart>(&json)?.sent;
            }
            let cart = &cart;
            // Names and kitchen flags from the catalog, archived included:
            // a cancelled dish may have been archived since it was sent.
            let mut ids: Vec<String> = cart
                .lines
                .iter()
                .map(|l| l.product_id.to_string())
                .collect();
            ids.extend(cart.sent.iter().map(|s| s.product_id.to_string()));
            let catalog: HashMap<String, (String, bool, String)> = products::table
                .filter(products::id.eq_any(&ids))
                .select((
                    products::id,
                    products::name,
                    products::kitchen,
                    products::options_json,
                ))
                .load::<(String, String, bool, String)>(conn)?
                .into_iter()
                .map(|(id, name, kitchen, options)| (id, (name, kitchen, options)))
                .collect();
            let kitchen = |id: Uuid| catalog.get(&id.to_string()).is_some_and(|c| c.1);
            let (changes, sent) = pos_domain::kitchen_changes(cart, kitchen);
            let mut stored = cart.clone();
            stored.sent = sent;
            if stored.lines.is_empty() {
                diesel::delete(held_carts::table.find(cart.id.to_string())).execute(conn)?;
            } else {
                save_held(conn, &stored, now)?;
            }
            if changes.is_empty() {
                return Ok((stored, None));
            }
            let mut lines = Vec::with_capacity(changes.len());
            for (item, quantity) in changes {
                let (name, _, options_json) = catalog
                    .get(&item.product_id.to_string())
                    .cloned()
                    .unwrap_or_default();
                let groups: Vec<pos_domain::OptionGroup> =
                    serde_json::from_str(&options_json).unwrap_or_default();
                let options = item
                    .options
                    .iter()
                    .filter_map(|o| {
                        groups
                            .iter()
                            .find(|g| g.id == o.group_id)?
                            .choices
                            .iter()
                            .find(|c| c.id == o.choice_id)
                            .map(|c| c.name.clone())
                    })
                    .collect();
                lines.push(TicketLine {
                    product_id: item.product_id,
                    name,
                    quantity,
                    options,
                    note: item.note,
                    done: false,
                });
            }
            let table = match cart.table {
                Some(id) => find_table(conn, id)?.map(|t| t.name).unwrap_or_default(),
                None => String::new(),
            };
            let mut ticket = KitchenTicket {
                id: Uuid::new_v4(),
                number: 0,
                cart_id: cart.id,
                table: if table.is_empty() {
                    cart.label.clone()
                } else {
                    table
                },
                guests: cart.guests,
                created_at: now,
                lines,
                bumped_at: None,
                staff,
            };
            let number = diesel::insert_into(kitchen_tickets::table)
                .values((
                    kitchen_tickets::id.eq(ticket.id.to_string()),
                    kitchen_tickets::cart_id.eq(cart.id.to_string()),
                    kitchen_tickets::created_at.eq(now),
                    kitchen_tickets::ticket_json.eq("{}"),
                ))
                .returning(kitchen_tickets::number)
                .get_result::<i64>(conn)?;
            ticket.number = u64::try_from(number).map_err(|_| Error::CorruptData)?;
            diesel::update(kitchen_tickets::table.find(number))
                .set(kitchen_tickets::ticket_json.eq(serde_json::to_string(&ticket)?))
                .execute(conn)?;
            // Cancelling dishes the kitchen already has is worth an audit trail.
            if ticket.lines.iter().any(|l| l.quantity < 0) {
                let cancelled: Vec<_> = ticket
                    .lines
                    .iter()
                    .filter(|l| l.quantity < 0)
                    .map(|l| serde_json::json!({"name": l.name, "quantity": -l.quantity}))
                    .collect();
                crate::audit::record(
                    conn,
                    crate::ActivityKind::Kitchen,
                    &ticket.id.to_string(),
                    now,
                    &ticket.staff,
                    &serde_json::json!({
                        "ticket": ticket.number,
                        "table": ticket.table,
                        "cancelled": cancelled,
                    }),
                )?;
            }
            Ok((stored, Some(ticket)))
        })
    }
    /// Open tickets oldest first, then the most recently bumped for recall.
    pub fn kitchen_tickets(&mut self) -> Result<Vec<KitchenTicket>> {
        let open = kitchen_tickets::table
            .filter(kitchen_tickets::bumped_at.is_null())
            .order(kitchen_tickets::number)
            .limit(200)
            .select((
                kitchen_tickets::number,
                kitchen_tickets::bumped_at,
                kitchen_tickets::ticket_json,
            ))
            .load::<(i64, Option<i64>, String)>(&mut self.conn)?;
        let done = kitchen_tickets::table
            .filter(kitchen_tickets::bumped_at.is_not_null())
            .order(kitchen_tickets::bumped_at.desc())
            .limit(RECALL)
            .select((
                kitchen_tickets::number,
                kitchen_tickets::bumped_at,
                kitchen_tickets::ticket_json,
            ))
            .load::<(i64, Option<i64>, String)>(&mut self.conn)?;
        open.into_iter()
            .chain(done)
            .map(|(n, b, json)| load_ticket(&json, n, b))
            .collect()
    }
    /// Marks a whole ticket done (`Some(time)`) or recalls it (`None`).
    pub fn bump_ticket(&mut self, id: Uuid, bumped_at: Option<i64>) -> Result<()> {
        let changed =
            diesel::update(kitchen_tickets::table.filter(kitchen_tickets::id.eq(id.to_string())))
                .set(kitchen_tickets::bumped_at.eq(bumped_at))
                .execute(&mut self.conn)?;
        if changed == 0 {
            return Err(Error::TicketNotFound);
        }
        Ok(())
    }
    /// Ticks one line of a ticket as cooked (or not).
    pub fn set_ticket_line(&mut self, id: Uuid, line: usize, done: bool) -> Result<()> {
        self.conn.immediate_transaction(|conn| {
            let (number, json) = kitchen_tickets::table
                .filter(kitchen_tickets::id.eq(id.to_string()))
                .select((kitchen_tickets::number, kitchen_tickets::ticket_json))
                .first::<(i64, String)>(conn)
                .optional()?
                .ok_or(Error::TicketNotFound)?;
            let mut ticket: KitchenTicket = serde_json::from_str(&json)?;
            let l = ticket.lines.get_mut(line).ok_or(Error::TicketNotFound)?;
            l.done = done;
            diesel::update(kitchen_tickets::table.find(number))
                .set(kitchen_tickets::ticket_json.eq(serde_json::to_string(&ticket)?))
                .execute(conn)?;
            Ok(())
        })
    }
}
