//! Display formatting shared by every screen (design/docs/01 Localization).
//! Money is grouped with `,`, always shows two decimals and uses U+2212 for negatives.
use crate::i18n::Language;
use pos_domain::{Money, SignedMoney};

pub const MINUS: char = '\u{2212}';

fn grouped(satang: u64) -> String {
    let baht = (satang / 100).to_string();
    let mut out = String::with_capacity(baht.len() + baht.len() / 3 + 3);
    for (i, c) in baht.chars().enumerate() {
        if i > 0 && (baht.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("{out}.{:02}", satang % 100)
}
/// `1,323.50`
pub fn money(value: Money) -> String {
    grouped(value.satang().unsigned_abs())
}
/// `฿1,323.50`
pub fn baht(value: Money) -> String {
    format!("฿{}", money(value))
}
/// `−345.00` / `12,480.00`
pub fn signed(value: SignedMoney) -> String {
    let text = grouped(value.satang().unsigned_abs());
    if value.satang() < 0 {
        format!("{MINUS}{text}")
    } else {
        text
    }
}
/// `+500.00` / `−220.00` for ledger lines; zero has no sign.
pub fn delta(value: SignedMoney) -> String {
    if value.satang() > 0 {
        format!("+{}", signed(value))
    } else {
        signed(value)
    }
}
pub fn negative(value: Money) -> SignedMoney {
    SignedMoney::from_satang(value.satang().saturating_neg())
}
pub fn positive(value: Money) -> SignedMoney {
    SignedMoney::from_satang(value.satang())
}
/// `10%`, `12.5%` or `฿20.00` for a discount rule.
pub fn discount_rule(rule: pos_domain::Discount) -> String {
    match rule {
        pos_domain::Discount::Amount(m) => baht(m),
        pos_domain::Discount::Percent(bp) => {
            let whole = bp / 100;
            match bp % 100 {
                0 => format!("{whole}%"),
                f if f % 10 == 0 => format!("{whole}.{}%", f / 10),
                f => format!("{whole}.{f:02}%"),
            }
        }
    }
}
/// `−3.00` for an amount taken off.
pub fn minus(value: Money) -> String {
    signed(negative(value))
}
/// Accepts what a MoneyField shows (`1,500.00`) as well as plain input.
pub fn parse_money(text: &str) -> Result<Money, pos_domain::DomainError> {
    text.trim().trim_start_matches('฿').replace(',', "").parse()
}
/// Integer quantities with thousands separators.
pub fn count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
/// Ingredient quantity from thousandths: grouped, up to three decimals,
/// trailing zeros dropped ("1,250.5", "−0.25").
pub fn qty(milli: i64) -> String {
    let whole = signed_count(milli / 1000);
    let frac = (milli % 1000).unsigned_abs();
    let whole = if milli < 0 && milli > -1000 {
        format!("{MINUS}0")
    } else {
        whole
    };
    if frac == 0 {
        return whole;
    }
    let digits = format!("{frac:03}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}
/// "1,250.5" → 1_250_500 thousandths; at most three decimals, not negative.
pub fn parse_qty(text: &str) -> Option<i64> {
    let text = text.trim().replace(',', "");
    let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
    if whole.is_empty() && frac.is_empty() || frac.len() > 3 {
        return None;
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let frac: i64 = format!("{frac:0<3}").parse().ok()?;
    whole.checked_mul(1000)?.checked_add(frac)
}
/// Signed ingredient change: "+1", "−0.5".
pub fn signed_qty(milli: i64) -> String {
    if milli > 0 {
        format!("+{}", qty(milli))
    } else {
        qty(milli)
    }
}
/// "+1", "-0.5" or "−0.5" → signed thousandths; zero is refused.
pub fn parse_signed_qty(text: &str) -> Option<i64> {
    let text = text.trim();
    let (negative, rest) = match text.strip_prefix(['-', '\u{2212}']) {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let n = parse_qty(rest).filter(|n| *n > 0)?;
    Some(if negative { -n } else { n })
}
pub fn signed_count(value: i64) -> String {
    let text = count(value.unsigned_abs());
    if value < 0 {
        format!("{MINUS}{text}")
    } else {
        text
    }
}

const BANGKOK: time::UtcOffset = time::macros::offset!(+7);
fn local(unix: i64) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::from_unix_timestamp(unix)
        .ok()
        .map(|t| t.to_offset(BANGKOK))
}
const TH_MONTHS: [&str; 12] = [
    "ม.ค.",
    "ก.พ.",
    "มี.ค.",
    "เม.ย.",
    "พ.ค.",
    "มิ.ย.",
    "ก.ค.",
    "ส.ค.",
    "ก.ย.",
    "ต.ค.",
    "พ.ย.",
    "ธ.ค.",
];
const EN_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// `13 ต.ค. 2569` (Buddhist era) / `13 Oct 2026`
pub fn date(lang: Language, d: time::Date) -> String {
    let month = usize::from(u8::from(d.month())) - 1;
    match lang {
        Language::Thai => format!("{} {} {}", d.day(), TH_MONTHS[month], d.year() + 543),
        Language::English => format!("{} {} {}", d.day(), EN_MONTHS[month], d.year()),
    }
}
/// 24-hour `14:05` in Bangkok time.
pub fn clock(unix: i64) -> String {
    local(unix).map_or_else(
        || "—".into(),
        |t| format!("{:02}:{:02}", t.hour(), t.minute()),
    )
}
/// `13 ต.ค. 2569 14:05`
pub fn date_time(lang: Language, unix: i64) -> String {
    local(unix).map_or_else(
        || "—".into(),
        |t| format!("{} {}", date(lang, t.date()), clock(unix)),
    )
}
/// `9 ชม. 41 นาที` / `9h 41m`
pub fn duration(lang: Language, seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    match (lang, h) {
        (Language::Thai, 0) => format!("{m} นาที"),
        (Language::Thai, _) => format!("{h} ชม. {m} นาที"),
        (Language::English, 0) => format!("{m}m"),
        (Language::English, _) => format!("{h}h {m}m"),
    }
}
pub fn receipt_number(number: u64) -> String {
    format!("#{number:06}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ingredient_quantities_round_trip() {
        assert_eq!(qty(1_250_500), "1,250.5");
        assert_eq!(qty(-250), "\u{2212}0.25");
        assert_eq!(qty(-3000), "\u{2212}3");
        assert_eq!(parse_qty("1,250.5"), Some(1_250_500));
        assert_eq!(parse_qty(".25"), Some(250));
        assert_eq!(parse_qty("0.0005"), None);
        assert_eq!(parse_qty("-1"), None);
        assert_eq!(parse_qty(""), None);
        assert_eq!(parse_signed_qty("-1"), Some(-1000));
        assert_eq!(parse_signed_qty("\u{2212}0.5"), Some(-500));
        assert_eq!(parse_signed_qty("+2"), Some(2000));
        assert_eq!(parse_signed_qty("0"), None);
        assert_eq!(signed_qty(1000), "+1");
    }
    #[test]
    fn discount_rules_read_naturally() {
        use pos_domain::Discount;
        assert_eq!(discount_rule(Discount::Percent(1_000)), "10%");
        assert_eq!(discount_rule(Discount::Percent(1_250)), "12.5%");
        assert_eq!(discount_rule(Discount::Percent(1_255)), "12.55%");
        assert_eq!(discount_rule(Discount::Percent(5)), "0.05%");
        assert_eq!(
            minus(Money::from_satang(300).unwrap_or_default()),
            "\u{2212}3.00"
        );
    }
    #[test]
    fn money_is_grouped_signed_and_round_trips() -> Result<(), pos_domain::DomainError> {
        assert_eq!(money(Money::from_satang(132_350)?), "1,323.50");
        assert_eq!(money(Money::from_satang(5)?), "0.05");
        assert_eq!(money(Money::from_satang(100_000_000)?), "1,000,000.00");
        assert_eq!(signed(SignedMoney::from_satang(-2_000)), "\u{2212}20.00");
        assert_eq!(delta(SignedMoney::from_satang(50_000)), "+500.00");
        assert_eq!(parse_money("1,500.00")?.satang(), 150_000);
        assert_eq!(parse_money("฿ 12")?.satang(), 1_200);
        assert!(parse_money("-1").is_err());
        Ok(())
    }
    #[test]
    fn thai_dates_use_buddhist_era() -> Result<(), time::error::ComponentRange> {
        let d = time::Date::from_calendar_date(2026, time::Month::October, 13)?;
        assert_eq!(date(Language::Thai, d), "13 ต.ค. 2569");
        assert_eq!(date(Language::English, d), "13 Oct 2026");
        assert_eq!(duration(Language::Thai, 9 * 3600 + 41 * 60), "9 ชม. 41 นาที");
        Ok(())
    }
}
