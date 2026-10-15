//! The shop's business protocol: serializable requests and replies and one
//! dispatcher over [`Pos`]. The desktop worker calls [`handle`] in-process;
//! the planned shop-hub service (docs/multi-device.md) will call the same
//! function for paired devices, so every device runs identical code paths.
//!
//! Device-local work (backups, recovery, files, pictures on disk, the
//! automatic-backup policy, demo seeding) stays in the desktop worker.
use pos_core::{ActivityQuery, CatalogCounts, Pos, ProductQuery, Result};
use pos_domain::{
    Cart, DailySummary, Money, PaymentRequest, Product, ProductInput, SaleRecord, ShiftCount,
    VoidRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Bumped whenever a request or reply changes shape; peers with another
/// version are refused (docs/multi-device.md, phase 1).
pub const PROTOCOL_VERSION: u32 = 1;

/// Bangkok dates `start..=end` for reports (at most 366 days).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Period {
    pub start: time::Date,
    pub end: time::Date,
}
impl Period {
    pub fn day(date: time::Date) -> Self {
        Self {
            start: date,
            end: date,
        }
    }
    pub fn single(self) -> bool {
        self.start == self.end
    }
    pub fn days(self) -> i64 {
        (self.end - self.start).whole_days() + 1
    }
    /// The comparison period: same weekday last week for one day, otherwise
    /// the equally long period right before.
    pub fn previous(self) -> Option<Self> {
        let back = if self.single() { 7 } else { self.days() };
        Some(Self {
            start: self.start.checked_sub(time::Duration::days(back))?,
            end: self.end.checked_sub(time::Duration::days(back))?,
        })
    }
}

/// The Bangkok calendar date of a Unix time.
pub fn bangkok_date(unix: i64) -> time::Date {
    time::OffsetDateTime::from_unix_timestamp(unix)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .to_offset(time::macros::offset!(+7))
        .date()
}

/// What a screen refresh needs from the shop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    pub products: ProductQuery,
    pub sale_offset: u32,
    pub report: Period,
    pub shift_offset: u32,
    pub movement_shift: Option<Uuid>,
    pub movement_offset: u32,
    pub stock_product: Option<Uuid>,
}

/// One consistent snapshot for the screens.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
    pub promotions: Vec<pos_domain::Promotion>,
    pub ingredients: Vec<pos_domain::Ingredient>,
    pub option_sets: Vec<(pos_domain::OptionGroup, u64)>,
    /// Open kitchen tickets oldest first, then recently bumped ones.
    pub kitchen: Vec<pos_domain::KitchenTicket>,
}

/// A business request. Writes carry their own retry identity (cart ID,
/// request UUID) exactly as the core API expects.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Refresh(Box<Query>),
    SaveProduct(Box<ProductInput>),
    Archive(Uuid),
    SetOnSale(Vec<Uuid>, bool),
    SetCategory(Vec<Uuid>, String),
    AdjustStock(pos_domain::StockRequest),
    Pay(Box<Cart>, PaymentRequest, Uuid),
    PaySplit(Box<Cart>, pos_domain::SplitPayment, Uuid),
    SplitBill(Box<Cart>, Vec<(usize, u32)>, Uuid),
    Void(VoidRequest, Uuid),
    RefundItems(pos_domain::RefundRequest, Uuid),
    OpenShift(Uuid, Money),
    MoveCash(pos_domain::CashMovement),
    CloseShift(Uuid, u64, ShiftCount),
    Hold(Box<Cart>),
    RemoveHeld(Uuid),
    Resume(Uuid),
    Lookup(String),
    ImportProducts(String),
    Activity(ActivityQuery),
    SignIn(Uuid, String),
    SignOut,
    SaveStaff(pos_domain::StaffInput),
    SaveRole(pos_domain::RoleDef),
    DeleteRole(String),
    SetTax(pos_domain::TaxSettings),
    SetPricing(pos_domain::PricingSettings),
    SaveTable(pos_domain::DiningTable),
    SavePromotion(Box<pos_domain::Promotion>),
    DeletePromotion(Uuid),
    SaveIngredient(pos_domain::Ingredient),
    AdjustIngredient(pos_domain::IngredientRequest),
    IngredientHistory(Uuid),
    LoadPurchases(u32),
    PurchaseDetail(Uuid, u32),
    PurchaseProducts(String),
    SaveSupplier(pos_domain::Supplier),
    CreatePurchase(pos_domain::PurchaseOrderRequest),
    ReceivePurchase(pos_domain::ReceivePurchaseRequest),
    CancelPurchase(pos_domain::CancelPurchaseRequest),
    AddCategory(String),
    RenameCategory(String, String),
    ReorderCategories(Vec<String>),
    DeleteCategory(String, Option<String>),
    SaveOptionSet(pos_domain::OptionGroup),
    DeleteOptionSet(Uuid),
    LinkOptionSet(Uuid, Vec<Uuid>, bool),
    ProductImage(Uuid),
    SetProductImage(Uuid, Option<Vec<u8>>),
    SendKitchen(Box<Cart>),
    BumpTicket(Uuid, Option<i64>),
    TicketLine(Uuid, usize, bool),
    IssueInvoice(Uuid, pos_domain::Buyer),
    /// Runs `inner` once with an approver's PIN for a restricted action.
    Approved {
        approver: Uuid,
        pin: String,
        permission: pos_domain::Permission,
        inner: Box<Request>,
    },
}

/// The result of a [`Request`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Reply {
    Data(Box<Data>),
    ProductSaved(Box<Product>),
    Archived,
    Bulk(usize),
    StockAdjusted(pos_domain::StockMovement),
    Paid(Box<pos_domain::Sale>),
    /// The rest of the bill and the split-off bill.
    Split(Box<Cart>, Box<Cart>),
    Voided(Box<pos_domain::SaleVoid>),
    Refunded(Box<pos_domain::SaleRefund>),
    ShiftOpened,
    CashMoved,
    ShiftClosed(Box<pos_domain::Shift>),
    Held(Uuid),
    Removed,
    Resumed(Box<Cart>, Vec<Product>),
    Found(String, Option<Box<Product>>),
    ProductsImported(usize),
    Activity(Vec<pos_core::ActivityEvent>),
    SignedIn(pos_domain::StaffRef),
    SignedOut,
    StaffSaved(pos_domain::Staff),
    RoleSaved,
    TaxSaved,
    PricingSaved,
    TableSaved,
    PromotionSaved,
    IngredientSaved,
    IngredientHistory(Uuid, Vec<pos_domain::IngredientMovement>),
    Purchases {
        suppliers: Vec<pos_domain::Supplier>,
        orders: Vec<pos_domain::PurchaseOrder>,
        offset: u32,
    },
    PurchaseDetail {
        order: Box<pos_domain::PurchaseOrder>,
        receipts: Vec<pos_domain::PurchaseReceipt>,
        offset: u32,
    },
    PurchaseProducts(String, Vec<Product>),
    SupplierSaved(pos_domain::Supplier),
    PurchaseCreated(Box<pos_domain::PurchaseOrder>),
    PurchaseReceived(Box<pos_domain::PurchaseOrder>),
    PurchaseCancelled(Box<pos_domain::PurchaseOrder>),
    CategoriesSaved,
    OptionSetsSaved,
    ProductImage(Uuid, Option<(i64, Vec<u8>)>),
    ImageSaved,
    KitchenSent(Box<Cart>, Option<Box<pos_domain::KitchenTicket>>),
    Kitchen(Vec<pos_domain::KitchenTicket>),
    InvoiceIssued(Box<pos_domain::TaxInvoice>),
}

/// Runs one request at `now` (Unix seconds). Approval covers only the
/// wrapped request and is cleared afterwards, whatever the outcome.
pub fn handle(pos: &mut Pos, request: Request, now: i64) -> Result<Reply> {
    Ok(match request {
        Request::Refresh(query) => Reply::Data(Box::new(load(pos, &query, now)?)),
        Request::SaveProduct(input) => {
            Reply::ProductSaved(Box::new(pos.save_product_at(*input, now)?))
        }
        Request::Archive(id) => {
            pos.archive_product(id)?;
            Reply::Archived
        }
        Request::SetOnSale(ids, on) => Reply::Bulk(pos.set_on_sale(&ids, on)?),
        Request::SetCategory(ids, category) => Reply::Bulk(pos.set_category(&ids, &category)?),
        Request::AdjustStock(request) => Reply::StockAdjusted(pos.adjust_stock(&request, now)?),
        Request::Pay(cart, payment, shift) => Reply::Paid(Box::new(pos.checkout_with_payment(
            &cart,
            payment,
            Some(shift),
            now,
        )?)),
        Request::PaySplit(cart, split, shift) => Reply::Paid(Box::new(pos.checkout_split(
            &cart,
            &split,
            Some(shift),
            now,
        )?)),
        Request::SplitBill(cart, take, id) => {
            let (rest, moved) = pos.split_bill(&cart, &take, id, now)?;
            Reply::Split(Box::new(rest), Box::new(moved))
        }
        Request::Void(request, shift) => Reply::Voided(Box::new(pos.void_sale_for_shift(
            &request,
            Some(shift),
            now,
        )?)),
        Request::RefundItems(request, shift) => {
            Reply::Refunded(Box::new(pos.refund_items(&request, Some(shift), now)?))
        }
        Request::OpenShift(id, opening) => {
            pos.open_shift(id, opening, now)?;
            Reply::ShiftOpened
        }
        Request::MoveCash(mut movement) => {
            movement.created_at = now;
            pos.move_cash(&movement)?;
            Reply::CashMoved
        }
        Request::CloseShift(id, revision, count) => {
            Reply::ShiftClosed(Box::new(pos.close_shift_with(id, revision, &count, now)?))
        }
        Request::Hold(cart) => {
            pos.hold(&cart, now)?;
            Reply::Held(cart.id)
        }
        Request::RemoveHeld(id) => {
            pos.remove_held(id)?;
            Reply::Removed
        }
        Request::Resume(id) => {
            let (cart, products) = pos.resume(id)?;
            Reply::Resumed(Box::new(cart), products)
        }
        Request::Lookup(code) => {
            let found = pos.find_by_code(&code)?.map(Box::new);
            Reply::Found(code, found)
        }
        Request::ImportProducts(source) => Reply::ProductsImported(pos.import_products(&source)?),
        Request::Activity(query) => Reply::Activity(pos.activity(&query)?),
        Request::SignIn(id, pin) => Reply::SignedIn(pos.sign_in(id, &pin)?),
        Request::SignOut => {
            pos.sign_out();
            Reply::SignedOut
        }
        Request::SaveStaff(input) => Reply::StaffSaved(pos.save_staff(&input, now)?),
        Request::SaveRole(role) => {
            pos.save_role(&role)?;
            Reply::RoleSaved
        }
        Request::DeleteRole(id) => {
            pos.delete_role(&id)?;
            Reply::RoleSaved
        }
        Request::SetTax(tax) => {
            pos.set_tax_settings(&tax)?;
            Reply::TaxSaved
        }
        Request::SetPricing(p) => {
            pos.set_pricing_settings(&p)?;
            Reply::PricingSaved
        }
        Request::SaveTable(t) => {
            pos.save_table(&t)?;
            Reply::TableSaved
        }
        Request::SavePromotion(p) => {
            pos.save_promotion(&p, now)?;
            Reply::PromotionSaved
        }
        Request::DeletePromotion(id) => {
            pos.delete_promotion(id)?;
            Reply::PromotionSaved
        }
        Request::SaveIngredient(i) => {
            pos.save_ingredient(&i, now)?;
            Reply::IngredientSaved
        }
        Request::AdjustIngredient(r) => {
            pos.adjust_ingredient(&r, now)?;
            Reply::IngredientSaved
        }
        Request::IngredientHistory(id) => {
            Reply::IngredientHistory(id, pos.ingredient_movements(id, 0)?)
        }
        Request::LoadPurchases(offset) => Reply::Purchases {
            suppliers: pos.suppliers("", true)?,
            orders: pos.purchase_orders(offset)?,
            offset,
        },
        Request::PurchaseDetail(id, offset) => Reply::PurchaseDetail {
            order: Box::new(pos.purchase_order(id)?),
            receipts: pos.purchase_receipts(id, offset)?,
            offset,
        },
        Request::PurchaseProducts(search) => {
            // Purchasing lists stock-tracked products only.
            let products = pos
                .catalog(&ProductQuery {
                    search: search.clone(),
                    limit: 50,
                    ..Default::default()
                })?
                .into_iter()
                .filter(|p| p.stock.is_some())
                .collect();
            Reply::PurchaseProducts(search, products)
        }
        Request::SaveSupplier(supplier) => Reply::SupplierSaved(pos.save_supplier(&supplier, now)?),
        Request::CreatePurchase(request) => {
            Reply::PurchaseCreated(Box::new(pos.create_purchase_order(&request, now)?))
        }
        Request::ReceivePurchase(request) => {
            pos.receive_purchase_order(&request, now)?;
            Reply::PurchaseReceived(Box::new(pos.purchase_order(request.order_id)?))
        }
        Request::CancelPurchase(request) => {
            Reply::PurchaseCancelled(Box::new(pos.cancel_purchase_order(&request, now)?))
        }
        Request::AddCategory(name) => {
            pos.add_category(&name, now)?;
            Reply::CategoriesSaved
        }
        Request::RenameCategory(old, new) => {
            pos.rename_category(&old, &new, now)?;
            Reply::CategoriesSaved
        }
        Request::ReorderCategories(names) => {
            pos.reorder_categories(&names)?;
            Reply::CategoriesSaved
        }
        Request::DeleteCategory(name, into) => {
            pos.delete_category(&name, into.as_deref(), now)?;
            Reply::CategoriesSaved
        }
        Request::SaveOptionSet(set) => {
            pos.save_option_set(&set, now)?;
            Reply::OptionSetsSaved
        }
        Request::DeleteOptionSet(id) => {
            pos.delete_option_set(id, now)?;
            Reply::OptionSetsSaved
        }
        Request::LinkOptionSet(id, products, link) => {
            pos.link_option_set(id, &products, link, now)?;
            Reply::OptionSetsSaved
        }
        Request::ProductImage(id) => Reply::ProductImage(id, pos.product_image(id)?),
        Request::SetProductImage(id, png) => {
            pos.set_product_image(id, png.as_deref(), now)?;
            Reply::ImageSaved
        }
        Request::SendKitchen(cart) => {
            let (cart, ticket) = pos.send_to_kitchen(&cart, now)?;
            Reply::KitchenSent(Box::new(cart), ticket.map(Box::new))
        }
        Request::BumpTicket(id, at) => {
            pos.bump_ticket(id, at)?;
            Reply::Kitchen(pos.kitchen_tickets()?)
        }
        Request::TicketLine(id, line, done) => {
            pos.set_ticket_line(id, line, done)?;
            Reply::Kitchen(pos.kitchen_tickets()?)
        }
        Request::IssueInvoice(sale, buyer) => {
            Reply::InvoiceIssued(Box::new(pos.issue_tax_invoice(sale, &buyer, now)?))
        }
        Request::Approved {
            approver,
            pin,
            permission,
            inner,
        } => {
            pos.approve(approver, &pin, permission)?;
            let result = handle(pos, *inner, now);
            // Approval covers this one request only.
            pos.clear_approval();
            result?
        }
    })
}

/// Loads every screen's data in one go.
pub fn load(pos: &mut Pos, q: &Query, now: i64) -> Result<Data> {
    let period = q.report;
    let summary = pos.summary(period.start, period.end)?;
    let compare = match period.previous() {
        Some(p) => pos.summary(p.start, p.end)?,
        None => DailySummary::default(),
    };
    let today = bangkok_date(now);
    let today_summary = if period == Period::day(today) {
        summary.clone()
    } else {
        pos.daily_summary(today)?
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
        promotions: pos.promotions()?,
        ingredients: pos.ingredients()?,
        option_sets: pos.option_sets()?,
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
