//! POS business values and validation. No UI, filesystem, or database dependencies.
mod dining;
mod discount;
mod ingredient;
mod money;
mod options;
mod payment;
mod pricing;
mod promotion;
mod promptpay;
mod reporting;
mod sale;
mod shift;
mod staff;
mod stock;
mod tax;
mod void;

pub use dining::{DiningTable, KitchenTicket, SentItem, TicketLine, kitchen_changes};
pub use discount::{BillPrice, Discount, LinePrice, price_bill, price_sale, refund_share};
pub use ingredient::{
    Ingredient, IngredientKind, IngredientMovement, IngredientRequest, MAX_RECIPE, MILLI,
    OptionRecipe, RecipeItem, ingredient_use, line_recipe, validate_option_recipe, validate_recipe,
};
pub use money::{Money, SignedMoney};
pub use options::{
    LineOption, OptionChoice, OptionGroup, SaleOption, price_with_options, validate_groups,
};
pub use payment::{
    MethodSummary, PaymentMethod, PaymentRequest, PaymentSummary, SplitPayment, Tender,
};
pub use pricing::{CashRounding, PriceOptions, PricingSettings, RoundMode, percent_of};
pub use promotion::{
    ALL_DAYS, LocalTime, PromoLine, PromoRule, PromoTarget, PromoUse, Promoted, Promotion,
    apply_promotions, same_code,
};
pub use promptpay::{PromptPayId, PromptPayKind};
pub use reporting::{
    CategorySales, DailySummary, DaySales, IngredientUsage, ProductMovement, ProfitSummary,
    PromoSales, StaffSales,
};
pub use sale::{Cart, CartLine, LOW_STOCK, Product, ProductInput, Sale, SaleLine, split_cart};
pub use shift::{CashCount, CashKind, CashMovement, Shift, ShiftClosing, ShiftCount};
pub use staff::{
    Access, Attribution, CASHIER, MANAGER, OWNER, Permission, RoleDef, Staff, StaffInput, StaffRef,
    discount_share_bp, valid_pin,
};
pub use stock::{StockKind, StockMovement, StockRequest};
pub use tax::{
    Buyer, CreditLine, CreditNote, DEFAULT_VAT_BP, SaleTax, TaxInvoice, TaxSettings, valid_branch,
    valid_tax_id, vat_included,
};
pub use void::{
    RefundLine, RefundRequest, RefundedLine, SaleRecord, SaleRefund, SaleVoid, StockReturn,
    VoidRequest,
};

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("จำนวนเงินไม่ถูกต้อง กรุณาใส่เลขไม่ติดลบและทศนิยมไม่เกิน 2 ตำแหน่ง")]
    InvalidMoney,
    #[error("จำนวนเงินหรือจำนวนสินค้าเกินขอบเขตที่รองรับ")]
    Overflow,
    #[error("กรุณาระบุรหัสสินค้าและชื่อสินค้า")]
    MissingProductFields,
    #[error("ข้อมูลยาวเกินกำหนด (รหัส 64, ชื่อ 160, หมวดหมู่ 80, หมายเหตุ 240 ตัวอักษร)")]
    TextTooLong,
    #[error("กรุณาเลือกสินค้าอย่างน้อยหนึ่งรายการ")]
    EmptyCart,
    #[error("จำนวนสินค้าต้องมากกว่า 0 และไม่เกิน 999,999")]
    InvalidQuantity,
    #[error("เงินที่รับน้อยกว่ายอดที่ต้องชำระ")]
    InsufficientPayment,
    #[error("กรุณาระบุเหตุผลยกเลิกบิล")]
    MissingVoidReason,
    #[error("กรุณาระบุเหตุผลของส่วนต่างเงินสด")]
    MissingDifferenceReason,
    #[error("ยอดนับแยกตามชนิดเงินไม่ตรงกับยอดนับรวม")]
    CountMismatch,
    #[error("ตัวเลือกสินค้าไม่ถูกต้อง")]
    InvalidOptions,
    #[error("จำนวนสต็อกไม่ถูกต้อง")]
    InvalidStockChange,
    #[error("พร้อมเพย์ต้องเป็นเบอร์มือถือ 10 หลัก เลขประจำตัว 13 หลัก หรือ e-Wallet 15 หลัก")]
    InvalidPromptPay,
    #[error("ส่วนลดไม่ถูกต้อง: ใส่จำนวนเงินมากกว่า 0 หรือ 0.01–100%")]
    InvalidDiscount,
    #[error("ส่วนลดมากกว่าราคา")]
    DiscountTooLarge,
    #[error("ชื่อพนักงานต้องมี 1–60 ตัวอักษร และต้องเลือกบทบาท")]
    InvalidStaff,
    #[error("PIN ต้องเป็นตัวเลข 4–8 หลัก")]
    InvalidPin,
    #[error("ข้อมูลภาษีไม่ครบหรือไม่ถูกต้อง")]
    InvalidTaxSettings,
    #[error("ข้อมูลผู้ซื้อไม่ครบหรือเลขประจำตัวผู้เสียภาษีไม่ถูกต้อง")]
    InvalidBuyer,
    #[error("ตั้งค่าราคาไม่ถูกต้อง: service charge 0–30% และปัดเศษ 0, 0.25, 0.50 หรือ 1 บาท")]
    InvalidPricing,
    #[error("บทบาทต้องมีชื่อ 1–40 ตัวอักษร และให้สิทธิ์จัดการพนักงานได้เฉพาะเจ้าของร้าน")]
    InvalidRole,
    #[error("โต๊ะต้องมีชื่อ 1–20 ตัวอักษร โซนไม่เกิน 40 ตัวอักษร และที่นั่งไม่เกิน 99")]
    InvalidTable,
    #[error("แบ่งจ่ายต้องใช้อย่างน้อย 2 วิธี และยอดรวมต้องเท่ากับยอดชำระพอดี")]
    InvalidSplit,
    #[error("แยกบิลต้องเลือกอย่างน้อยหนึ่งรายการ และเหลือในบิลเดิมอย่างน้อยหนึ่งรายการ")]
    InvalidBillSplit,
    #[error("โปรโมชันไม่ถูกต้อง: ตรวจชื่อ เงื่อนไข วัน เวลา และคูปอง")]
    InvalidPromotion,
    #[error("วัตถุดิบหรือสูตรไม่ถูกต้อง: ต้องมีชื่อและหน่วย ปริมาณมากกว่า 0 และไม่ซ้ำกัน")]
    InvalidIngredient,
}
