use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, Discount, DomainError, Money, PaymentMethod, PaymentRequest, Product,
    ProductInput, RefundLine, RefundRequest, Sale,
};
use std::error::Error as StdError;
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
struct Shop {
    pos: Pos,
    shift: Uuid,
    water: Product,
    rice: Product,
    _dir: tempfile::TempDir,
}
fn shop() -> Result<Shop> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, "100".parse()?, 0)?;
    let water = pos.save_product(ProductInput {
        sku: "W".into(),
        name: "น้ำ".into(),
        category: "Drinks".into(),
        price: "10".parse()?,
        stock: Some(20),
        ..Default::default()
    })?;
    let rice = pos.save_product(ProductInput {
        sku: "R".into(),
        name: "ข้าว".into(),
        category: "Food".into(),
        price: "50".parse()?,
        ..Default::default()
    })?;
    Ok(Shop {
        pos,
        shift,
        water,
        rice,
        _dir: dir,
    })
}
/// 3 water at 10% off (30 − 3 = 27) + 2 rice (100); bill −20 → 107.
fn cart(s: &Shop) -> Result<Cart> {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: s.water.id,
        quantity: 3,
        discount: Some(Discount::Percent(1_000)),
        ..Default::default()
    });
    cart.lines.push(CartLine {
        product_id: s.rice.id,
        quantity: 2,
        ..Default::default()
    });
    cart.discount = Some(Discount::Amount("20".parse()?));
    cart.discount_reason = "สมาชิก".into();
    Ok(cart)
}
fn pay(s: &mut Shop, cart: &Cart, method: PaymentMethod, amount: &str, at: i64) -> Result<Sale> {
    Ok(s.pos.checkout_with_payment(
        cart,
        PaymentRequest {
            method,
            amount: amount.parse()?,
        },
        Some(s.shift),
        at,
    )?)
}

#[test]
fn checkout_prices_line_and_bill_discounts() -> Result {
    let mut s = shop()?;
    let cart = cart(&s)?;
    let sale = pay(&mut s, &cart, PaymentMethod::Cash, "200", 10)?;
    assert_eq!(sale.subtotal(), "130".parse()?);
    assert_eq!(sale.lines[0].discount, "3".parse()?);
    assert_eq!(sale.discount, "20".parse()?);
    assert_eq!(sale.discounts(), "23".parse()?);
    assert_eq!(sale.total, "107".parse()?);
    assert_eq!(sale.change, "93".parse()?);
    assert_eq!(sale.discount_reason, "สมาชิก");
    // The bill discount is spread 27 : 100 over the lines.
    let shares: i64 = sale.lines.iter().map(|l| l.bill_discount.satang()).sum();
    assert_eq!(shares, 2_000);
    let nets: i64 = sale.lines.iter().map(|l| l.net().satang()).sum();
    assert_eq!(nets, sale.total.satang());
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.sales, "107".parse()?);
    // The stored receipt reads back the same.
    let stored = &s.pos.history(0)?[0].sale;
    assert_eq!(stored.discounts(), sale.discounts());
    assert_eq!(
        stored.lines[0].discount_rule,
        Some(Discount::Percent(1_000))
    );
    Ok(())
}

#[test]
fn noncash_review_must_match_the_discounted_total() -> Result {
    let mut s = shop()?;
    let cart = cart(&s)?;
    assert!(matches!(
        pay(&mut s, &cart, PaymentMethod::Transfer, "130", 10)
            .map_err(|e| e.downcast::<Error>().map(|e| *e)),
        Err(Ok(Error::PaymentTotalChanged))
    ));
    assert_eq!(s.pos.product(s.water.id)?.stock, Some(20));
    let sale = pay(&mut s, &cart, PaymentMethod::Transfer, "107", 11)?;
    assert_eq!((sale.cash, sale.change), (Money::ZERO, Money::ZERO));
    Ok(())
}

#[test]
fn oversized_or_invalid_discounts_refuse_the_whole_sale() -> Result {
    let mut s = shop()?;
    let mut big = cart(&s)?;
    big.lines[0].discount = Some(Discount::Amount("30.01".parse()?));
    let err = pay(&mut s, &big, PaymentMethod::Cash, "200", 10)
        .err()
        .and_then(|e| e.downcast::<Error>().ok());
    assert!(
        matches!(
            err.as_deref(),
            Some(Error::Domain(DomainError::DiscountTooLarge))
        ),
        "{err:?}"
    );
    let mut bill = cart(&s)?;
    bill.discount = Some(Discount::Amount("127.01".parse()?));
    assert!(pay(&mut s, &bill, PaymentMethod::Cash, "200", 11).is_err());
    let mut zero = cart(&s)?;
    zero.discount = Some(Discount::Percent(0));
    assert!(pay(&mut s, &zero, PaymentMethod::Cash, "200", 12).is_err());
    assert_eq!(s.pos.product(s.water.id)?.stock, Some(20));
    assert!(s.pos.history(0)?.is_empty());
    // A fully discounted bill is still a sale.
    let mut free = cart(&s)?;
    free.discount = Some(Discount::Percent(10_000));
    let sale = pay(&mut s, &free, PaymentMethod::Cash, "0", 13)?;
    assert_eq!(sale.total, Money::ZERO);
    Ok(())
}

#[test]
fn partial_refunds_return_what_was_paid() -> Result {
    let mut s = shop()?;
    let cart = cart(&s)?;
    let sale = pay(&mut s, &cart, PaymentMethod::Cash, "200", 10)?;
    let mut refunded = Money::ZERO;
    for (line, quantity) in [(0, 1), (1, 1), (0, 2), (1, 1)] {
        let r = s.pos.refund_items(
            &RefundRequest {
                id: Uuid::new_v4(),
                sale_id: sale.id,
                refund_method: PaymentMethod::Cash,
                reason: "คืน".into(),
                lines: vec![RefundLine { line, quantity }],
                legacy_restock: None,
            },
            Some(s.shift),
            20,
        )?;
        assert!(r.refund <= sale.lines[line].net());
        refunded = refunded.checked_add(r.refund)?;
    }
    // Every satang paid comes back, and no more.
    assert_eq!(refunded, sale.total);
    let record = &s.pos.history(0)?[0];
    assert!(record.fully_refunded());
    assert_eq!(record.refunded_total(), sale.total);
    Ok(())
}

#[test]
fn reports_show_paid_amounts_and_discounts() -> Result {
    let mut s = shop()?;
    let day = date!(2026 - 10 - 13);
    let at = day.midnight().assume_offset(offset!(+7)).unix_timestamp() + 3600;
    let cart = cart(&s)?;
    let sale = pay(&mut s, &cart, PaymentMethod::Card, "107", at)?;
    let r = s.pos.daily_summary(day)?;
    assert_eq!(r.gross_sales, "107".parse()?);
    assert_eq!(r.discounts, "23".parse()?);
    let product_total: i64 = r.top_products.iter().map(|p| p.net_sales.satang()).sum();
    assert_eq!(product_total, sale.total.satang());
    let category_total: i64 = r.categories.iter().map(|c| c.net_sales.satang()).sum();
    assert_eq!(category_total, sale.total.satang());
    Ok(())
}

#[test]
fn held_bills_keep_their_discounts() -> Result {
    let mut s = shop()?;
    let cart = cart(&s)?;
    s.pos.hold(&cart, 5)?;
    let held = s.pos.held()?;
    assert_eq!(held[0], cart);
    assert_eq!(held[0].lines[0].discount, Some(Discount::Percent(1_000)));
    Ok(())
}
