//! Shop-defined promotions applied automatically at pricing time: percent or
//! baht off a product or category, buy X get Y free, and spend-over bill
//! discounts, optionally limited to days, hours, dates or a coupon code.
//! A cashier's own line or bill discount always wins over a promotion on
//! that line or bill, and each line takes the single best promotion for the
//! customer. Checkout and the desktop cart both price through
//! `apply_promotions`, so the reviewed total and the receipt agree.
use crate::{Discount, DomainError, Money};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum PromoTarget {
    All,
    Product(Uuid),
    Category(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PromoRule {
    /// Percent off each matching line, or baht off each matching unit.
    Off { discount: Discount },
    /// Buy `buy` units, get `free` more of the same line free.
    BuyGet { buy: u32, free: u32 },
    /// Bills spending at least `min_spend` (after line discounts) get this.
    Bill {
        min_spend: Money,
        discount: Discount,
    },
}
/// Every day of the week (bit 0 = Monday … bit 6 = Sunday).
pub const ALL_DAYS: u8 = 0b111_1111;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    pub id: Uuid,
    pub name: String,
    pub rule: PromoRule,
    /// Which lines it applies to (ignored for bill promotions).
    pub target: PromoTarget,
    /// Applies only when this code is entered on the bill; empty = automatic.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coupon: String,
    /// Bills it may be used on in total; None = unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
    /// Bills it was used on (kept by the database, not the JSON).
    #[serde(skip)]
    pub uses: u32,
    pub days: u8,
    /// Minutes after midnight, Bangkok time. Equal values mean all day; a
    /// window may run past midnight (e.g. 22:00–02:00).
    pub from_minute: u16,
    pub to_minute: u16,
    /// Inclusive Bangkok dates "YYYY-MM-DD"; empty = open-ended.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub starts: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ends: String,
    pub active: bool,
}
impl Promotion {
    pub fn new(name: &str, rule: PromoRule, target: PromoTarget) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            rule,
            target,
            coupon: String::new(),
            max_uses: None,
            uses: 0,
            days: ALL_DAYS,
            from_minute: 0,
            to_minute: 0,
            starts: String::new(),
            ends: String::new(),
            active: true,
        }
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        let name = self.name.trim().chars().count();
        let rule_ok = match self.rule {
            PromoRule::Off { discount } => discount.validate().is_ok(),
            PromoRule::BuyGet { buy, free } => (1..=99).contains(&buy) && (1..=99).contains(&free),
            PromoRule::Bill { discount, .. } => discount.validate().is_ok(),
        };
        let target_ok = match &self.target {
            PromoTarget::Category(c) => !c.trim().is_empty() && c.chars().count() <= 80,
            _ => true,
        };
        let date_ok = |d: &str| d.is_empty() || parse_date(d).is_some();
        if name == 0
            || name > 60
            || !rule_ok
            || !target_ok
            || self.coupon.chars().count() > 32
            || self.coupon.chars().any(char::is_whitespace)
            || self.days == 0
            || self.days > ALL_DAYS
            || self.from_minute > 1440
            || self.to_minute > 1440
            || !date_ok(&self.starts)
            || !date_ok(&self.ends)
            || (!self.starts.is_empty() && !self.ends.is_empty() && self.starts > self.ends)
            || self.max_uses == Some(0)
        {
            return Err(DomainError::InvalidPromotion);
        }
        Ok(())
    }
    pub fn is_coupon(&self) -> bool {
        !self.coupon.is_empty()
    }
    /// Active, within its dates, days and hours, not used up, and (for a
    /// coupon) entered on the bill.
    pub fn live(&self, when: &LocalTime, coupons: &[String]) -> bool {
        let minute = when.minute;
        let hours = if self.from_minute == self.to_minute {
            true
        } else if self.from_minute < self.to_minute {
            (self.from_minute..self.to_minute).contains(&minute)
        } else {
            minute >= self.from_minute || minute < self.to_minute
        };
        self.active
            && (self.starts.is_empty() || when.date >= self.starts)
            && (self.ends.is_empty() || when.date <= self.ends)
            && self.days & (1 << when.weekday) != 0
            && hours
            && self.max_uses.is_none_or(|max| self.uses < max)
            && (!self.is_coupon() || coupons.iter().any(|c| same_code(c, &self.coupon)))
    }
    fn matches(&self, line: &PromoLine<'_>) -> bool {
        match &self.target {
            PromoTarget::All => true,
            PromoTarget::Product(id) => *id == line.product_id,
            PromoTarget::Category(c) => c == line.category,
        }
    }
}
/// Coupon codes compare without case or surrounding spaces.
pub fn same_code(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}
fn parse_date(s: &str) -> Option<(i64, u32, u32)> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}
/// A moment in Bangkok time (UTC+7, no daylight saving).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTime {
    /// "YYYY-MM-DD".
    pub date: String,
    /// 0 = Monday … 6 = Sunday.
    pub weekday: u8,
    /// Minutes after midnight.
    pub minute: u16,
}
impl LocalTime {
    pub fn bangkok(unix: i64) -> Self {
        let local = unix + 7 * 3600;
        let days = local.div_euclid(86_400);
        let minute = (local.rem_euclid(86_400) / 60) as u16;
        // 1970-01-01 was a Thursday (weekday 3).
        let weekday = (days + 3).rem_euclid(7) as u8;
        // Civil date from days since the epoch (H. Hinnant's algorithm).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        Self {
            date: format!("{y:04}-{m:02}-{d:02}"),
            weekday,
            minute,
        }
    }
}
/// One bill line as promotions see it.
#[derive(Debug, Clone)]
pub struct PromoLine<'a> {
    pub product_id: Uuid,
    pub category: &'a str,
    /// Unit price (options included) × quantity.
    pub gross: Money,
    pub quantity: u32,
    /// The cashier's own discount on this line, which wins over promotions.
    pub manual: Option<Discount>,
}
/// Discounts to price the bill with, and which promotion gave which.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Promoted {
    /// Effective line discounts: the manual one, or a promotion's amount.
    pub line_discounts: Vec<Option<Discount>>,
    /// Promotion behind each line's discount (None for manual or none).
    pub line_promos: Vec<Option<Uuid>>,
    pub bill: Option<Discount>,
    pub bill_promo: Option<Uuid>,
}
impl Promoted {
    /// Promotions used on this bill, in first-use order.
    pub fn used(&self) -> Vec<Uuid> {
        let mut out: Vec<Uuid> = Vec::new();
        for id in self
            .line_promos
            .iter()
            .flatten()
            .chain(self.bill_promo.iter())
        {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        out
    }
}
/// What `promo` takes off `line`, if it applies.
fn line_off(promo: &Promotion, line: &PromoLine<'_>) -> Result<Money, DomainError> {
    Ok(match promo.rule {
        PromoRule::Off {
            discount: Discount::Amount(each),
        } => each.checked_mul(line.quantity)?.min(line.gross),
        PromoRule::Off { discount } => discount.off(line.gross)?,
        PromoRule::BuyGet { buy, free } => {
            let groups = line.quantity / (buy + free);
            let units = i128::from(groups * free);
            // gross = unit × quantity, so this is exact.
            let off = i128::from(line.gross.satang()) * units / i128::from(line.quantity.max(1));
            Money::from_satang(i64::try_from(off).map_err(|_| DomainError::Overflow)?)?
        }
        PromoRule::Bill { .. } => Money::ZERO,
    })
}
/// Applies the live promotions to a bill. Lines with a manual discount keep
/// it; other lines take the promotion that saves the customer the most. A
/// spend-over promotion applies only without a manual bill discount, judged
/// on the total after line discounts; the best one wins.
pub fn apply_promotions(
    lines: &[PromoLine<'_>],
    manual_bill: Option<Discount>,
    promos: &[Promotion],
    when: &LocalTime,
    coupons: &[String],
) -> Result<Promoted, DomainError> {
    let live: Vec<&Promotion> = promos.iter().filter(|p| p.live(when, coupons)).collect();
    let mut out = Promoted::default();
    let mut after = Money::ZERO;
    for line in lines {
        let mut best: Option<(Money, Uuid)> = None;
        if line.manual.is_none() {
            for promo in live.iter().filter(|p| p.matches(line)) {
                let off = line_off(promo, line)?;
                if off > Money::ZERO && best.is_none_or(|(b, _)| off > b) {
                    best = Some((off, promo.id));
                }
            }
        }
        let (discount, promo) = match (line.manual, best) {
            (Some(d), _) => (Some(d), None),
            (None, Some((off, id))) => (Some(Discount::Amount(off)), Some(id)),
            (None, None) => (None, None),
        };
        let off = match discount {
            Some(d) => d.off(line.gross)?,
            None => Money::ZERO,
        };
        after = after.checked_add(Money::from_satang(line.gross.satang() - off.satang())?)?;
        out.line_discounts.push(discount);
        out.line_promos.push(promo);
    }
    out.bill = manual_bill;
    if manual_bill.is_none() {
        let mut best: Option<(Money, Discount, Uuid)> = None;
        for promo in &live {
            if let PromoRule::Bill {
                min_spend,
                discount,
            } = promo.rule
                && after >= min_spend
                && after > Money::ZERO
            {
                let off = match discount {
                    Discount::Amount(m) => m.min(after),
                    d => d.off(after)?,
                };
                if off > Money::ZERO && best.as_ref().is_none_or(|(b, ..)| off > *b) {
                    let applied = match discount {
                        Discount::Amount(_) => Discount::Amount(off),
                        d => d,
                    };
                    best = Some((off, applied, promo.id));
                }
            }
        }
        if let Some((_, d, id)) = best {
            out.bill = Some(d);
            out.bill_promo = Some(id);
        }
    }
    Ok(out)
}
/// A promotion used on a sale, snapshotted on the receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromoUse {
    pub id: Uuid,
    pub name: String,
    /// What it took off this bill.
    pub amount: Money,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn baht(b: i64) -> Money {
        Money::from_satang(b * 100).unwrap_or_default()
    }
    fn line(id: u128, category: &str, unit: i64, quantity: u32) -> PromoLine<'_> {
        PromoLine {
            product_id: Uuid::from_u128(id),
            category,
            gross: baht(unit * i64::from(quantity)),
            quantity,
            manual: None,
        }
    }
    // Monday 2025-10-13 18:30 in Bangkok.
    const MONDAY_EVENING: i64 = 1_760_355_000;

    #[test]
    fn bangkok_time_has_the_right_date_weekday_and_minute() {
        let t = LocalTime::bangkok(MONDAY_EVENING);
        assert_eq!(
            (t.date.as_str(), t.weekday, t.minute),
            ("2025-10-13", 0, 18 * 60 + 30)
        );
        let epoch = LocalTime::bangkok(0);
        assert_eq!(
            (epoch.date.as_str(), epoch.weekday, epoch.minute),
            ("1970-01-01", 3, 7 * 60)
        );
        let leap = LocalTime::bangkok(951_782_400); // 2000-02-29 07:00 +7
        assert_eq!(leap.date, "2000-02-29");
    }
    #[test]
    fn each_line_takes_its_best_promotion_and_manual_discounts_win() -> Result<(), DomainError> {
        let when = LocalTime::bangkok(MONDAY_EVENING);
        let drinks = Promotion::new(
            "เครื่องดื่มลด 10%",
            PromoRule::Off {
                discount: Discount::Percent(1000),
            },
            PromoTarget::Category("Drinks".into()),
        );
        let mut tea = Promotion::new(
            "ชา 2 แถม 1",
            PromoRule::BuyGet { buy: 2, free: 1 },
            PromoTarget::Product(Uuid::from_u128(1)),
        );
        tea.from_minute = 17 * 60;
        tea.to_minute = 19 * 60;
        let mut lines = vec![
            line(1, "Drinks", 45, 7),
            line(2, "Drinks", 50, 1),
            line(3, "Food", 60, 1),
        ];
        let p = apply_promotions(&lines, None, &[drinks.clone(), tea.clone()], &when, &[])?;
        // Tea: two free (90) beats 10% (31.50); coffee 10%; food nothing.
        assert_eq!(p.line_discounts[0], Some(Discount::Amount(baht(90))));
        assert_eq!(p.line_promos[0], Some(tea.id));
        assert_eq!(p.line_discounts[1], Some(Discount::Amount(baht(5))));
        assert_eq!(p.line_discounts[2], None);
        assert_eq!(p.used(), vec![tea.id, drinks.id]);
        // Outside happy hour the tea gets the category 10%.
        let late = LocalTime {
            minute: 20 * 60,
            ..when.clone()
        };
        let p = apply_promotions(&lines, None, &[drinks.clone(), tea.clone()], &late, &[])?;
        assert_eq!(p.line_promos[0], Some(drinks.id));
        // A cashier's discount stays.
        lines[0].manual = Some(Discount::Percent(500));
        let p = apply_promotions(&lines, None, &[drinks, tea], &when, &[])?;
        assert_eq!(
            (p.line_discounts[0], p.line_promos[0]),
            (Some(Discount::Percent(500)), None)
        );
        Ok(())
    }
    #[test]
    fn bill_promotions_coupons_dates_and_limits() -> Result<(), DomainError> {
        let when = LocalTime::bangkok(MONDAY_EVENING);
        let mut spend = Promotion::new(
            "ครบ 500 ลด 50",
            PromoRule::Bill {
                min_spend: baht(500),
                discount: Discount::Amount(baht(50)),
            },
            PromoTarget::All,
        );
        let lines = [line(1, "Food", 100, 5)];
        let p = apply_promotions(&lines, None, std::slice::from_ref(&spend), &when, &[])?;
        assert_eq!(
            (p.bill, p.bill_promo),
            (Some(Discount::Amount(baht(50))), Some(spend.id))
        );
        // Not reached, or a manual bill discount: no promotion.
        let small = [line(1, "Food", 100, 4)];
        assert_eq!(
            apply_promotions(&small, None, std::slice::from_ref(&spend), &when, &[])?.bill,
            None
        );
        let manual = Some(Discount::Percent(1000));
        let p = apply_promotions(&lines, manual, std::slice::from_ref(&spend), &when, &[])?;
        assert_eq!((p.bill, p.bill_promo), (manual, None));
        // Coupons need their code; used-up and out-of-date promotions stop.
        spend.coupon = "WELCOME".into();
        assert!(!spend.live(&when, &[]));
        assert!(spend.live(&when, &[" welcome ".into()]));
        spend.max_uses = Some(3);
        spend.uses = 3;
        assert!(!spend.live(&when, &["WELCOME".into()]));
        spend.uses = 0;
        spend.ends = "2025-10-12".into();
        assert!(!spend.live(&when, &["WELCOME".into()]));
        spend.ends.clear();
        spend.days = ALL_DAYS & !1; // not on Mondays
        assert!(!spend.live(&when, &["WELCOME".into()]));
        // Late-night windows wrap past midnight.
        let mut night = Promotion::new(
            "ดึก",
            PromoRule::BuyGet { buy: 1, free: 1 },
            PromoTarget::All,
        );
        night.from_minute = 22 * 60;
        night.to_minute = 2 * 60;
        assert!(night.live(
            &LocalTime {
                minute: 60,
                ..when.clone()
            },
            &[]
        ));
        assert!(!night.live(&when, &[]));
        assert_eq!(night.validate(), Ok(()));
        night.name.clear();
        assert_eq!(night.validate(), Err(DomainError::InvalidPromotion));
        Ok(())
    }
}
