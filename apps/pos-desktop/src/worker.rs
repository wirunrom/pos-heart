use eframe::egui;
use pos_core::{Error, Pos, ProductQuery};
use pos_domain::{
    Cart, Money, PaymentRequest, Product, ProductInput, Sale, SaleVoid, ShiftCount, VoidRequest,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};
use uuid::Uuid;

pub use pos_service::{Data, Query};
#[derive(Clone)]
pub enum Command {
    Refresh(Query),
    Save(ProductInput),
    Archive(Uuid),
    SetOnSale(Vec<Uuid>, bool),
    SetCategory(Vec<Uuid>, String),
    AdjustStock(pos_domain::StockRequest),
    Pay(Cart, PaymentRequest, Uuid),
    /// One bill paid by several methods.
    PaySplit(Cart, pos_domain::SplitPayment, Uuid),
    /// Move (line, units) of a bill to a new bill with this ID.
    SplitBill(Cart, Vec<(usize, u32)>, Uuid),
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
    SaveAutoBackup(PathBuf, crate::auto_backup::Settings),
    RetryAutoBackup(PathBuf),
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
    RevokeSession,
    SaveStaff(pos_domain::StaffInput),
    SaveRole(pos_domain::RoleDef),
    DeleteRole(String),
    SetTax(pos_domain::TaxSettings),
    SetPricing(pos_domain::PricingSettings),
    SaveTable(pos_domain::DiningTable),
    SavePromotion(pos_domain::Promotion),
    SaveIngredient(pos_domain::Ingredient),
    LoadPurchases(u32),
    PurchaseDetail(Uuid, u32),
    PurchaseProducts(String),
    SaveSupplier(pos_domain::Supplier),
    CreatePurchase(pos_domain::PurchaseOrderRequest),
    ReceivePurchase(pos_domain::ReceivePurchaseRequest),
    CancelPurchase(pos_domain::CancelPurchaseRequest),
    AddCategory(String),
    SaveOptionSet(pos_domain::OptionGroup),
    DeleteOptionSet(Uuid),
    LinkOptionSet(Uuid, Vec<Uuid>, bool),
    RenameCategory(String, String),
    ReorderCategories(Vec<String>),
    DeleteCategory(String, Option<String>),
    /// Background: a product picture for the cache.
    ProductImage(Uuid),
    SetProductImage(Uuid, Option<Vec<u8>>),
    AdjustIngredient(pos_domain::IngredientRequest),
    IngredientHistory(Uuid),
    DeletePromotion(Uuid),
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
                | Self::RevokeSession
        )
    }
}
pub enum Event {
    Purchases {
        suppliers: Vec<pos_domain::Supplier>,
        orders: Vec<pos_domain::PurchaseOrder>,
        offset: u32,
    },
    PurchaseDetail {
        order: pos_domain::PurchaseOrder,
        receipts: Vec<pos_domain::PurchaseReceipt>,
        offset: u32,
    },
    PurchaseProducts(String, Vec<Product>),
    SupplierSaved(pos_domain::Supplier),
    PurchaseCreated(pos_domain::PurchaseOrder),
    PurchaseReceived(pos_domain::PurchaseOrder),
    PurchaseCancelled(pos_domain::PurchaseOrder),
    Ready(Box<Data>),
    ShiftOpened,
    CashMoved,
    ShiftClosed(Box<pos_domain::Shift>),
    Saved(Box<Product>),
    Bulk(usize),
    StockAdjusted(pos_domain::StockMovement),
    Archived,
    Paid(Sale),
    /// The rest of the bill and the split-off bill.
    Split(Cart, Cart),
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
    AutoBackupSaved(crate::auto_backup::Settings),
    AutoBackupResult(Result<crate::auto_backup::Outcome, String>),
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
    PromotionSaved,
    IngredientSaved,
    CategoriesSaved,
    OptionSetsSaved,
    ProductImage(Uuid, Option<(i64, Vec<u8>)>),
    ImageSaved,
    IngredientHistory(Uuid, Vec<pos_domain::IngredientMovement>),
    KitchenSent(Cart, Option<Box<pos_domain::KitchenTicket>>),
    Kitchen(Vec<pos_domain::KitchenTicket>),
    InvoiceIssued(pos_domain::TaxInvoice),
    /// Background command finished (successfully or not); never changes `busy`.
    Quiet,
    Failed(Error),
}
struct CommandEnvelope {
    generation: u64,
    command: Command,
}
pub struct EventEnvelope {
    pub generation: u64,
    pub background: bool,
    pub event: Event,
}
pub struct Worker {
    tx: SyncSender<CommandEnvelope>,
    pub rx: Receiver<EventEnvelope>,
    epoch: Arc<AtomicU64>,
}
/// Revocation is observed before every command, independently of queue capacity.
/// A command already executing may finish; its result keeps the old generation.
fn execute(
    pos: &mut Pos,
    envelope: CommandEnvelope,
    epoch: &AtomicU64,
    observed: &mut u64,
) -> EventEnvelope {
    let current = epoch.load(Ordering::SeqCst);
    if *observed != current {
        pos.sign_out();
        *observed = current;
    }
    let background = envelope.command.background();
    let safe_draft = matches!(
        &envelope.command,
        Command::Autosave(_) | Command::Discard(_) | Command::Restore(_)
    );
    let event = if envelope.generation != current && !safe_draft {
        if background {
            Event::Quiet
        } else {
            Event::Failed(Error::SignInRequired)
        }
    } else {
        match run(pos, envelope.command) {
            Err(_) if background => Event::Quiet,
            other => other.unwrap_or_else(Event::Failed),
        }
    };
    // In-flight sign-in must never survive a revocation that arrived during it.
    let after = epoch.load(Ordering::SeqCst);
    if *observed != after {
        pos.sign_out();
        *observed = after;
    }
    EventEnvelope {
        generation: envelope.generation,
        background,
        event,
    }
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
    pub fn generation(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }
    pub fn send(&self, command: Command) -> bool {
        self.tx
            .try_send(CommandEnvelope {
                generation: self.generation(),
                command,
            })
            .is_ok()
    }
    /// Always revokes immediately, even if the queue is full or a write is busy.
    pub fn revoke(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let _ = self.send(Command::RevokeSession);
    }
    pub fn start(path: PathBuf, context: egui::Context) -> std::io::Result<Self> {
        let (tx, commands) = mpsc::sync_channel::<CommandEnvelope>(16);
        let (events, rx) = mpsc::channel();
        let epoch = Arc::new(AtomicU64::new(0));
        let worker_epoch = Arc::clone(&epoch);
        thread::Builder::new()
            .name("pos-database".into())
            .spawn(move || {
                let send = |envelope: EventEnvelope| {
                    let _ = events.send(envelope);
                    context.request_repaint();
                };
                let data_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                let mut pos = match Pos::open(path) {
                    Ok(pos) => pos,
                    Err(e) => {
                        send(EventEnvelope {
                            generation: 0,
                            background: false,
                            event: Event::Failed(e),
                        });
                        return;
                    }
                };
                let initial = Query {
                    products: ProductQuery::default(),
                    sale_offset: 0,
                    report: pos_service::Period::day(today()),
                    shift_offset: 0,
                    movement_shift: None,
                    movement_offset: 0,
                    stock_product: None,
                };
                send(EventEnvelope {
                    generation: 0,
                    background: false,
                    event: match pos_service::load(&mut pos, &initial, now()) {
                        Ok(data) => Event::Ready(Box::new(data)),
                        Err(e) => Event::Failed(e),
                    },
                });
                let mut observed = 0;
                let mut next_backup = std::time::Instant::now();
                let mut last_backup_error = None;
                loop {
                    if std::time::Instant::now() >= next_backup {
                        send_automatic(
                            &mut pos,
                            &data_dir,
                            crate::auto_backup::Trigger::Daily,
                            &worker_epoch,
                            &mut last_backup_error,
                            &send,
                        );
                        next_backup =
                            std::time::Instant::now() + std::time::Duration::from_secs(60);
                    }
                    let wait = next_backup.saturating_duration_since(std::time::Instant::now());
                    let envelope = match commands.recv_timeout(wait) {
                        Ok(envelope) => envelope,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let result = execute(&mut pos, envelope, &worker_epoch, &mut observed);
                    let stopping = matches!(&result.event, Event::RecoveryPending(_));
                    let closed = match &result.event {
                        Event::ShiftClosed(shift) => Some(shift.id),
                        _ => None,
                    };
                    send(result);
                    if stopping {
                        break;
                    }
                    if let Some(id) = closed {
                        send_automatic(
                            &mut pos,
                            &data_dir,
                            crate::auto_backup::Trigger::ShiftClosed(id),
                            &worker_epoch,
                            &mut last_backup_error,
                            &send,
                        );
                    }
                }
            })?;
        Ok(Self { tx, rx, epoch })
    }
}

fn send_automatic(
    pos: &mut Pos,
    data: &Path,
    trigger: crate::auto_backup::Trigger,
    epoch: &AtomicU64,
    last_error: &mut Option<String>,
    send: &impl Fn(EventEnvelope),
) {
    let result = crate::auto_backup::run(pos, data, trigger, now()).map_err(|e| e.to_string());
    let notify = match &result {
        Err(error) => last_error.as_ref() != Some(error),
        Ok(outcome) => {
            outcome.latest.is_some()
                || outcome.created.is_some()
                || outcome.warning.is_some()
                || last_error.is_some()
        }
    };
    *last_error = result.as_ref().err().cloned();
    if notify {
        send(EventEnvelope {
            generation: epoch.load(Ordering::SeqCst),
            background: true,
            event: Event::AutoBackupResult(result),
        });
    }
}

/// How a business reply maps back to the screens' events.
enum Shape {
    Normal,
    /// Background saves report nothing.
    Quiet,
    /// The startup restore of the current bill.
    Restored,
}
/// Splits a command into a business request (and how to report it) or
/// returns it unchanged when it is device-local.
fn request(command: Command) -> Result<(pos_service::Request, Shape), Box<Command>> {
    use pos_service::Request as R;
    let normal = |r| Ok((r, Shape::Normal));
    match command {
        Command::Refresh(q) => normal(R::Refresh(Box::new(q))),
        Command::Save(input) => normal(R::SaveProduct(Box::new(input))),
        Command::Archive(id) => normal(R::Archive(id)),
        Command::SetOnSale(ids, on) => normal(R::SetOnSale(ids, on)),
        Command::SetCategory(ids, c) => normal(R::SetCategory(ids, c)),
        Command::AdjustStock(r) => normal(R::AdjustStock(r)),
        Command::Pay(cart, p, shift) => normal(R::Pay(Box::new(cart), p, shift)),
        Command::PaySplit(cart, p, shift) => normal(R::PaySplit(Box::new(cart), p, shift)),
        Command::SplitBill(cart, take, id) => normal(R::SplitBill(Box::new(cart), take, id)),
        Command::Void(r, shift) => normal(R::Void(r, shift)),
        Command::RefundItems(r, shift) => normal(R::RefundItems(r, shift)),
        Command::OpenShift(id, m) => normal(R::OpenShift(id, m)),
        Command::MoveCash(m) => normal(R::MoveCash(m)),
        Command::CloseShift(id, rev, count) => normal(R::CloseShift(id, rev, count)),
        Command::Hold(cart) => normal(R::Hold(Box::new(cart))),
        Command::RemoveHeld(id) => normal(R::RemoveHeld(id)),
        Command::Resume(id) => normal(R::Resume(id)),
        Command::Lookup(code) => normal(R::Lookup(code)),
        Command::ImportProducts(source) => normal(R::ImportProducts(source)),
        Command::Activity(q) => normal(R::Activity(q)),
        Command::SignIn(id, pin) => normal(R::SignIn(id, pin)),
        Command::RevokeSession => normal(R::SignOut),
        Command::SaveStaff(i) => normal(R::SaveStaff(i)),
        Command::SaveRole(r) => normal(R::SaveRole(r)),
        Command::DeleteRole(id) => normal(R::DeleteRole(id)),
        Command::SetTax(t) => normal(R::SetTax(t)),
        Command::SetPricing(p) => normal(R::SetPricing(p)),
        Command::SaveTable(t) => normal(R::SaveTable(t)),
        Command::SavePromotion(p) => normal(R::SavePromotion(Box::new(p))),
        Command::DeletePromotion(id) => normal(R::DeletePromotion(id)),
        Command::SaveIngredient(i) => normal(R::SaveIngredient(i)),
        Command::AdjustIngredient(r) => normal(R::AdjustIngredient(r)),
        Command::IngredientHistory(id) => normal(R::IngredientHistory(id)),
        Command::LoadPurchases(o) => normal(R::LoadPurchases(o)),
        Command::PurchaseDetail(id, o) => normal(R::PurchaseDetail(id, o)),
        Command::PurchaseProducts(s) => normal(R::PurchaseProducts(s)),
        Command::SaveSupplier(s) => normal(R::SaveSupplier(s)),
        Command::CreatePurchase(r) => normal(R::CreatePurchase(r)),
        Command::ReceivePurchase(r) => normal(R::ReceivePurchase(r)),
        Command::CancelPurchase(r) => normal(R::CancelPurchase(r)),
        Command::AddCategory(n) => normal(R::AddCategory(n)),
        Command::RenameCategory(a, b) => normal(R::RenameCategory(a, b)),
        Command::ReorderCategories(n) => normal(R::ReorderCategories(n)),
        Command::DeleteCategory(n, into) => normal(R::DeleteCategory(n, into)),
        Command::SaveOptionSet(s) => normal(R::SaveOptionSet(s)),
        Command::DeleteOptionSet(id) => normal(R::DeleteOptionSet(id)),
        Command::LinkOptionSet(id, p, l) => normal(R::LinkOptionSet(id, p, l)),
        Command::ProductImage(id) => normal(R::ProductImage(id)),
        Command::SetProductImage(id, png) => normal(R::SetProductImage(id, png)),
        Command::SendKitchen(cart) => normal(R::SendKitchen(Box::new(cart))),
        Command::BumpTicket(id, at) => normal(R::BumpTicket(id, at)),
        Command::TicketLine(id, line, done) => normal(R::TicketLine(id, line, done)),
        Command::IssueInvoice(id, buyer) => normal(R::IssueInvoice(id, buyer)),
        Command::Autosave(cart) => Ok((R::Hold(Box::new(cart)), Shape::Quiet)),
        Command::Discard(id) => Ok((R::RemoveHeld(id), Shape::Quiet)),
        Command::Restore(id) => Ok((R::Resume(id), Shape::Restored)),
        Command::Approved {
            approver,
            pin,
            permission,
            inner,
        } => match request(*inner) {
            Ok((inner, shape)) => Ok((
                R::Approved {
                    approver,
                    pin,
                    permission,
                    inner: Box::new(inner),
                },
                shape,
            )),
            Err(local) => Err(Box::new(Command::Approved {
                approver,
                pin,
                permission,
                inner: local,
            })),
        },
        local => Err(Box::new(local)),
    }
}
/// The screens' event for a business reply.
fn event(reply: pos_service::Reply, shape: Shape) -> Event {
    use pos_service::Reply as R;
    match (shape, reply) {
        (Shape::Quiet, _) => Event::Quiet,
        (Shape::Restored, R::Resumed(cart, products)) => Event::Restored(*cart, products),
        (_, R::Data(d)) => Event::Ready(d),
        (_, R::ProductSaved(p)) => Event::Saved(p),
        (_, R::Archived) => Event::Archived,
        (_, R::Bulk(n)) => Event::Bulk(n),
        (_, R::StockAdjusted(m)) => Event::StockAdjusted(m),
        (_, R::Paid(sale)) => Event::Paid(*sale),
        (_, R::Split(rest, moved)) => Event::Split(*rest, *moved),
        (_, R::Voided(v)) => Event::Voided(*v),
        (_, R::Refunded(r)) => Event::Refunded(*r),
        (_, R::ShiftOpened) => Event::ShiftOpened,
        (_, R::CashMoved) => Event::CashMoved,
        (_, R::ShiftClosed(s)) => Event::ShiftClosed(s),
        (_, R::Held(id)) => Event::Held(id),
        (_, R::Removed) => Event::Removed,
        (_, R::Resumed(cart, products)) => Event::Resumed(*cart, products),
        (_, R::Found(code, p)) => Event::Found(code, p),
        (_, R::ProductsImported(n)) => Event::ProductsImported(n),
        (_, R::Activity(a)) => Event::Activity(a),
        (_, R::SignedIn(s)) => Event::SignedIn(s),
        (_, R::SignedOut) => Event::SignedOut,
        (_, R::StaffSaved(s)) => Event::StaffSaved(s),
        (_, R::RoleSaved) => Event::RoleSaved,
        (_, R::TaxSaved) => Event::TaxSaved,
        (_, R::PricingSaved) => Event::PricingSaved,
        (_, R::TableSaved) => Event::TableSaved,
        (_, R::PromotionSaved) => Event::PromotionSaved,
        (_, R::IngredientSaved) => Event::IngredientSaved,
        (_, R::IngredientHistory(id, m)) => Event::IngredientHistory(id, m),
        (
            _,
            R::Purchases {
                suppliers,
                orders,
                offset,
            },
        ) => Event::Purchases {
            suppliers,
            orders,
            offset,
        },
        (
            _,
            R::PurchaseDetail {
                order,
                receipts,
                offset,
            },
        ) => Event::PurchaseDetail {
            order: *order,
            receipts,
            offset,
        },
        (_, R::PurchaseProducts(s, p)) => Event::PurchaseProducts(s, p),
        (_, R::SupplierSaved(s)) => Event::SupplierSaved(s),
        (_, R::PurchaseCreated(o)) => Event::PurchaseCreated(*o),
        (_, R::PurchaseReceived(o)) => Event::PurchaseReceived(*o),
        (_, R::PurchaseCancelled(o)) => Event::PurchaseCancelled(*o),
        (_, R::CategoriesSaved) => Event::CategoriesSaved,
        (_, R::OptionSetsSaved) => Event::OptionSetsSaved,
        (_, R::ProductImage(id, png)) => Event::ProductImage(id, png),
        (_, R::ImageSaved) => Event::ImageSaved,
        (_, R::KitchenSent(cart, ticket)) => Event::KitchenSent(*cart, ticket),
        (_, R::Kitchen(k)) => Event::Kitchen(k),
        (_, R::InvoiceIssued(i)) => Event::InvoiceIssued(*i),
        // Registers are managed from Settings → Devices (multi-device phase 1).
        (_, R::RegisterAdded(_)) => Event::Quiet,
    }
}
fn run(pos: &mut Pos, command: Command) -> pos_core::Result<Event> {
    // Business commands run through the shared protocol (pos-service), the
    // same path a paired device will use; the rest is device-local.
    let command = match request(command) {
        Ok((request, shape)) => return Ok(event(pos_service::handle(pos, request, now())?, shape)),
        Err(local) => *local,
    };
    Ok(match command {
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
        Command::SaveAutoBackup(data, settings) => {
            pos.authorize_recovery()?;
            settings.save(&data)?;
            Event::AutoBackupSaved(settings)
        }
        Command::RetryAutoBackup(data) => {
            pos.authorize_recovery()?;
            Event::AutoBackupResult(
                crate::auto_backup::run(
                    pos,
                    &data,
                    crate::auto_backup::Trigger::ManualRetry,
                    now(),
                )
                .map_err(|e| e.to_string()),
            )
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
        Command::ExportProducts(path) => {
            export_products(pos, &path)?;
            Event::Exported(path)
        }
        Command::ExportReport(path, start, end) => {
            export_report(pos, &path, start, end)?;
            Event::Exported(path)
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
        // `request` already routed every business command to pos-service.
        _ => {
            return Err(std::io::Error::other("business command reached the local worker").into());
        }
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
        vec!["partial_refund_count".into(), s.refund_count.to_string()],
        vec!["cash_received".into(), s.cash_received.to_string()],
        vec!["change_given".into(), s.change_given.to_string()],
        vec!["restocked_units".into(), s.restocked_units.to_string()],
    ];
    for method in [
        pos_domain::PaymentMethod::Cash,
        pos_domain::PaymentMethod::Transfer,
        pos_domain::PaymentMethod::Card,
    ] {
        let m = s.payments.method(method);
        rows.push(vec![format!("{}_sales", method.key()), m.sales.to_string()]);
        rows.push(vec![
            format!("{}_refunds", method.key()),
            m.refunds.to_string(),
        ]);
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
    rows.push(Vec::new());
    rows.push(
        ["staff_id", "staff_name", "sale_count", "sales", "refunds"]
            .map(String::from)
            .to_vec(),
    );
    for staff in &s.staff {
        rows.push(vec![
            staff.staff_id.to_string(),
            staff.name.clone(),
            staff.sale_count.to_string(),
            staff.sales.to_string(),
            staff.refunds.to_string(),
        ]);
    }
    rows.push(Vec::new());
    rows.push(
        [
            "promotion_id",
            "promotion_name",
            "net_bills",
            "net_discount",
        ]
        .map(String::from)
        .to_vec(),
    );
    for promo in &s.promotions {
        rows.push(vec![
            promo.id.to_string(),
            promo.name.clone(),
            promo.bills.to_string(),
            promo.amount.to_string(),
        ]);
    }
    rows.push(Vec::new());
    rows.push(vec![
        "ingredient_cost_basis".into(),
        "Current unit cost estimate; not historical inventory valuation. Blank means unavailable; zero means known zero cost.".into(),
    ]);
    rows.push(
        [
            "ingredient_id",
            "ingredient_name",
            "unit",
            "net_used",
            "manually_removed",
            "estimated_cost_at_current_unit_cost",
        ]
        .map(String::from)
        .to_vec(),
    );
    for ingredient in &s.ingredients {
        rows.push(vec![
            ingredient.ingredient_id.to_string(),
            ingredient.name.clone(),
            ingredient.unit.clone(),
            csv_quantity(ingredient.used),
            csv_quantity(ingredient.removed),
            ingredient.cost.map(|c| c.to_string()).unwrap_or_default(),
        ]);
    }
    write_csv(path, &rows)?;
    Ok(())
}

/// Exact, spreadsheet-friendly thousandths without grouping or float conversion.
fn csv_quantity(milli: i64) -> String {
    let amount = milli.unsigned_abs();
    format!(
        "{}{}.{:03}",
        if milli < 0 { "-" } else { "" },
        amount / 1000,
        amount % 1000
    )
}

#[cfg(test)]
#[path = "report_csv_tests.rs"]
mod report_csv_tests;

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
        let data = pos_service::load(
            &mut pos,
            &Query {
                products: ProductQuery::default(),
                sale_offset: 0,
                report: pos_service::Period::day(yesterday),
                shift_offset: 0,
                movement_shift: None,
                movement_offset: 0,
                stock_product: None,
            },
            now(),
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

#[cfg(test)]
#[path = "revocation_tests.rs"]
mod revocation_tests;
