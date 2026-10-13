use crate::{DomainError, Money, Sale};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct VoidRequest {
    pub sale_id: Uuid,
    pub refund_method: crate::PaymentMethod,
    pub reason: String,
    /// Explicit operator choices for legacy receipts without stock snapshots.
    /// None means the operator has not reviewed legacy stock yet.
    pub legacy_restock: Option<Vec<Uuid>>,
}
impl VoidRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.reason.trim().is_empty() {
            return Err(DomainError::MissingVoidReason);
        }
        if self.reason.chars().count() > 240 {
            return Err(DomainError::TextTooLong);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockReturn {
    pub product_id: Uuid,
    pub quantity: u32,
}

/// Separate immutable audit record; the original paid receipt is never rewritten.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleVoid {
    pub id: Uuid,
    pub sale_id: Uuid,
    #[serde(default)]
    pub refund_method: crate::PaymentMethod,
    pub created_at: i64,
    pub reason: String,
    pub refund: Money,
    pub restocked: Vec<StockReturn>,
    pub legacy_stock_reviewed: bool,
    /// Who did it and who approved (empty when the shop has no staff).
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
}

#[derive(Debug, Clone)]
pub struct SaleRecord {
    pub sale: Sale,
    pub cancellation: Option<SaleVoid>,
    /// Full tax invoice issued for this sale, if any.
    pub tax_invoice: Option<crate::TaxInvoice>,
    /// Partial refunds, oldest first. Empty when the sale was voided whole.
    pub refunds: Vec<SaleRefund>,
}

/// One receipt line to return: index into `Sale::lines` and units returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundLine {
    pub line: usize,
    pub quantity: u32,
}
/// Return some items of a paid sale. `id` makes a lost response safe to retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundRequest {
    pub id: Uuid,
    pub sale_id: Uuid,
    pub refund_method: crate::PaymentMethod,
    pub reason: String,
    pub lines: Vec<RefundLine>,
    /// For receipts made before stock snapshots: the refunded line indexes
    /// whose units go back to stock. None means not reviewed yet.
    pub legacy_restock: Option<Vec<usize>>,
}
impl RefundRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.reason.trim().is_empty() {
            return Err(DomainError::MissingVoidReason);
        }
        if self.reason.chars().count() > 240 {
            return Err(DomainError::TextTooLong);
        }
        let mut seen = std::collections::HashSet::new();
        if self.lines.is_empty()
            || self
                .lines
                .iter()
                .any(|l| l.quantity == 0 || !seen.insert(l.line))
        {
            return Err(DomainError::InvalidQuantity);
        }
        Ok(())
    }
}
/// Immutable audit record of a partial refund; the receipt is never rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundedLine {
    pub line: usize,
    pub product_id: Uuid,
    pub name: String,
    pub quantity: u32,
    /// Unit price (options included) × quantity.
    pub amount: Money,
    /// VAT-exclusive revenue reversed, snapshotted at refund time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revenue_ex_vat: Option<Money>,
    /// Units returned to stock (0 for untracked food).
    pub restocked: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleRefund {
    pub id: Uuid,
    pub sale_id: Uuid,
    pub refund_method: crate::PaymentMethod,
    pub created_at: i64,
    pub reason: String,
    pub refund: Money,
    pub lines: Vec<RefundedLine>,
    /// Who did it and who approved (empty when the shop has no staff).
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
}
impl SaleRecord {
    /// Units of each receipt line already returned by partial refunds.
    pub fn refunded_units(&self) -> Vec<u32> {
        let mut out = vec![0_u32; self.sale.lines.len()];
        for r in &self.refunds {
            for l in &r.lines {
                if let Some(n) = out.get_mut(l.line) {
                    *n = n.saturating_add(l.quantity);
                }
            }
        }
        out
    }
    /// Total returned so far, whether by a full void or partial refunds.
    pub fn refunded_total(&self) -> Money {
        if let Some(v) = &self.cancellation {
            return v.refund;
        }
        self.refunds
            .iter()
            .try_fold(Money::ZERO, |sum, r| sum.checked_add(r.refund))
            .unwrap_or(self.sale.total)
    }
    /// Every unit has been returned (full void or partial refunds).
    pub fn fully_refunded(&self) -> bool {
        self.cancellation.is_some()
            || (!self.refunds.is_empty()
                && self
                    .refunded_units()
                    .iter()
                    .zip(&self.sale.lines)
                    .all(|(r, l)| *r >= l.quantity))
    }
}
