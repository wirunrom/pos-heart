//! Thai VAT for VAT-registered shops: prices include VAT; receipts become
//! abbreviated tax invoices (ใบกำกับภาษีอย่างย่อ) and a full tax invoice
//! (ใบกำกับภาษีเต็มรูป) can be issued for a sale on request. The seller's
//! details are snapshotted on each receipt, so later edits never rewrite it.
use crate::{DomainError, Money};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Standard Thai VAT rate in basis points (7%).
pub const DEFAULT_VAT_BP: u32 = 700;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxSettings {
    pub enabled: bool,
    /// Registered name of the business.
    pub seller_name: String,
    /// 13-digit taxpayer ID.
    pub tax_id: String,
    /// "00000" for head office, otherwise the 5-digit branch number.
    pub branch: String,
    pub address: String,
    pub rate_bp: u32,
    /// Revenue Department POS registration number, if issued.
    #[serde(default)]
    pub pos_id: String,
    /// Shelf prices include VAT (retail); false adds VAT on top ("++").
    #[serde(default = "yes")]
    pub prices_include_vat: bool,
}
fn yes() -> bool {
    true
}
impl Default for TaxSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            seller_name: String::new(),
            tax_id: String::new(),
            branch: "00000".into(),
            address: String::new(),
            rate_bp: DEFAULT_VAT_BP,
            pos_id: String::new(),
            prices_include_vat: true,
        }
    }
}
/// 13 digits with the Thai mod-11 check digit.
pub fn valid_tax_id(id: &str) -> bool {
    let digits: Vec<u32> = id.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 13 || id.chars().count() != 13 {
        return false;
    }
    let sum: u32 = digits[..12]
        .iter()
        .enumerate()
        .map(|(i, d)| d * (13 - i as u32))
        .sum();
    (11 - sum % 11) % 10 == digits[12]
}
pub fn valid_branch(branch: &str) -> bool {
    branch.len() == 5 && branch.bytes().all(|b| b.is_ascii_digit())
}
impl TaxSettings {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.enabled {
            return Ok(());
        }
        let name = self.seller_name.trim().chars().count();
        if name == 0
            || name > 160
            || !valid_tax_id(&self.tax_id)
            || !valid_branch(&self.branch)
            || self.address.trim().is_empty()
            || self.address.chars().count() > 300
            || !(1..=3_000).contains(&self.rate_bp)
            || self.pos_id.chars().count() > 40
        {
            return Err(DomainError::InvalidTaxSettings);
        }
        Ok(())
    }
}
/// VAT contained in a VAT-inclusive amount, rounded half up to the satang.
pub fn vat_included(amount: Money, rate_bp: u32) -> Money {
    let a = i128::from(amount.satang());
    let r = i128::from(rate_bp);
    let denominator = 10_000 + r;
    let vat = (a * r * 2 + denominator) / (denominator * 2);
    Money::from_satang(i64::try_from(vat).unwrap_or(0)).unwrap_or_default()
}
/// Tax snapshot on a receipt (abbreviated tax invoice).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleTax {
    pub seller_name: String,
    pub tax_id: String,
    pub branch: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pos_id: String,
    pub rate_bp: u32,
    /// Total before VAT.
    pub base: Money,
    pub vat: Money,
}
impl SaleTax {
    pub fn for_total(settings: &TaxSettings, total: Money) -> Self {
        let vat = vat_included(total, settings.rate_bp);
        Self {
            seller_name: settings.seller_name.trim().to_owned(),
            tax_id: settings.tax_id.clone(),
            branch: settings.branch.clone(),
            address: settings.address.trim().to_owned(),
            pos_id: settings.pos_id.trim().to_owned(),
            rate_bp: settings.rate_bp,
            base: Money::from_satang(total.satang() - vat.satang()).unwrap_or_default(),
            vat,
        }
    }
}
/// The buyer on a full tax invoice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Buyer {
    pub name: String,
    pub tax_id: String,
    /// "00000" head office; 5 digits for a branch.
    pub branch: String,
    pub address: String,
}
impl Buyer {
    pub fn validate(&self) -> Result<(), DomainError> {
        let name = self.name.trim().chars().count();
        if name == 0
            || name > 160
            || !valid_tax_id(&self.tax_id)
            || !valid_branch(&self.branch)
            || self.address.trim().is_empty()
            || self.address.chars().count() > 300
        {
            return Err(DomainError::InvalidBuyer);
        }
        Ok(())
    }
}
/// A full tax invoice issued for one sale; numbered in its own sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxInvoice {
    pub id: Uuid,
    pub number: u64,
    pub sale_id: Uuid,
    pub sale_number: u64,
    pub created_at: i64,
    pub buyer: Buyer,
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn with_check(first12: &str) -> String {
        let sum: u32 = first12
            .chars()
            .filter_map(|c| c.to_digit(10))
            .enumerate()
            .map(|(i, d)| d * (13 - i as u32))
            .sum();
        format!("{first12}{}", (11 - sum % 11) % 10)
    }
    #[test]
    fn tax_ids_use_the_check_digit() {
        let id = with_check("010555512345");
        assert!(valid_tax_id(&id));
        let mut wrong = id.clone();
        wrong.pop();
        wrong.push(if id.ends_with('0') { '1' } else { '0' });
        assert!(!valid_tax_id(&wrong));
        assert!(!valid_tax_id("12345"));
        assert!(!valid_tax_id("0-1055-55123-45-6"));
    }
    #[test]
    fn vat_is_extracted_from_inclusive_prices() {
        let m = |s: &str| s.parse::<Money>().unwrap_or_default();
        assert_eq!(vat_included(m("107"), 700), m("7"));
        // 100 × 7 / 107 = 6.542… → 6.54
        assert_eq!(vat_included(m("100"), 700), m("6.54"));
        // 0.15 × 7 / 107 = 0.0098 → 0.01
        assert_eq!(vat_included(m("0.15"), 700), m("0.01"));
        let s = TaxSettings {
            enabled: true,
            seller_name: "บริษัท หัวใจ จำกัด".into(),
            tax_id: with_check("010555512345"),
            address: "กรุงเทพฯ".into(),
            ..Default::default()
        };
        assert_eq!(s.validate(), Ok(()));
        let snap = SaleTax::for_total(&s, m("358"));
        assert_eq!(snap.vat.satang() + snap.base.satang(), 35_800);
        assert_eq!(snap.vat, m("23.42"));
        let bad = TaxSettings {
            tax_id: "1234567890123".into(),
            ..s
        };
        assert_eq!(bad.validate(), Err(DomainError::InvalidTaxSettings));
    }
}
