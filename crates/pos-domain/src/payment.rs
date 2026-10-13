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
#[derive(Debug, Clone, Copy)]
pub struct PaymentRequest {
    pub method: PaymentMethod,
    pub amount: Money,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MethodSummary {
    pub sales: Money,
    pub refunds: Money,
}
impl MethodSummary {
    pub fn net(self) -> SignedMoney {
        SignedMoney::from_satang(self.sales.satang() - self.refunds.satang())
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
