//! Stock movements: receiving, removing (damage, expiry, own use) and counting.
//! Each movement records the stock before and after, so the history explains
//! every manual change. Sales and refunds remain recorded on receipts.
use crate::DomainError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockKind {
    /// Stock tracking started (new product or tracking switched on).
    Initial,
    /// Goods received: adds units.
    Receive,
    /// Damaged, expired or used: removes units; needs a reason.
    Remove,
    /// Physical count: sets the stock to what was counted.
    Count,
}
impl StockKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Receive => "receive",
            Self::Remove => "remove",
            Self::Count => "count",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "initial" => Self::Initial,
            "receive" => Self::Receive,
            "remove" => Self::Remove,
            "count" => Self::Count,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockMovement {
    pub id: Uuid,
    pub product_id: Uuid,
    pub created_at: i64,
    pub kind: StockKind,
    pub before: u32,
    pub after: u32,
    pub reason: String,
    pub staff: crate::Attribution,
}
impl StockMovement {
    pub fn delta(&self) -> i64 {
        i64::from(self.after) - i64::from(self.before)
    }
}
/// An operator's stock change. `id` makes a lost response safe to retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockRequest {
    pub id: Uuid,
    pub product_id: Uuid,
    pub kind: StockKind,
    /// Units received or removed, or the counted total.
    pub quantity: u32,
    /// For counts: the stock shown when counting started. A sale in between
    /// refuses the count instead of silently overwriting it.
    pub expected: Option<u32>,
    pub reason: String,
}
impl StockRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.reason.chars().count() > 240 {
            return Err(DomainError::TextTooLong);
        }
        match self.kind {
            StockKind::Initial => Err(DomainError::InvalidStockChange),
            StockKind::Receive if self.quantity == 0 => Err(DomainError::InvalidStockChange),
            StockKind::Remove if self.quantity == 0 || self.reason.trim().is_empty() => {
                Err(DomainError::InvalidStockChange)
            }
            _ => Ok(()),
        }
    }
    /// Stock after applying this request to `before`, with checked arithmetic.
    pub fn after(&self, before: u32) -> Result<u32, DomainError> {
        match self.kind {
            StockKind::Receive => before
                .checked_add(self.quantity)
                .ok_or(DomainError::Overflow),
            StockKind::Remove => before
                .checked_sub(self.quantity)
                .ok_or(DomainError::InvalidStockChange),
            StockKind::Count => Ok(self.quantity),
            StockKind::Initial => Err(DomainError::InvalidStockChange),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(kind: StockKind, quantity: u32, reason: &str) -> StockRequest {
        StockRequest {
            id: Uuid::nil(),
            product_id: Uuid::nil(),
            kind,
            quantity,
            expected: None,
            reason: reason.into(),
        }
    }
    #[test]
    fn movements_apply_with_checked_arithmetic() {
        assert_eq!(request(StockKind::Receive, 12, "").after(3), Ok(15));
        assert_eq!(request(StockKind::Remove, 2, "แตก").after(3), Ok(1));
        assert_eq!(request(StockKind::Count, 7, "").after(3), Ok(7));
        assert_eq!(
            request(StockKind::Remove, 4, "แตก").after(3),
            Err(DomainError::InvalidStockChange)
        );
        assert_eq!(
            request(StockKind::Receive, 1, "").after(u32::MAX),
            Err(DomainError::Overflow)
        );
    }
    #[test]
    fn invalid_requests_are_refused() {
        assert!(request(StockKind::Receive, 0, "").validate().is_err());
        assert!(request(StockKind::Remove, 1, "  ").validate().is_err());
        assert!(request(StockKind::Initial, 1, "").validate().is_err());
        assert!(request(StockKind::Count, 0, "").validate().is_ok());
        assert!(
            request(StockKind::Remove, 1, &"x".repeat(241))
                .validate()
                .is_err()
        );
    }
}
