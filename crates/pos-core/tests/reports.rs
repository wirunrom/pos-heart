use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, Money, PaymentMethod, PaymentRequest, Product, ProductInput, VoidRequest,
};
use std::error::Error as StdError;
use time::{
    Date,
    macros::{date, offset},
};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
fn midnight(d: Date) -> i64 {
    d.midnight().assume_offset(offset!(+7)).unix_timestamp()
}
fn product(pos: &mut Pos, sku: &str, category: &str, price: &str) -> Result<Product> {
    Ok(pos.save_product(ProductInput {
        sku: sku.into(),
        name: sku.into(),
        category: category.into(),
        price: price.parse()?,
        ..Default::default()
    })?)
}
fn sell(pos: &mut Pos, shift: Uuid, p: &Product, qty: u32, at: i64) -> Result<pos_domain::Sale> {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: p.id,
        quantity: qty,
        ..Default::default()
    });
    Ok(pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: "1000".parse()?,
        },
        Some(shift),
        at,
    )?)
}

#[test]
fn ranges_split_days_hours_and_categories() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let d1 = date!(2026 - 10 - 12);
    let d2 = date!(2026 - 10 - 13);
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, midnight(d1))?;
    let tea = product(&mut pos, "TEA", "Drinks", "45")?;
    let rice = product(&mut pos, "RICE", "Food", "60")?;
    let first = sell(&mut pos, shift, &tea, 2, midnight(d1) + 9 * 3600)?;
    sell(&mut pos, shift, &rice, 1, midnight(d2) + 9 * 3600 + 30)?;
    sell(&mut pos, shift, &tea, 1, midnight(d2) + 12 * 3600)?;
    // Day 1's sale is refunded on day 2.
    pos.void_sale_for_shift(
        &VoidRequest {
            sale_id: first.id,
            refund_method: PaymentMethod::Cash,
            reason: "ผิดรายการ".into(),
            legacy_restock: None,
        },
        Some(shift),
        midnight(d2) + 13 * 3600,
    )?;
    let s = pos.summary(d1, d2)?;
    assert_eq!(
        (s.date.as_str(), s.end_date.as_str()),
        ("2026-10-12", "2026-10-13")
    );
    assert_eq!(s.days.len(), 2);
    assert_eq!(s.days[0].gross, "90".parse()?);
    assert_eq!(s.days[0].refunds, Money::ZERO);
    assert_eq!(s.days[1].gross, "105".parse()?);
    assert_eq!(s.days[1].refunds, "90".parse()?);
    assert_eq!(s.gross_sales, "195".parse()?);
    assert_eq!(s.net.satang(), 10_500);
    // 09:00 on both days lands in the same hour bucket.
    assert_eq!(s.hourly[9], "150".parse()?);
    assert_eq!(s.hourly[12], "45".parse()?);
    let food = s
        .categories
        .iter()
        .find(|c| c.category == "Food")
        .ok_or("food")?;
    assert_eq!((food.net_quantity, food.net_sales.satang()), (1, 6_000));
    let drinks = s
        .categories
        .iter()
        .find(|c| c.category == "Drinks")
        .ok_or("drinks")?;
    assert_eq!((drinks.net_quantity, drinks.net_sales.satang()), (1, 4_500));
    // A single day still matches the daily report.
    let day2 = pos.daily_summary(d2)?;
    assert_eq!(day2.net.satang(), 6_000 + 4_500 - 9_000);
    assert_eq!(day2.days.len(), 1);
    Ok(())
}

#[test]
fn invalid_ranges_are_refused() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let d = date!(2026 - 10 - 13);
    assert!(matches!(
        pos.summary(d, date!(2026 - 10 - 12)),
        Err(Error::InvalidReportDate)
    ));
    assert!(matches!(
        pos.summary(date!(2025 - 10 - 12), d),
        Err(Error::InvalidReportDate)
    ));
    assert_eq!(pos.summary(date!(2025 - 10 - 13), d)?.days.len(), 366);
    Ok(())
}
