use crate::DomainError;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// Nonnegative Thai baht stored as integer satang. All arithmetic is checked.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct Money(i64);

impl Money {
    pub const ZERO: Self = Self(0);
    pub fn from_satang(value: i64) -> Result<Self, DomainError> {
        if value < 0 {
            Err(DomainError::InvalidMoney)
        } else {
            Ok(Self(value))
        }
    }
    pub fn satang(self) -> i64 {
        self.0
    }
    pub fn checked_add(self, other: Self) -> Result<Self, DomainError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(DomainError::Overflow)
    }
    pub fn checked_mul(self, quantity: u32) -> Result<Self, DomainError> {
        self.0
            .checked_mul(i64::from(quantity))
            .map(Self)
            .ok_or(DomainError::Overflow)
    }
    pub fn change_from(self, cash: Self) -> Result<Self, DomainError> {
        if cash < self {
            return Err(DomainError::InsufficientPayment);
        }
        Ok(Self(cash.0 - self.0))
    }
}
impl TryFrom<i64> for Money {
    type Error = DomainError;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::from_satang(value)
    }
}
impl From<Money> for i64 {
    fn from(value: Money) -> Self {
        value.0
    }
}
impl FromStr for Money {
    type Err = DomainError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let mut parts = text.split('.');
        let whole = parts.next().ok_or(DomainError::InvalidMoney)?;
        let fraction = parts.next();
        if parts.next().is_some() || whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(DomainError::InvalidMoney);
        }
        let cents = match fraction {
            None => 0,
            Some(f) if (1..=2).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_digit()) => {
                let n: i64 = f.parse().map_err(|_| DomainError::InvalidMoney)?;
                if f.len() == 1 { n * 10 } else { n }
            }
            _ => return Err(DomainError::InvalidMoney),
        };
        let baht: i64 = whole.parse().map_err(|_| DomainError::Overflow)?;
        baht.checked_mul(100)
            .and_then(|v| v.checked_add(cents))
            .map(Self)
            .ok_or(DomainError::Overflow)
    }
}
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

/// Signed report values only. Payments and refunds continue to use nonnegative Money.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignedMoney(i64);

impl SignedMoney {
    pub fn from_satang(value: i64) -> Self {
        Self(value)
    }
    pub fn satang(self) -> i64 {
        self.0
    }
    pub fn checked_add(self, other: Self) -> Result<Self, DomainError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(DomainError::Overflow)
    }
}
impl fmt::Display for SignedMoney {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:02}", magnitude / 100, magnitude % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_money_has_no_float_rounding() -> Result<(), DomainError> {
        assert_eq!(
            "0.10"
                .parse::<Money>()?
                .checked_add("0.20".parse()?)?
                .to_string(),
            "0.30"
        );
        assert_eq!("1.5".parse::<Money>()?.satang(), 150);
        assert_eq!("0.01".parse::<Money>()?.checked_mul(3)?.satang(), 3);
        Ok(())
    }
    #[test]
    fn rejects_negative_excess_precision_and_overflow() {
        for s in ["-1", "1.001", "NaN", "1e3", "", "1.", "+1", "1,000"] {
            assert!(s.parse::<Money>().is_err(), "accepted {s}");
        }
        assert!("92233720368547758.08".parse::<Money>().is_err());
        assert!(
            Money::from_satang(i64::MAX)
                .and_then(|m| m.checked_mul(2))
                .is_err()
        );
    }
}
