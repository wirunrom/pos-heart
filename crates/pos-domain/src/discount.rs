//! Discounts on a line or on the whole bill, in baht or percent.
//! Checkout and the desktop cart both price through `price_bill`, so the
//! reviewed total and the stored receipt always agree.
use crate::{DomainError, Money};
use serde::{Deserialize, Serialize};

/// A discount rule as entered by the cashier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum Discount {
    /// A fixed amount off, never more than the price it applies to.
    Amount(Money),
    /// Basis points: 1_000 = 10%, 10_000 = 100%.
    Percent(u32),
}
impl Discount {
    pub fn validate(&self) -> Result<(), DomainError> {
        match *self {
            Self::Amount(m) if m.satang() > 0 => Ok(()),
            Self::Percent(bp) if (1..=10_000).contains(&bp) => Ok(()),
            _ => Err(DomainError::InvalidDiscount),
        }
    }
    /// Amount taken off `price`. Percentages round half up to the satang.
    pub fn off(&self, price: Money) -> Result<Money, DomainError> {
        self.validate()?;
        match *self {
            Self::Amount(m) if m > price => Err(DomainError::DiscountTooLarge),
            Self::Amount(m) => Ok(m),
            Self::Percent(bp) => {
                let scaled = i128::from(price.satang()) * i128::from(bp);
                let off = (scaled + 5_000) / 10_000;
                Money::from_satang(i64::try_from(off).map_err(|_| DomainError::Overflow)?)
            }
        }
    }
}
/// One priced line: `gross` is unit price × quantity before any discount.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinePrice {
    pub gross: Money,
    /// From the line's own discount.
    pub discount: Money,
    /// This line's share of the bill discount.
    pub bill_share: Money,
    /// Service charge on this line (zero when exempt).
    pub service: Money,
    /// VAT added on top when prices exclude VAT.
    pub vat_added: Money,
    /// Share of the cash rounding, in satang (negative when rounded down).
    pub rounding: i64,
}
impl LinePrice {
    /// What the customer pays for this line.
    pub fn net(&self) -> Money {
        // Shares never exceed what they apply to (see price_sale).
        Money::from_satang(
            self.gross.satang() - self.discount.satang() - self.bill_share.satang()
                + self.service.satang()
                + self.vat_added.satang()
                + self.rounding,
        )
        .unwrap_or_default()
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BillPrice {
    pub lines: Vec<LinePrice>,
    /// Sum of gross line amounts.
    pub subtotal: Money,
    pub line_discounts: Money,
    pub bill_discount: Money,
    pub service_charge: Money,
    /// VAT added when prices exclude VAT (zero when prices include it).
    pub vat_added: Money,
    /// Cash rounding in satang (negative when rounded down).
    pub rounding: i64,
    /// Amount to pay.
    pub total: Money,
}
impl BillPrice {
    pub fn discounts(&self) -> Money {
        Money::from_satang(self.line_discounts.satang() + self.bill_discount.satang())
            .unwrap_or_default()
    }
}
/// Discounts only; see `price_sale` for service charge, VAT and rounding.
pub fn price_bill(
    lines: &[(Money, Option<Discount>)],
    bill: Option<Discount>,
) -> Result<BillPrice, DomainError> {
    let lines: Vec<_> = lines.iter().map(|&(g, d)| (g, d, false)).collect();
    price_sale(&lines, bill, &crate::PriceOptions::default())
}
/// Prices a bill in a fixed order: line discounts, bill discount (spread by
/// line), service charge on eligible lines, VAT added on top when prices
/// exclude it, then cash rounding. Every amount is spread across lines
/// (largest remainder) so partial refunds return exactly what was paid.
/// Lines are (gross, discount, service charge applies).
pub fn price_sale(
    lines: &[(Money, Option<Discount>, bool)],
    bill: Option<Discount>,
    options: &crate::PriceOptions,
) -> Result<BillPrice, DomainError> {
    let mut out = BillPrice::default();
    let mut after = Vec::with_capacity(lines.len());
    for (gross, discount, _) in lines {
        let off = match discount {
            Some(d) => d.off(*gross)?,
            None => Money::ZERO,
        };
        out.subtotal = out.subtotal.checked_add(*gross)?;
        out.line_discounts = out.line_discounts.checked_add(off)?;
        out.lines.push(LinePrice {
            gross: *gross,
            discount: off,
            ..LinePrice::default()
        });
        after.push(gross.satang() - off.satang());
    }
    let remaining: i64 = after.iter().sum();
    let remaining = Money::from_satang(remaining)?;
    out.bill_discount = match bill {
        Some(d) => d.off(remaining)?,
        None => Money::ZERO,
    };
    for (line, share) in out
        .lines
        .iter_mut()
        .zip(allocate(out.bill_discount, &after)?)
    {
        line.bill_share = share;
    }
    // What each line costs after discounts.
    let mut weights: Vec<i64> = out
        .lines
        .iter()
        .map(|l| l.gross.satang() - l.discount.satang() - l.bill_share.satang())
        .collect();
    // Service charge on eligible lines.
    if options.service_bp > 0 {
        let eligible: Vec<i64> = weights
            .iter()
            .zip(lines)
            .map(|(w, (_, _, on))| if *on { *w } else { 0 })
            .collect();
        let base = Money::from_satang(eligible.iter().sum())?;
        out.service_charge = crate::percent_of(base, options.service_bp)?;
        for ((line, share), w) in out
            .lines
            .iter_mut()
            .zip(allocate(out.service_charge, &eligible)?)
            .zip(weights.iter_mut())
        {
            line.service = share;
            *w += share.satang();
        }
    }
    // VAT on top when prices exclude it.
    if let Some(rate) = options.vat_exclusive_bp {
        let base = Money::from_satang(weights.iter().sum())?;
        out.vat_added = crate::percent_of(base, rate)?;
        for ((line, share), w) in out
            .lines
            .iter_mut()
            .zip(allocate(out.vat_added, &weights)?)
            .zip(weights.iter_mut())
        {
            line.vat_added = share;
            *w += share.satang();
        }
    }
    let before = Money::from_satang(weights.iter().sum())?;
    // Cash rounding, spread as a signed share.
    if let Some(r) = options.rounding {
        out.rounding = r.adjustment(before);
        let shares = allocate(Money::from_satang(out.rounding.abs())?, &weights)?;
        for (line, share) in out.lines.iter_mut().zip(shares) {
            line.rounding = share.satang() * out.rounding.signum();
        }
    }
    out.total = Money::from_satang(before.satang() + out.rounding)?;
    Ok(out)
}
/// Splits `amount` (≤ sum of weights) in proportion to nonnegative weights,
/// exactly, giving leftover satang to the largest remainders.
pub(crate) fn allocate(amount: Money, weights: &[i64]) -> Result<Vec<Money>, DomainError> {
    let total: i128 = weights.iter().map(|w| i128::from(*w)).sum();
    if total == 0 || amount.satang() == 0 {
        return Ok(vec![Money::ZERO; weights.len()]);
    }
    let a = i128::from(amount.satang());
    let mut shares: Vec<i128> = weights.iter().map(|w| a * i128::from(*w) / total).collect();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    // Largest remainder first; ties go to the earlier line.
    order.sort_by_key(|&i| std::cmp::Reverse(a * i128::from(weights[i]) % total));
    let mut left = a - shares.iter().sum::<i128>();
    for i in order {
        if left == 0 {
            break;
        }
        shares[i] += 1;
        left -= 1;
    }
    shares
        .into_iter()
        .map(|s| Money::from_satang(i64::try_from(s).map_err(|_| DomainError::Overflow)?))
        .collect()
}
/// Amount refunded for `quantity` more units of a line whose `done` units were
/// refunded before. Cumulative rounding makes all refunds add up to `net`.
pub fn refund_share(net: Money, sold: u32, done: u32, quantity: u32) -> Result<Money, DomainError> {
    let after = done.checked_add(quantity).ok_or(DomainError::Overflow)?;
    if sold == 0 || after > sold {
        return Err(DomainError::InvalidQuantity);
    }
    let part = |units: u32| i128::from(net.satang()) * i128::from(units) / i128::from(sold);
    Money::from_satang(i64::try_from(part(after) - part(done)).map_err(|_| DomainError::Overflow)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(s: &str) -> Money {
        s.parse().unwrap_or_default()
    }
    #[test]
    fn percent_rounds_half_up_and_amounts_cap_at_price() {
        assert_eq!(Discount::Percent(1_000).off(m("45")), Ok(m("4.5")));
        // 12.5% of 0.20 = 0.025 → 0.03
        assert_eq!(Discount::Percent(1_250).off(m("0.20")), Ok(m("0.03")));
        assert_eq!(Discount::Percent(10_000).off(m("89.50")), Ok(m("89.50")));
        assert_eq!(Discount::Amount(m("10")).off(m("10")), Ok(m("10")));
        assert_eq!(
            Discount::Amount(m("10.01")).off(m("10")),
            Err(DomainError::DiscountTooLarge)
        );
        assert_eq!(
            Discount::Percent(0).off(m("10")),
            Err(DomainError::InvalidDiscount)
        );
        assert_eq!(
            Discount::Percent(10_001).validate(),
            Err(DomainError::InvalidDiscount)
        );
        assert_eq!(
            Discount::Amount(Money::ZERO).validate(),
            Err(DomainError::InvalidDiscount)
        );
    }
    #[test]
    fn bill_discount_spreads_exactly_across_lines() -> Result<(), DomainError> {
        let price = price_bill(
            &[
                (m("100"), Some(Discount::Percent(1_000))),
                (m("50"), None),
                (m("0.01"), None),
            ],
            Some(Discount::Amount(m("10"))),
        )?;
        assert_eq!(price.subtotal, m("150.01"));
        assert_eq!(price.line_discounts, m("10"));
        assert_eq!(price.bill_discount, m("10"));
        assert_eq!(price.total, m("130.01"));
        let shares: i64 = price.lines.iter().map(|l| l.bill_share.satang()).sum();
        assert_eq!(shares, 1_000);
        let nets: i64 = price.lines.iter().map(|l| l.net().satang()).sum();
        assert_eq!(nets, price.total.satang());
        // 90 : 50 : 0.01 → 6.43 / 3.57 / 0.00
        assert_eq!(price.lines[0].bill_share, m("6.43"));
        assert_eq!(price.lines[1].bill_share, m("3.57"));
        // A bill discount larger than what is left is refused.
        assert_eq!(
            price_bill(&[(m("5"), None)], Some(Discount::Amount(m("6")))),
            Err(DomainError::DiscountTooLarge)
        );
        // Free bills are allowed.
        assert_eq!(
            price_bill(&[(m("5"), None)], Some(Discount::Percent(10_000)))?.total,
            Money::ZERO
        );
        Ok(())
    }
    #[test]
    fn refund_shares_add_up_to_the_line() -> Result<(), DomainError> {
        let net = m("100");
        let a = refund_share(net, 3, 0, 1)?;
        let b = refund_share(net, 3, 1, 1)?;
        let c = refund_share(net, 3, 2, 1)?;
        assert_eq!((a, b, c), (m("33.33"), m("33.33"), m("33.34")));
        assert_eq!(refund_share(m("110"), 2, 0, 1)?, m("55"));
        assert_eq!(
            refund_share(net, 3, 2, 2),
            Err(DomainError::InvalidQuantity)
        );
        Ok(())
    }
}
