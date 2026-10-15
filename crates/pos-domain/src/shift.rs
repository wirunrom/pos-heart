use crate::{DomainError, Money, SignedMoney};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Drawer totals are distinct from calendar-day sales reports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shift {
    pub id: Uuid,
    pub opened_at: i64,
    pub last_activity_at: i64,
    pub opening: Money,
    /// Cash sales/refunds only; legacy field names retained for immutable snapshots.
    pub sales: Money,
    #[serde(default)]
    pub transfer_sales: Money,
    #[serde(default)]
    pub card_sales: Money,
    #[serde(default)]
    pub transfer_refunds: Money,
    #[serde(default)]
    pub card_refunds: Money,
    pub refunds: Money,
    pub cash_in: Money,
    pub cash_out: Money,
    pub sale_count: u64,
    pub refund_count: u64,
    pub revision: u64,
    pub closing: Option<ShiftClosing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_by: Option<crate::StaffRef>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShiftClosing {
    pub closed_at: i64,
    pub expected: SignedMoney,
    pub counted: Money,
    /// Counted minus expected: negative is a shortage, positive is an overage.
    pub difference: SignedMoney,
    /// Required when the count does not balance. Absent on older snapshots.
    #[serde(default)]
    pub reason: String,
    /// Optional denomination breakdown that adds up to `counted`.
    #[serde(default)]
    pub denominations: Vec<CashCount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_by: Option<crate::StaffRef>,
}
/// One denomination line of a drawer count, e.g. 9 × ฿1,000.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CashCount {
    pub value: Money,
    pub count: u32,
}
/// What the operator confirms when closing a shift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftCount {
    pub counted: Money,
    pub reason: String,
    pub denominations: Vec<CashCount>,
}
impl ShiftCount {
    pub fn total(denominations: &[CashCount]) -> Result<Money, DomainError> {
        denominations.iter().try_fold(Money::ZERO, |sum, d| {
            sum.checked_add(d.value.checked_mul(d.count)?)
        })
    }
    pub fn validate(&self, difference: SignedMoney) -> Result<(), DomainError> {
        if self.reason.chars().count() > 240 {
            return Err(DomainError::TextTooLong);
        }
        if difference.satang() != 0 && self.reason.trim().is_empty() {
            return Err(DomainError::MissingDifferenceReason);
        }
        if !self.denominations.is_empty() && Self::total(&self.denominations)? != self.counted {
            return Err(DomainError::CountMismatch);
        }
        Ok(())
    }
}
impl Shift {
    pub fn expected(&self) -> Result<SignedMoney, DomainError> {
        let value = i128::from(self.opening.satang()) + i128::from(self.sales.satang())
            - i128::from(self.refunds.satang())
            + i128::from(self.cash_in.satang())
            - i128::from(self.cash_out.satang());
        Ok(SignedMoney::from_satang(
            i64::try_from(value).map_err(|_| DomainError::Overflow)?,
        ))
    }
    pub fn difference(&self, counted: Money) -> Result<SignedMoney, DomainError> {
        Ok(SignedMoney::from_satang(
            counted
                .satang()
                .checked_sub(self.expected()?.satang())
                .ok_or(DomainError::Overflow)?,
        ))
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CashKind {
    Sale,
    Refund,
    CashIn,
    CashOut,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CashMovement {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub created_at: i64,
    pub kind: CashKind,
    pub amount: Money,
    pub reason: String,
    #[serde(default)]
    pub payment_method: crate::PaymentMethod,
    /// Who did it and who approved (empty when the shop has no staff).
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
}
