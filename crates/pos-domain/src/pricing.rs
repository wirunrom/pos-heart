//! Shop-wide pricing rules beyond discounts: service charge and cash rounding.
//! VAT (inclusive or exclusive prices) comes from the tax settings.
use crate::{DomainError, Money};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundMode {
    /// Always in the customer's favour (default).
    #[default]
    Down,
    /// To the nearest step, halves up.
    Nearest,
}
/// Rounding of cash totals to coins in use, e.g. 25 satang or 1 baht.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashRounding {
    /// 0 (off), 25, 50 or 100 satang.
    pub step: u32,
    pub mode: RoundMode,
}
impl CashRounding {
    pub fn enabled(&self) -> bool {
        self.step > 0
    }
    /// Satang to add to `total` (negative when rounding down).
    pub fn adjustment(&self, total: Money) -> i64 {
        let step = i64::from(self.step);
        if step <= 1 {
            return 0;
        }
        let t = total.satang();
        let rounded = match self.mode {
            RoundMode::Down => t - t % step,
            RoundMode::Nearest => (2 * t + step) / (2 * step) * step,
        };
        rounded - t
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingSettings {
    /// Service charge in basis points (1000 = 10%); 0 = off.
    #[serde(default)]
    pub service_bp: u32,
    #[serde(default)]
    pub rounding: CashRounding,
}
impl PricingSettings {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.service_bp > 3_000 || ![0, 25, 50, 100].contains(&self.rounding.step) {
            return Err(DomainError::InvalidPricing);
        }
        Ok(())
    }
}
/// What applies to one bill being priced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PriceOptions {
    pub service_bp: u32,
    /// VAT rate added on top when prices exclude VAT.
    pub vat_exclusive_bp: Option<u32>,
    /// Cash rounding, for cash payments only.
    pub rounding: Option<CashRounding>,
}
impl PriceOptions {
    /// Options for a bill: service only when the bill takes it, VAT on top
    /// only for exclusive pricing, rounding only for cash.
    pub fn for_bill(
        pricing: &PricingSettings,
        tax: &crate::TaxSettings,
        service_on: bool,
        cash: bool,
    ) -> Self {
        Self {
            service_bp: if service_on { pricing.service_bp } else { 0 },
            vat_exclusive_bp: (tax.enabled && !tax.prices_include_vat).then_some(tax.rate_bp),
            rounding: (cash && pricing.rounding.enabled()).then_some(pricing.rounding),
        }
    }
}
/// `amount × bp / 10000`, rounded half up to the satang.
pub fn percent_of(amount: Money, bp: u32) -> Result<Money, DomainError> {
    let v = (i128::from(amount.satang()) * i128::from(bp) + 5_000) / 10_000;
    Money::from_satang(i64::try_from(v).map_err(|_| DomainError::Overflow)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Discount, price_sale};
    fn m(s: &str) -> Money {
        s.parse().unwrap_or_default()
    }
    #[test]
    fn rounding_steps_and_modes() {
        let down = |step| CashRounding {
            step,
            mode: RoundMode::Down,
        };
        let near = |step| CashRounding {
            step,
            mode: RoundMode::Nearest,
        };
        assert_eq!(down(25).adjustment(m("358.37")), -12);
        assert_eq!(down(100).adjustment(m("358.37")), -37);
        assert_eq!(near(100).adjustment(m("358.50")), 50);
        assert_eq!(near(100).adjustment(m("358.49")), -49);
        assert_eq!(near(25).adjustment(m("10.13")), 12);
        assert_eq!(near(25).adjustment(m("10.12")), -12);
        assert_eq!(down(0).adjustment(m("1.01")), 0);
    }
    #[test]
    fn service_vat_and_rounding_spread_exactly() -> Result<(), DomainError> {
        // 100 (with 10% off) + 45 exempt; bill 10% service on eligible,
        // VAT 7% on top, cash to whole baht down.
        let options = PriceOptions {
            service_bp: 1_000,
            vat_exclusive_bp: Some(700),
            rounding: Some(CashRounding {
                step: 100,
                mode: RoundMode::Down,
            }),
        };
        let p = price_sale(
            &[
                (m("100"), Some(Discount::Percent(1_000)), true),
                (m("45"), None, false),
            ],
            None,
            &options,
        )?;
        assert_eq!(p.service_charge, m("9")); // 10% of 90 only
        assert_eq!(p.vat_added, m("10.08")); // 7% of 144
        assert_eq!(p.rounding, -8); // 154.08 → 154.00
        assert_eq!(p.total, m("154"));
        let nets: i64 = p.lines.iter().map(|l| l.net().satang()).sum();
        assert_eq!(nets, p.total.satang());
        assert_eq!(p.lines[1].service, Money::ZERO);
        Ok(())
    }
    #[test]
    fn settings_are_validated() {
        let mut s = PricingSettings {
            service_bp: 1_000,
            ..Default::default()
        };
        assert!(s.validate().is_ok());
        s.rounding.step = 10;
        assert_eq!(s.validate(), Err(DomainError::InvalidPricing));
    }
}
