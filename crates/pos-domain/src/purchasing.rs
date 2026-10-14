//! Purchase quantities are thousandths; product quantities must be whole units.
use crate::{Attribution, DomainError, MILLI, Money};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Supplier {
    pub id: Uuid,
    pub name: String,
    pub contact: String,
    pub note: String,
    pub active: bool,
}
impl Supplier {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.id.is_nil()
            || self.name.trim().is_empty()
            || self.name.chars().count() > 160
            || self.contact.chars().count() > 240
            || self.note.chars().count() > 240
        {
            return Err(DomainError::InvalidPurchase);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseTarget {
    Product(Uuid),
    Ingredient(Uuid),
}
impl PurchaseTarget {
    pub fn validate_quantity(self, q: i64) -> Result<(), DomainError> {
        let id = match self {
            Self::Product(id) | Self::Ingredient(id) => id,
        };
        if id.is_nil()
            || q <= 0
            || matches!(self, Self::Product(_))
                && (q % MILLI != 0 || q / MILLI > i64::from(u32::MAX))
        {
            Err(DomainError::InvalidPurchase)
        } else {
            Ok(())
        }
    }
}
/// Cost per whole unit, rounded half up for each receiving line.
pub fn purchase_cost(cost: Money, quantity: i64) -> Result<Money, DomainError> {
    if quantity < 0 {
        return Err(DomainError::InvalidPurchase);
    }
    let n = i128::from(cost.satang())
        .checked_mul(i128::from(quantity))
        .and_then(|n| n.checked_add(i128::from(MILLI / 2)))
        .ok_or(DomainError::Overflow)?
        / i128::from(MILLI);
    Money::from_satang(i64::try_from(n).map_err(|_| DomainError::Overflow)?)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseItemInput {
    pub target: PurchaseTarget,
    pub quantity: i64,
    pub unit_cost: Money,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseOrderRequest {
    pub id: Uuid,
    pub supplier_id: Uuid,
    pub items: Vec<PurchaseItemInput>,
    pub note: String,
}
impl PurchaseOrderRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.id.is_nil()
            || self.supplier_id.is_nil()
            || self.items.is_empty()
            || self.items.len() > 100
            || self.note.chars().count() > 240
        {
            return Err(DomainError::InvalidPurchase);
        }
        let mut seen = std::collections::HashSet::new();
        let mut total = Money::ZERO;
        for i in &self.items {
            i.target.validate_quantity(i.quantity)?;
            if !seen.insert(i.target) {
                return Err(DomainError::InvalidPurchase);
            }
            total = total.checked_add(purchase_cost(i.unit_cost, i.quantity)?)?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseLine {
    pub target: PurchaseTarget,
    pub name: String,
    pub unit: String,
    pub quantity: i64,
    pub received: i64,
    pub unit_cost: Money,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PurchaseStatus {
    Open,
    Partial,
    Received,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseOrder {
    pub id: Uuid,
    pub supplier: Supplier,
    pub items: Vec<PurchaseLine>,
    pub note: String,
    pub created_at: i64,
    pub revision: i64,
    pub cancelled: bool,
    pub received_cost: Money,
    pub cancellation: Option<PurchaseCancellation>,
    pub staff: Attribution,
}
impl PurchaseOrder {
    pub fn status(&self) -> PurchaseStatus {
        if self.cancelled {
            PurchaseStatus::Cancelled
        } else if self.items.iter().all(|i| i.quantity == i.received) {
            PurchaseStatus::Received
        } else if self.items.iter().any(|i| i.received > 0) {
            PurchaseStatus::Partial
        } else {
            PurchaseStatus::Open
        }
    }
    pub fn total(&self) -> Result<Money, DomainError> {
        self.items.iter().try_fold(Money::ZERO, |a, i| {
            a.checked_add(purchase_cost(i.unit_cost, i.quantity)?)
        })
    }
    pub fn received_total(&self) -> Result<Money, DomainError> {
        Ok(self.received_cost)
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        self.supplier.validate()?;
        PurchaseOrderRequest {
            id: self.id,
            supplier_id: self.supplier.id,
            items: self
                .items
                .iter()
                .map(|i| PurchaseItemInput {
                    target: i.target,
                    quantity: i.quantity,
                    unit_cost: i.unit_cost,
                })
                .collect(),
            note: self.note.clone(),
        }
        .validate()?;
        if self.cancelled != self.cancellation.is_some() {
            return Err(DomainError::InvalidPurchase);
        }
        if let Some(c) = &self.cancellation {
            c.request.validate()?;
            if c.request.order_id != self.id
                || c.request.revision.checked_add(1) != Some(self.revision)
            {
                return Err(DomainError::InvalidPurchase);
            }
        }
        if self.revision < 0
            || self.items.iter().any(|i| {
                i.name.trim().is_empty()
                    || i.unit.trim().is_empty()
                    || i.received < 0
                    || i.received > i.quantity
                    || matches!(i.target, PurchaseTarget::Product(_)) && i.received % MILLI != 0
            })
        {
            return Err(DomainError::InvalidPurchase);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseReceiveItem {
    pub line: usize,
    pub quantity: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceivePurchaseRequest {
    pub id: Uuid,
    pub order_id: Uuid,
    pub revision: i64,
    pub items: Vec<PurchaseReceiveItem>,
    pub note: String,
}
impl ReceivePurchaseRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        let mut seen = std::collections::HashSet::new();
        if self.id.is_nil()
            || self.order_id.is_nil()
            || self.revision < 0
            || self.items.is_empty()
            || self.items.len() > 100
            || self.note.chars().count() > 240
            || self
                .items
                .iter()
                .any(|i| i.line >= 100 || i.quantity <= 0 || !seen.insert(i.line))
        {
            return Err(DomainError::InvalidPurchase);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseReceivedLine {
    pub received_before: i64,
    pub line: usize,
    pub target: PurchaseTarget,
    pub name: String,
    pub unit: String,
    pub quantity: i64,
    pub unit_cost: Money,
    pub total: Money,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseReceipt {
    pub id: Uuid,
    pub order_id: Uuid,
    pub request: ReceivePurchaseRequest,
    pub created_at: i64,
    pub lines: Vec<PurchaseReceivedLine>,
    pub total: Money,
    pub staff: Attribution,
}
impl PurchaseReceipt {
    pub fn validate(&self) -> Result<(), DomainError> {
        self.request.validate()?;
        if self.id != self.request.id
            || self.order_id != self.request.order_id
            || self.lines.len() != self.request.items.len()
        {
            return Err(DomainError::InvalidPurchase);
        }
        let mut total = Money::ZERO;
        for (l, r) in self.lines.iter().zip(&self.request.items) {
            l.target.validate_quantity(l.quantity)?;
            if l.line != r.line
                || l.quantity != r.quantity
                || l.name.trim().is_empty()
                || l.unit.trim().is_empty()
                || l.received_before < 0
                || l.total != purchase_receive_cost(l.unit_cost, l.received_before, l.quantity)?
            {
                return Err(DomainError::InvalidPurchase);
            }
            total = total.checked_add(l.total)?;
        }
        if total != self.total {
            return Err(DomainError::InvalidPurchase);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelPurchaseRequest {
    pub id: Uuid,
    pub order_id: Uuid,
    pub revision: i64,
    pub reason: String,
}
impl CancelPurchaseRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.id.is_nil()
            || self.order_id.is_nil()
            || self.revision < 0
            || self.reason.trim().is_empty()
            || self.reason.chars().count() > 240
        {
            Err(DomainError::InvalidPurchase)
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseCancellation {
    pub request: CancelPurchaseRequest,
    pub created_at: i64,
    pub staff: Attribution,
}

/// Allocate fractional costs against cumulative received quantity; completed
/// receipts therefore sum exactly to the order line's rounded total.
pub fn purchase_receive_cost(
    cost: Money,
    before: i64,
    quantity: i64,
) -> Result<Money, DomainError> {
    if before < 0 || quantity <= 0 {
        return Err(DomainError::InvalidPurchase);
    }
    let after = before.checked_add(quantity).ok_or(DomainError::Overflow)?;
    let total = purchase_cost(cost, after)?
        .satang()
        .checked_sub(purchase_cost(cost, before)?.satang())
        .ok_or(DomainError::Overflow)?;
    Money::from_satang(total)
}
