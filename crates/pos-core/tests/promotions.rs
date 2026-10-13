use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, Discount, Money, PaymentMethod, PaymentRequest, ProductInput, PromoRule,
    PromoTarget, Promotion,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

// Monday 2025-10-13 18:30 in Bangkok.
const MONDAY_EVENING: i64 = 1_760_355_000;

fn money(s: &str) -> Result<Money> {
    Ok(s.parse()?)
}
fn bill(lines: &[(&pos_domain::Product, u32)]) -> Cart {
    let mut c = Cart::new();
    for (p, quantity) in lines {
        c.lines.push(CartLine {
            product_id: p.id,
            quantity: *quantity,
            ..Default::default()
        });
    }
    c
}
fn card(
    pos: &mut Pos,
    cart: &Cart,
    amount: Money,
    shift: Uuid,
    at: i64,
) -> Result<pos_domain::Sale> {
    Ok(pos.checkout_with_payment(
        cart,
        PaymentRequest {
            method: PaymentMethod::Card,
            amount,
        },
        Some(shift),
        at,
    )?)
}

#[test]
fn promotions_apply_at_checkout_and_are_snapshotted() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let tea = pos.save_product(ProductInput {
        sku: "T".into(),
        name: "ชาไทย".into(),
        category: "Drinks".into(),
        price: money("45")?,
        ..Default::default()
    })?;
    let rice = pos.save_product(ProductInput {
        sku: "R".into(),
        name: "ข้าวผัด".into(),
        category: "Food".into(),
        price: money("60")?,
        ..Default::default()
    })?;
    let mut happy = Promotion::new(
        "ชา 2 แถม 1",
        PromoRule::BuyGet { buy: 2, free: 1 },
        PromoTarget::Product(tea.id),
    );
    happy.from_minute = 17 * 60;
    happy.to_minute = 19 * 60;
    let spend = Promotion::new(
        "ครบ 200 ลด 10%",
        PromoRule::Bill {
            min_spend: money("200")?,
            discount: Discount::Percent(1000),
        },
        PromoTarget::All,
    );
    pos.save_promotion(&happy, 1)?;
    pos.save_promotion(&spend, 2)?;
    // 3 tea (one free) + 2 rice = 90 + 120 = 210 ≥ 200 → 10% off = 189.
    let cart = bill(&[(&tea, 3), (&rice, 2)]);
    let full = PaymentRequest {
        method: PaymentMethod::Card,
        amount: money("255")?,
    };
    assert!(matches!(
        pos.checkout_with_payment(&cart, full, Some(shift), MONDAY_EVENING),
        Err(Error::PaymentTotalChanged)
    ));
    let sale = card(&mut pos, &cart, money("189")?, shift, MONDAY_EVENING)?;
    assert_eq!(sale.total, money("189")?);
    assert_eq!(sale.lines[0].promotion.as_deref(), Some("ชา 2 แถม 1"));
    assert_eq!(sale.lines[0].discount, money("45")?);
    assert_eq!(sale.lines[1].promotion, None);
    let names: Vec<(&str, Money)> = sale
        .promotions
        .iter()
        .map(|p| (p.name.as_str(), p.amount))
        .collect();
    assert_eq!(
        names,
        vec![
            ("ชา 2 แถม 1", money("45")?),
            ("ครบ 200 ลด 10%", money("21")?)
        ]
    );
    let uses: Vec<u32> = pos.promotions()?.iter().map(|p| p.uses).collect();
    assert_eq!(uses, vec![1, 1]);
    // Outside happy hour only the bill promotion would apply, and 3 tea
    // (135) does not reach it.
    let later = card(
        &mut pos,
        &bill(&[(&tea, 3)]),
        money("135")?,
        shift,
        MONDAY_EVENING + 7200,
    )?;
    assert!(later.promotions.is_empty());
    // The report counts bills and amounts per promotion, most saved first.
    let report = pos.daily_summary(time::macros::date!(2025 - 10 - 13))?;
    let rows: Vec<(&str, i64, String)> = report
        .promotions
        .iter()
        .map(|p| (p.name.as_str(), p.bills, p.amount.to_string()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("ชา 2 แถม 1", 1, "45.00".into()),
            ("ครบ 200 ลด 10%", 1, "21.00".into())
        ]
    );
    // A void takes the bill back out.
    pos.void_sale_for_shift(
        &pos_domain::VoidRequest {
            sale_id: sale.id,
            refund_method: sale.payment_method,
            reason: "ผิด".into(),
            legacy_restock: None,
        },
        Some(shift),
        MONDAY_EVENING + 60,
    )?;
    let report = pos.daily_summary(time::macros::date!(2025 - 10 - 13))?;
    assert!(report.promotions.is_empty());
    Ok(())
}

#[test]
fn coupons_need_their_code_and_stop_when_used_up() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let rice = pos.save_product(ProductInput {
        sku: "R".into(),
        name: "ข้าวผัด".into(),
        price: money("100")?,
        ..Default::default()
    })?;
    let mut coupon = Promotion::new(
        "คูปองลด 20",
        PromoRule::Off {
            discount: Discount::Amount(money("20")?),
        },
        PromoTarget::All,
    );
    coupon.coupon = " welcome ".into();
    coupon.max_uses = Some(1);
    pos.save_promotion(&coupon, 1)?;
    assert_eq!(pos.promotions()?[0].coupon, "WELCOME");
    let mut dup = coupon.clone();
    dup.id = Uuid::new_v4();
    assert!(matches!(
        pos.save_promotion(&dup, 2),
        Err(Error::DuplicateCoupon)
    ));
    // Without the code: full price.
    card(&mut pos, &bill(&[(&rice, 1)]), money("100")?, shift, 10)?;
    let mut with = bill(&[(&rice, 1)]);
    with.coupons = vec!["Welcome".into()];
    let used = card(&mut pos, &with, money("80")?, shift, 11)?;
    assert_eq!(used.promotions[0].amount, money("20")?);
    // Used up: the next bill with the code pays full price.
    let mut again = bill(&[(&rice, 1)]);
    again.coupons = vec!["WELCOME".into()];
    card(&mut pos, &again, money("100")?, shift, 12)?;
    // Deleting stops future use; receipts keep the name.
    pos.delete_promotion(coupon.id)?;
    assert!(pos.promotions()?.is_empty());
    Ok(())
}
