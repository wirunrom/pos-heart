use crate::{Money, SignedMoney};
use uuid::Uuid;

#[derive(Debug, Default, Clone)]
pub struct DailySummary {
    /// First Bangkok calendar date, YYYY-MM-DD. Activity uses
    /// [first midnight, midnight after `end_date`).
    pub date: String,
    /// Last Bangkok date included (equal to `date` for a single day).
    pub end_date: String,
    /// Gross sales and refunds per day of the period, oldest first.
    pub days: Vec<DaySales>,
    /// Net units and sales per product category (current catalog category).
    pub categories: Vec<CategorySales>,
    pub payments: crate::PaymentSummary,
    pub sale_count: u64,
    /// Whole sales voided on this period's days.
    pub void_count: u64,
    /// Partial refunds processed on this period's days.
    pub refund_count: u64,
    /// Amounts paid on sales (after discounts).
    pub gross_sales: Money,
    /// Line and bill discounts given on this period's sales.
    pub discounts: Money,
    pub refunds: Money,
    pub cash_received: Money,
    pub change_given: Money,
    pub net: SignedMoney,
    /// Gross sales by Bangkok hour of payment across the period (index 0 = 00:00–00:59).
    pub hourly: [Money; 24],
    /// Units returned to stock by refunds processed on this day.
    pub restocked_units: u64,
    /// Top ten by net units, then net amount. Same-day refunds reduce sold units.
    pub top_products: Vec<ProductMovement>,
    /// Service charge collected on this period's sales.
    pub service_charges: Money,
    /// Net cash rounding on this period's sales (negative = rounded down).
    pub rounding: SignedMoney,
    /// VAT contained in sales and in refunds of VAT receipts.
    pub vat_sales: Money,
    pub vat_refunds: Money,
    /// Sales and refunds by who rang them up (empty without staff).
    pub staff: Vec<StaffSales>,
    pub profit: ProfitSummary,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaffSales {
    pub staff_id: uuid::Uuid,
    pub name: String,
    pub sale_count: u64,
    pub sales: Money,
    pub refunds: Money,
}

#[derive(Debug, Clone)]
pub struct ProductMovement {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub net_quantity: i64,
    pub net_sales: SignedMoney,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaySales {
    pub date: String,
    pub gross: Money,
    pub refunds: Money,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorySales {
    pub category: String,
    pub net_quantity: i64,
    pub net_sales: SignedMoney,
}

/// Standard-cost estimate, not net business profit or inventory valuation.
/// Unknown-cost lines never silently count as free and never enter gross profit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfitSummary {
    pub covered_revenue: SignedMoney,
    pub cost_of_goods: SignedMoney,
    pub gross_profit: SignedMoney,
    pub unknown_revenue: SignedMoney,
    pub known_sold_units: u64,
    pub unknown_sold_units: u64,
    pub known_refunded_units: u64,
    pub unknown_refunded_units: u64,
}
impl ProfitSummary {
    pub fn record(
        &mut self,
        revenue: Money,
        cost: Option<Money>,
        quantity: u32,
        refund: bool,
    ) -> Result<(), crate::DomainError> {
        let direction = if refund { -1 } else { 1 };
        let revenue = SignedMoney::from_satang(revenue.satang() * direction);
        let units = match (cost.is_some(), refund) {
            (true, false) => &mut self.known_sold_units,
            (false, false) => &mut self.unknown_sold_units,
            (true, true) => &mut self.known_refunded_units,
            (false, true) => &mut self.unknown_refunded_units,
        };
        *units = units
            .checked_add(u64::from(quantity))
            .ok_or(crate::DomainError::Overflow)?;
        if let Some(cost) = cost {
            let cost = cost.checked_mul(quantity)?;
            self.covered_revenue = self.covered_revenue.checked_add(revenue)?;
            self.cost_of_goods = self
                .cost_of_goods
                .checked_add(SignedMoney::from_satang(cost.satang() * direction))?;
            self.gross_profit = SignedMoney::from_satang(
                self.covered_revenue
                    .satang()
                    .checked_sub(self.cost_of_goods.satang())
                    .ok_or(crate::DomainError::Overflow)?,
            );
        } else {
            self.unknown_revenue = self.unknown_revenue.checked_add(revenue)?;
        }
        Ok(())
    }
}
