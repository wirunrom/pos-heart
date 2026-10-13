use eframe::egui;
use pos_core::{CatalogCounts, Error, Pos, ProductQuery};
use pos_domain::{
    Cart, DailySummary, Money, PaymentRequest, Product, ProductInput, Sale, SaleRecord, SaleVoid,
    ShiftCount, VoidRequest,
};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};
use uuid::Uuid;

#[derive(Default)]
pub struct Data {
    pub products: Vec<Product>,
    pub counts: CatalogCounts,
    pub held: Vec<Cart>,
    pub sales: Vec<SaleRecord>,
    pub summary: DailySummary,
    /// Same weekday one week earlier, for report comparisons.
    pub compare: DailySummary,
    pub today_summary: DailySummary,
    pub shift: Option<pos_domain::Shift>,
    pub shifts: Vec<pos_domain::Shift>,
    pub shift_numbers: HashMap<Uuid, u64>,
    pub movements: Vec<pos_domain::CashMovement>,
    /// Stock history of the product open in the Products inspector.
    pub stock_history: Vec<pos_domain::StockMovement>,
    /// Roster (active first). Sign-in is required while any staff is active.
    pub staff: Vec<pos_domain::Staff>,
    pub staff_enabled: bool,
    pub signed_in: Option<pos_domain::StaffRef>,
    /// Owner first, then built-in and custom roles.
    pub roles: Vec<pos_domain::RoleDef>,
    pub tax: pos_domain::TaxSettings,
    pub pricing: pos_domain::PricingSettings,
    pub tables: Vec<pos_domain::DiningTable>,
    /// Open kitchen tickets oldest first, then recently bumped ones.
    pub kitchen: Vec<pos_domain::KitchenTicket>,
}
#[derive(Clone)]
pub struct Query {
    pub products: ProductQuery,
    pub sale_offset: u32,
    pub report_start: time::Date,
    pub report_end: time::Date,
    pub shift_offset: u32,
    pub movement_shift: Option<Uuid>,
    pub movement_offset: u32,
    pub stock_product: Option<Uuid>,
}
#[derive(Clone)]
pub enum Command {
    Refresh(Query),
    Save(ProductInput),
    Archive(Uuid),
    SetOnSale(Vec<Uuid>, bool),
    SetCategory(Vec<Uuid>, String),
    AdjustStock(pos_domain::StockRequest),
    Pay(Cart, PaymentRequest, Uuid),
    Void(VoidRequest, Uuid),
    RefundItems(pos_domain::RefundRequest, Uuid),
    OpenShift(Uuid, Money),
    MoveCash(pos_domain::CashMovement),
    CloseShift(Uuid, u64, ShiftCount),
    Hold(Cart),
    RemoveHeld(Uuid),
    Resume(Uuid),
    Lookup(String),
    Demo,
    RecoveryBackup(PathBuf, PathBuf, Cart, Option<Uuid>),
    InspectBackup(PathBuf),
    PrepareRecovery(PathBuf, crate::backup::Review),
    PreviewImport(PathBuf),
    ImportProducts(String),
    Activity(pos_core::ActivityQuery),
    ExportProducts(PathBuf),
    ExportReport(PathBuf, time::Date, time::Date),
    /// Background: keeps the current bill in SQLite so a crash or exit loses nothing.
    Autosave(Cart),
    /// Background: drops the saved copy of a bill that became empty or was cleared.
    Discard(Uuid),
    /// Background: reopens the bill that was current when the app last closed.
    Restore(Uuid),
    SignIn(Uuid, String),
    SignOut,
    SaveStaff(pos_domain::StaffInput),
    SaveRole(pos_domain::RoleDef),
    DeleteRole(String),
    SetTax(pos_domain::TaxSettings),
    SetPricing(pos_domain::PricingSettings),
    SaveTable(pos_domain::DiningTable),
    SendKitchen(Cart),
    /// Kitchen screen: bump (Some time) or recall (None) a ticket.
    BumpTicket(Uuid, Option<i64>),
    TicketLine(Uuid, usize, bool),
    IssueInvoice(Uuid, pos_domain::Buyer),
    /// Runs `inner` once with an approver's PIN for a restricted action.
    Approved {
        approver: Uuid,
        pin: String,
        permission: pos_domain::Permission,
        inner: Box<Command>,
    },
}
impl Command {
    /// Background commands never block the UI or surface errors as banners.
    pub fn background(&self) -> bool {
        matches!(
            self,
            Self::Autosave(_)
                | Self::Discard(_)
                | Self::Restore(_)
                | Self::BumpTicket(..)
                | Self::TicketLine(..)
        )
    }
}
pub enum Event {
    Ready(Box<Data>),
    ShiftOpened,
    CashMoved,
    ShiftClosed(Box<pos_domain::Shift>),
    Saved(Box<Product>),
    Bulk(usize),
    StockAdjusted(pos_domain::StockMovement),
    Archived,
    Paid(Sale),
    Voided(SaleVoid),
    Refunded(pos_domain::SaleRefund),
    Resumed(Cart, Vec<Product>),
    Held(Uuid),
    Removed,
    Found(String, Option<Box<Product>>),
    Demo,
    Backup(PathBuf),
    Exported(PathBuf),
    BackupInspected(crate::backup::Review),
    RecoveryPending(PathBuf),
    ImportPreview(pos_core::ImportPreview),
    ProductsImported(usize),
    Activity(Vec<pos_core::ActivityEvent>),
    Restored(Cart, Vec<Product>),
    SignedIn(pos_domain::StaffRef),
    SignedOut,
    StaffSaved(pos_domain::Staff),
    RoleSaved,
    TaxSaved,
    PricingSaved,
    TableSaved,
    KitchenSent(Cart, Option<Box<pos_domain::KitchenTicket>>),
    Kitchen(Vec<pos_domain::KitchenTicket>),
    InvoiceIssued(pos_domain::TaxInvoice),
    /// Background command finished (successfully or not); never changes `busy`.
    Quiet,
    Failed(Error),
}
pub struct Worker {
    pub tx: SyncSender<Command>,
    pub rx: Receiver<Event>,
}
fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
pub fn today() -> time::Date {
    time::OffsetDateTime::now_utc()
        .to_offset(time::macros::offset!(+7))
        .date()
}
impl Worker {
    pub fn start(path: PathBuf, context: egui::Context) -> std::io::Result<Self> {
        let (tx, commands) = mpsc::sync_channel::<Command>(16);
        let (events, rx) = mpsc::channel();
        thread::Builder::new()
            .name("pos-database".into())
            .spawn(move || {
                let send = |event: Event| {
                    let _ = events.send(event);
                    context.request_repaint();
                };
                let mut pos = match Pos::open(path) {
                    Ok(pos) => pos,
                    Err(e) => {
                        send(Event::Failed(e));
                        return;
                    }
                };
                let initial = Query {
                    products: ProductQuery::default(),
                    sale_offset: 0,
                    report_start: today(),
                    report_end: today(),
                    shift_offset: 0,
                    movement_shift: None,
                    movement_offset: 0,
                    stock_product: None,
                };
                send(match load(&mut pos, &initial) {
                    Ok(data) => Event::Ready(Box::new(data)),
                    Err(e) => Event::Failed(e),
                });
                while let Ok(command) = commands.recv() {
                    let background = command.background();
                    let result = run(&mut pos, command);
                    let stopping = matches!(&result, Ok(Event::RecoveryPending(_)));
                    send(match result {
                        Err(_) if background => Event::Quiet,
                        other => other.unwrap_or_else(Event::Failed),
                    });
                    if stopping {
                        break;
                    }
                }
            })?;
        Ok(Self { tx, rx })
    }
}
fn run(pos: &mut Pos, command: Command) -> pos_core::Result<Event> {
    Ok(match command {
        Command::Refresh(query) => Event::Ready(Box::new(load(pos, &query)?)),
        Command::Save(input) => Event::Saved(Box::new(pos.save_product_at(input, now())?)),
        Command::Archive(id) => {
            pos.archive_product(id)?;
            Event::Archived
        }
        Command::SetOnSale(ids, on) => Event::Bulk(pos.set_on_sale(&ids, on)?),
        Command::SetCategory(ids, category) => Event::Bulk(pos.set_category(&ids, &category)?),
        Command::AdjustStock(request) => Event::StockAdjusted(pos.adjust_stock(&request, now())?),
        Command::Pay(cart, payment, shift) => {
            Event::Paid(pos.checkout_with_payment(&cart, payment, Some(shift), now())?)
        }
        Command::Void(request, shift) => {
            Event::Voided(pos.void_sale_for_shift(&request, Some(shift), now())?)
        }
        Command::RefundItems(request, shift) => {
            Event::Refunded(pos.refund_items(&request, Some(shift), now())?)
        }
        Command::OpenShift(id, opening) => {
            pos.open_shift(id, opening, now())?;
            Event::ShiftOpened
        }
        Command::MoveCash(mut movement) => {
            movement.created_at = now();
            pos.move_cash(&movement)?;
            Event::CashMoved
        }
        Command::CloseShift(id, revision, count) => Event::ShiftClosed(Box::new(
            pos.close_shift_with(id, revision, &count, now())?,
        )),
        Command::Hold(cart) => {
            pos.hold(&cart, now())?;
            Event::Held(cart.id)
        }
        Command::RemoveHeld(id) => {
            pos.remove_held(id)?;
            Event::Removed
        }
        Command::Resume(id) => {
            let (cart, products) = pos.resume(id)?;
            Event::Resumed(cart, products)
        }
        Command::Lookup(code) => {
            let found = pos.find_by_code(&code)?.map(Box::new);
            Event::Found(code, found)
        }
        Command::Demo => {
            pos.seed_demo()?;
            Event::Demo
        }
        Command::RecoveryBackup(data, path, cart, saved) => {
            pos.authorize_recovery()?;
            if !cart.lines.is_empty() {
                pos.hold(&cart, now())?;
                std::fs::write(data.join("current-bill.txt"), cart.id.to_string())?;
            }
            if cart.lines.is_empty() {
                if let Some(id) = saved {
                    match pos.remove_held(id) {
                        Ok(()) | Err(pos_core::Error::CartNotFound) => {}
                        Err(e) => return Err(e),
                    }
                }
                match std::fs::remove_file(data.join("current-bill.txt")) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Event::Backup(crate::backup::create(pos, &data, &path)?)
        }
        Command::InspectBackup(path) => {
            pos.authorize_recovery()?;
            Event::BackupInspected(crate::backup::inspect(&path)?)
        }
        Command::PrepareRecovery(data, review) => {
            Event::RecoveryPending(crate::backup::prepare(pos, &data, &review)?)
        }
        Command::PreviewImport(path) => {
            use std::io::Read;
            let mut source = String::new();
            std::fs::File::open(path)?
                .take(pos_core::MAX_IMPORT_BYTES as u64 + 1)
                .read_to_string(&mut source)?;
            Event::ImportPreview(pos.preview_product_import(&source)?)
        }
        Command::ImportProducts(source) => Event::ProductsImported(pos.import_products(&source)?),
        Command::Activity(query) => Event::Activity(pos.activity(&query)?),
        Command::ExportProducts(path) => {
            export_products(pos, &path)?;
            Event::Exported(path)
        }
        Command::ExportReport(path, start, end) => {
            export_report(pos, &path, start, end)?;
            Event::Exported(path)
        }
        Command::Autosave(cart) => {
            pos.hold(&cart, now())?;
            Event::Quiet
        }
        Command::Discard(id) => {
            pos.remove_held(id)?;
            Event::Quiet
        }
        Command::Restore(id) => {
            let (cart, products) = pos.resume(id)?;
            Event::Restored(cart, products)
        }
        Command::SignIn(id, pin) => Event::SignedIn(pos.sign_in(id, &pin)?),
        Command::SignOut => {
            pos.sign_out();
            Event::SignedOut
        }
        Command::SaveStaff(input) => Event::StaffSaved(pos.save_staff(&input, now())?),
        Command::SaveTable(t) => {
            pos.save_table(&t)?;
            Event::TableSaved
        }
        Command::SendKitchen(cart) => {
            let (cart, ticket) = pos.send_to_kitchen(&cart, now())?;
            Event::KitchenSent(cart, ticket.map(Box::new))
        }
        Command::BumpTicket(id, at) => {
            pos.bump_ticket(id, at)?;
            Event::Kitchen(pos.kitchen_tickets()?)
        }
        Command::TicketLine(id, line, done) => {
            pos.set_ticket_line(id, line, done)?;
            Event::Kitchen(pos.kitchen_tickets()?)
        }
        Command::SetPricing(p) => {
            pos.set_pricing_settings(&p)?;
            Event::PricingSaved
        }
        Command::SetTax(tax) => {
            pos.set_tax_settings(&tax)?;
            Event::TaxSaved
        }
        Command::IssueInvoice(sale, buyer) => {
            Event::InvoiceIssued(pos.issue_tax_invoice(sale, &buyer, now())?)
        }
        Command::SaveRole(role) => {
            pos.save_role(&role)?;
            Event::RoleSaved
        }
        Command::DeleteRole(id) => {
            pos.delete_role(&id)?;
            Event::RoleSaved
        }
        Command::Approved {
            approver,
            pin,
            permission,
            inner,
        } => {
            pos.approve(approver, &pin, permission)?;
            let result = run(pos, *inner);
            // Approval covers this one command only.
            pos.clear_approval();
            result?
        }
    })
}
fn load(pos: &mut Pos, q: &Query) -> pos_core::Result<Data> {
    let period = crate::report_ui::Period {
        start: q.report_start,
        end: q.report_end,
    };
    let summary = pos.summary(period.start, period.end)?;
    let compare = match period.previous() {
        Some(p) => pos.summary(p.start, p.end)?,
        None => DailySummary::default(),
    };
    let today_summary = if period == crate::report_ui::Period::day(today()) {
        summary.clone()
    } else {
        pos.daily_summary(today())?
    };
    let shift = pos.active_shift()?;
    let movements = q
        .movement_shift
        .or_else(|| shift.as_ref().map(|s| s.id))
        .map(|id| pos.cash_movements(id, q.movement_offset))
        .transpose()?
        .unwrap_or_default();
    let shifts = pos.shift_history(q.shift_offset)?;
    let shift_numbers = pos.shift_numbers()?;
    let stock_history = q
        .stock_product
        .map(|id| pos.stock_movements(id, 0))
        .transpose()?
        .unwrap_or_default();
    let staff = pos.staff()?;
    Ok(Data {
        staff_enabled: staff.iter().any(|s| s.active),
        staff,
        signed_in: pos.signed_in()?,
        roles: pos.roles()?,
        tax: pos.tax_settings()?,
        pricing: pos.pricing_settings()?,
        tables: pos.dining_tables()?,
        kitchen: pos.kitchen_tickets()?,
        stock_history,
        shift,
        shifts,
        shift_numbers,
        movements,
        products: pos.catalog(&q.products)?,
        counts: pos.catalog_counts()?,
        held: pos.held()?,
        sales: pos.history(q.sale_offset)?,
        summary,
        compare,
        today_summary,
    })
}

fn is_number(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.')
}
/// Quotes CSV cells and neutralises spreadsheet formulas (`=`, `+`, `-`, `@`).
/// Plain signed numbers such as `-345.00` stay numeric.
fn cell(text: &str) -> String {
    let guarded = if text.starts_with(['=', '+', '-', '@']) && !is_number(text) {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if guarded.contains([',', '"', '\n', '\r']) || guarded != guarded.trim() {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}
fn write_csv(path: &Path, rows: &[Vec<String>]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    // UTF-8 BOM so spreadsheet apps read Thai correctly.
    let mut out = String::from("\u{feff}");
    for row in rows {
        let line: Vec<String> = row.iter().map(|c| cell(c)).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    file.write_all(out.as_bytes())?;
    file.sync_all()
}
fn export_products(pos: &mut Pos, path: &Path) -> pos_core::Result<()> {
    let mut rows = vec![
        [
            "sku",
            "name",
            "category",
            "price",
            "stock",
            "barcode",
            "on_sale",
            "service_charge",
            "cost",
        ]
        .map(String::from)
        .to_vec(),
    ];
    let mut offset = 0;
    loop {
        let page = pos.catalog(&ProductQuery {
            offset,
            limit: 1000,
            ..ProductQuery::default()
        })?;
        let n = page.len();
        for p in page {
            rows.push(vec![
                p.sku,
                p.name,
                p.category,
                p.price.to_string(),
                p.stock.map(|s| s.to_string()).unwrap_or_default(),
                p.barcode.unwrap_or_default(),
                if p.on_sale { "1" } else { "0" }.into(),
                if p.service_charge { "1" } else { "0" }.into(),
                p.cost.map(|c| c.to_string()).unwrap_or_default(),
            ]);
        }
        if n < 1000 {
            break;
        }
        offset += 1000;
    }
    write_csv(path, &rows)?;
    Ok(())
}
fn export_report(
    pos: &mut Pos,
    path: &Path,
    start: time::Date,
    end: time::Date,
) -> pos_core::Result<()> {
    let s = pos.summary(start, end)?;
    let mut rows: Vec<Vec<String>> = vec![
        vec!["from".into(), s.date.clone()],
        vec!["to".into(), s.end_date.clone()],
        vec!["gross_sales".into(), s.gross_sales.to_string()],
        vec!["discounts".into(), s.discounts.to_string()],
        vec!["service_charges".into(), s.service_charges.to_string()],
        vec!["cash_rounding".into(), s.rounding.to_string()],
        vec!["refunds".into(), s.refunds.to_string()],
        vec!["vat_in_sales".into(), s.vat_sales.to_string()],
        vec!["vat_in_refunds".into(), s.vat_refunds.to_string()],
        vec!["net_sales".into(), s.net.to_string()],
        vec![
            "profit_covered_revenue_ex_vat".into(),
            s.profit.covered_revenue.to_string(),
        ],
        vec!["cost_of_goods".into(), s.profit.cost_of_goods.to_string()],
        vec![
            "estimated_gross_profit".into(),
            s.profit.gross_profit.to_string(),
        ],
        vec![
            "profit_unknown_cost_revenue_ex_vat".into(),
            s.profit.unknown_revenue.to_string(),
        ],
        vec![
            "cost_known_sold_units".into(),
            s.profit.known_sold_units.to_string(),
        ],
        vec![
            "cost_unknown_sold_units".into(),
            s.profit.unknown_sold_units.to_string(),
        ],
        vec![
            "cost_known_refunded_units".into(),
            s.profit.known_refunded_units.to_string(),
        ],
        vec![
            "cost_unknown_refunded_units".into(),
            s.profit.unknown_refunded_units.to_string(),
        ],
        vec!["sale_count".into(), s.sale_count.to_string()],
        vec!["void_count".into(), s.void_count.to_string()],
        vec!["restocked_units".into(), s.restocked_units.to_string()],
    ];
    for method in [
        pos_domain::PaymentMethod::Cash,
        pos_domain::PaymentMethod::Transfer,
        pos_domain::PaymentMethod::Card,
    ] {
        let m = s.payments.method(method);
        rows.push(vec![format!("{}_net", method.key()), m.net().to_string()]);
    }
    rows.push(Vec::new());
    rows.push(
        ["date", "gross_sales", "refunds"]
            .map(String::from)
            .to_vec(),
    );
    for d in &s.days {
        rows.push(vec![
            d.date.clone(),
            d.gross.to_string(),
            d.refunds.to_string(),
        ]);
    }
    rows.push(Vec::new());
    rows.push(
        ["category", "net_units", "net_sales"]
            .map(String::from)
            .to_vec(),
    );
    for c in &s.categories {
        rows.push(vec![
            c.category.clone(),
            c.net_quantity.to_string(),
            c.net_sales.to_string(),
        ]);
    }
    rows.push(Vec::new());
    rows.push(vec!["hour".into(), "gross_sales".into()]);
    for (hour, amount) in s.hourly.iter().enumerate() {
        rows.push(vec![format!("{hour:02}:00"), amount.to_string()]);
    }
    rows.push(Vec::new());
    rows.push(
        ["rank", "sku", "name", "net_units", "net_sales"]
            .map(String::from)
            .to_vec(),
    );
    for (i, p) in s.top_products.iter().enumerate() {
        rows.push(vec![
            (i + 1).to_string(),
            p.sku.clone(),
            p.name.clone(),
            p.net_quantity.to_string(),
            p.net_sales.to_string(),
        ]);
    }
    write_csv(path, &rows)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn home_date_stays_today_when_report_selects_another_day()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut pos = Pos::open(dir.path().join("test.sqlite3"))?;
        let today = today();
        let yesterday = today.previous_day().ok_or("date underflow")?;
        let data = load(
            &mut pos,
            &Query {
                products: ProductQuery::default(),
                sale_offset: 0,
                report_start: yesterday,
                report_end: yesterday,
                shift_offset: 0,
                movement_shift: None,
                movement_offset: 0,
                stock_product: None,
            },
        )?;
        assert_eq!(data.summary.date, yesterday.to_string());
        assert_eq!(data.today_summary.date, today.to_string());
        Ok(())
    }
    #[test]
    fn csv_quotes_and_neutralises_formulas() {
        assert_eq!(cell("plain"), "plain");
        assert_eq!(cell("a,b"), "\"a,b\"");
        assert_eq!(cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(cell("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(cell("-5.00"), "-5.00");
        assert_eq!(cell("-1+1"), "'-1+1");
    }
    #[test]
    fn recovery_backup_flushes_cleared_and_current_bills() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let mut pos = Pos::open(dir.path().join("pos-heart.sqlite3"))?;
        pos.seed_demo()?;
        let product = pos.catalog(&ProductQuery::default())?.remove(0);
        let mut old = Cart::new();
        old.lines.push(pos_domain::CartLine {
            product_id: product.id,
            quantity: 1,
            ..Default::default()
        });
        pos.hold(&old, 1)?;
        std::fs::write(dir.path().join("current-bill.txt"), old.id.to_string())?;
        let before = pos.held()?.len();
        let path = dir.path().join("cleared.posbackup");
        run(
            &mut pos,
            Command::RecoveryBackup(
                dir.path().to_path_buf(),
                path.clone(),
                Cart::new(),
                Some(old.id),
            ),
        )?;
        assert_eq!(
            crate::backup::inspect(&path)?.manifest.info.held as usize,
            before - 1
        );
        assert!(!path.join("current-bill.txt").exists());
        old.lines[0].quantity = 3;
        let path = dir.path().join("current.posbackup");
        run(
            &mut pos,
            Command::RecoveryBackup(dir.path().to_path_buf(), path.clone(), old.clone(), None),
        )?;
        let mut restored = Pos::open(path.join("pos-heart.sqlite3"))?;
        assert_eq!(restored.resume(old.id)?.0.lines[0].quantity, 3);
        assert_eq!(
            std::fs::read_to_string(path.join("current-bill.txt"))?,
            old.id.to_string()
        );
        Ok(())
    }
    #[test]
    fn recovery_authorization_precedes_cart_changes() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut pos = Pos::open(dir.path().join("pos-heart.sqlite3"))?;
        pos.seed_demo()?;
        let product = pos.catalog(&ProductQuery::default())?.remove(0);
        let mut cart = Cart::new();
        cart.lines.push(pos_domain::CartLine {
            product_id: product.id,
            quantity: 1,
            ..Default::default()
        });
        pos.hold(&cart, 1)?;
        std::fs::write(dir.path().join("current-bill.txt"), cart.id.to_string())?;
        pos.save_staff(
            &pos_domain::StaffInput {
                id: None,
                name: "Owner".into(),
                role: Some("owner".into()),
                pin: Some("9999".into()),
                active: true,
            },
            2,
        )?;
        pos.sign_out();
        let path = dir.path().join("denied.posbackup");
        assert!(matches!(
            run(
                &mut pos,
                Command::RecoveryBackup(
                    dir.path().to_path_buf(),
                    path.clone(),
                    Cart::new(),
                    Some(cart.id)
                )
            ),
            Err(Error::SignInRequired)
        ));
        assert!(!path.exists());
        assert!(pos.resume(cart.id).is_ok());
        assert!(dir.path().join("current-bill.txt").exists());
        Ok(())
    }
    #[test]
    fn product_export_writes_every_product_once() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let mut pos = Pos::open(dir.path().join("test.sqlite3"))?;
        pos.seed_demo()?;
        let path = dir.path().join("products.csv");
        export_products(&mut pos, &path)?;
        let text = std::fs::read_to_string(&path)?;
        assert_eq!(
            text.lines().count(),
            1 + pos.catalog_counts()?.total as usize
        );
        assert!(text.contains("ข้าวกะเพราไก่ไข่ดาว"));
        // Never overwrites an existing export.
        assert!(export_products(&mut pos, &path).is_err());
        Ok(())
    }
}
