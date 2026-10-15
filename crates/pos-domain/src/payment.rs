use crate::{DomainError, Money, SignedMoney};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    #[default]
    Cash,
    Transfer,
    Card,
}
impl PaymentMethod {
    pub fn key(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Transfer => "transfer",
            Self::Card => "card",
        }
    }
}
/// Amount is cash tender for Cash, or the exact reviewed total for non-cash.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub method: PaymentMethod,
    pub amount: Money,
}
/// One part of a split payment: the amount applied to the bill by a method
/// (for cash, the part of the bill paid in cash, not the notes handed over).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tender {
    pub method: PaymentMethod,
    pub amount: Money,
}
/// Several methods for one bill, e.g. part cash, part transfer. Split bills
/// are priced without cash rounding; the parts must add up to the exact total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitPayment {
    pub tenders: Vec<Tender>,
    /// Notes handed over for the cash part (zero without a cash part).
    pub cash_given: Money,
}
impl SplitPayment {
    /// Parts merged per method in a fixed order (cash, transfer, card), zero
    /// parts dropped. At least two methods are required.
    pub fn normalized(&self) -> Result<Vec<Tender>, DomainError> {
        let mut out: Vec<Tender> = Vec::new();
        for method in [
            PaymentMethod::Cash,
            PaymentMethod::Transfer,
            PaymentMethod::Card,
        ] {
            let mut amount = Money::ZERO;
            for t in self.tenders.iter().filter(|t| t.method == method) {
                amount = amount.checked_add(t.amount)?;
            }
            if amount > Money::ZERO {
                out.push(Tender { method, amount });
            }
        }
        if out.len() < 2 {
            return Err(DomainError::InvalidSplit);
        }
        let cash = out
            .iter()
            .find(|t| t.method == PaymentMethod::Cash)
            .map_or(Money::ZERO, |t| t.amount);
        if self.cash_given < cash || (cash == Money::ZERO && self.cash_given > Money::ZERO) {
            return Err(DomainError::InsufficientPayment);
        }
        Ok(out)
    }
    /// Sum of all parts.
    pub fn total(tenders: &[Tender]) -> Result<Money, DomainError> {
        tenders
            .iter()
            .try_fold(Money::ZERO, |sum, t| sum.checked_add(t.amount))
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MethodSummary {
    pub sales: Money,
    pub refunds: Money,
}
impl MethodSummary {
    pub fn net(self) -> SignedMoney {
        SignedMoney::from_satang(self.sales.satang() - self.refunds.satang())
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PaymentSummary {
    pub cash: MethodSummary,
    pub transfer: MethodSummary,
    pub card: MethodSummary,
}
impl PaymentSummary {
    pub fn method(self, method: PaymentMethod) -> MethodSummary {
        match method {
            PaymentMethod::Cash => self.cash,
            PaymentMethod::Transfer => self.transfer,
            PaymentMethod::Card => self.card,
        }
    }
    /// Every part of a split sale or refund, or the single method.
    pub fn record_all(&mut self, tenders: &[Tender], refund: bool) -> Result<(), DomainError> {
        for t in tenders {
            self.record(t.method, t.amount, refund)?;
        }
        Ok(())
    }
    pub fn record(
        &mut self,
        method: PaymentMethod,
        amount: Money,
        refund: bool,
    ) -> Result<(), DomainError> {
        let bucket = match method {
            PaymentMethod::Cash => &mut self.cash,
            PaymentMethod::Transfer => &mut self.transfer,
            PaymentMethod::Card => &mut self.card,
        };
        if refund {
            bucket.refunds = bucket.refunds.checked_add(amount)?;
        } else {
            bucket.sales = bucket.sales.checked_add(amount)?;
        }
        Ok(())
    }
}
