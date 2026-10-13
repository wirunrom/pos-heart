//! Transactional application interface. Only this crate knows SQLite.
mod audit;
mod catalog;
mod checkout;
mod database;
mod dining;
mod import;
mod ingredient;
mod promotion;
mod recovery;
mod refund;
mod reporting;
mod schema;
mod shifts;
mod staff;
mod stock;
mod tax;
mod void;

pub use audit::{ActivityEvent, ActivityKind, ActivityQuery};
pub use catalog::{CatalogCounts, ProductQuery, ProductView};
pub use database::{Pos, SCHEMA_VERSION};
pub use import::{ImportIssue, ImportPreview, ImportRow, MAX_IMPORT_BYTES, MAX_IMPORT_ROWS};
use pos_domain::DomainError;
pub use recovery::{BackupInfo, inspect_database};
pub use reporting::MAX_REPORT_DAYS;
pub use staff::DEFAULT_DISCOUNT_LIMIT_BP;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("Database connection error: {0}")]
    Connection(#[from] diesel::ConnectionError),
    #[error("Data error: {0}")]
    Data(#[from] serde_json::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Product code already exists")]
    DuplicateSku,
    #[error("Barcode already belongs to another product")]
    DuplicateBarcode,
    #[error("Product is not for sale: {0}")]
    NotForSale(String),
    #[error("Product no longer exists or is inactive")]
    ProductUnavailable,
    #[error("Insufficient stock: {0}")]
    InsufficientStock(String),
    #[error("This bill has already been paid")]
    AlreadyPaid,
    #[error("Held bill no longer exists")]
    CartNotFound,
    #[error("Unsupported database version: {0}")]
    UnsupportedSchema(i64),
    #[error("Database contains invalid values")]
    CorruptData,
    #[error("Paid sale not found")]
    SaleNotFound,
    #[error("Review legacy stock before voiding this receipt")]
    LegacyStockReviewRequired,
    #[error("Stock tracking changed for {0}; enable it before returning these units")]
    StockTrackingChanged(String),
    #[error("Invalid legacy stock selection")]
    InvalidStockSelection,
    #[error("Cancellation time cannot precede payment")]
    InvalidVoidTime,
    #[error("Total changed; review the updated bill before confirming payment")]
    PaymentTotalChanged,
    #[error("Refund must use the original payment method")]
    InvalidRefundMethod,
    #[error("Refunds by this method would exceed what it paid")]
    RefundExceedsMethod,
    #[error("Another promotion already uses this coupon code")]
    DuplicateCoupon,
    #[error("Ingredient not found or switched off")]
    IngredientNotFound,
    #[error("Another ingredient already has this name")]
    DuplicateIngredient,
    #[error("Open a shift before taking or refunding payment")]
    NoOpenShift,
    #[error("A shift is already open")]
    ShiftAlreadyOpen,
    #[error("Shift changed; refresh and review the current totals")]
    ShiftChanged,
    #[error("Shift is closed or does not exist")]
    ShiftClosed,
    #[error("Cash movement needs a positive amount and a reason of at most 240 characters")]
    InvalidCashMovement,
    #[error("Request identity was already used for different data")]
    ConflictingRequest,
    #[error("Check the computer clock; shift time precedes recorded activity")]
    InvalidShiftTime,
    #[error("Report date is outside the supported range")]
    InvalidReportDate,
    #[error("Stock changed since the count started; count again")]
    StockChanged,
    #[error("This product does not track stock")]
    StockNotTracked,
    #[error("This sale was already voided")]
    SaleVoided,
    #[error("Items of this sale were already refunded; refund the remaining items instead")]
    PartiallyRefunded,
    #[error("Refund exceeds the units sold on this receipt")]
    RefundExceedsSale,
    #[error("Sign in with your PIN first")]
    SignInRequired,
    #[error("A manager must approve this: {0:?}")]
    ApprovalRequired(pos_domain::Permission),
    #[error("Wrong PIN")]
    WrongPin,
    #[error("Too many wrong PINs; try again in {0} s")]
    PinLocked(u64),
    #[error("Staff member not found or inactive")]
    StaffNotFound,
    #[error("Active staff must include at least one active owner")]
    LastOwner,
    #[error("This role may not do this: {0:?}")]
    Forbidden(pos_domain::Permission),
    #[error("Role not found")]
    RoleNotFound,
    #[error("Staff still hold this role")]
    RoleInUse,
    #[error("Table not found or switched off")]
    TableNotFound,
    #[error("This table already has an open bill")]
    TableBusy,
    #[error("A table with this name already exists in the zone")]
    DuplicateTable,
    #[error("Kitchen ticket not found")]
    TicketNotFound,
    #[error("This receipt was paid before VAT was set up")]
    NotTaxReceipt,
    #[error("This sale was refunded; issue a credit note instead")]
    SaleRefunded,
}
