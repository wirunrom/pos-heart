use pos_core::Pos;
use pos_domain::{
    Cart, CartLine, CashRounding, Money, PaymentMethod, PaymentRequest, PricingSettings, Product,
    ProductInput, RefundLine, RefundRequest, RoundMode, Sale, TaxSettings,
};
use std::error::Error as StdError;
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
struct Shop {
    pos: Pos,
    shift: Uuid,
    food: Product,
    retail: Product,
    _dir: tempfile::TempDir,
}
fn shop() -> Result<Shop> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let food = pos.save_product(ProductInput {
        sku: "F".into(),
        name: "ข้าวผัด".into(),
        price: "89.50".parse()?,
        ..Default::default()
    })?;
    let retail = pos.save_product(ProductInput {
        sku: "R".into(),
        name: "น้ำดื่ม".into(),
        price: "15".parse()?,
        service_charge: false,
        ..Default::default()
    })?;
    Ok(Shop {
        pos,
        shift,
        food,
        retail,
        _dir: dir,
    })
}
fn cart(s: &Shop) -> Cart {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: s.food.id,
        quantity: 3,
        ..Default::default()
    });
    cart.lines.push(CartLine {
        product_id: s.retail.id,
        quantity: 1,
        ..Default::default()
    });
    cart
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
fn restaurant(s: &mut Shop, step: u32) -> Result {
    s.pos.set_pricing_settings(&PricingSettings {
        service_bp: 1_000,
        rounding: CashRounding {
            step,
            mode: RoundMode::Down,
        },
    })?;
    Ok(())
}

#[test]
fn service_charge_skips_exempt_items_and_bills() -> Result {
    let mut s = shop()?;
    restaurant(&mut s, 0)?;
    let c = cart(&s);
    // 3 × 89.50 = 268.50 → 10% = 26.85; water exempt. 268.50 + 15 + 26.85
    let sale = pay(&mut s, &c, PaymentMethod::Card, "310.35", 10)?;
    assert_eq!(sale.service_charge, "26.85".parse()?);
    assert_eq!(sale.service_rate_bp, 1_000);
    assert_eq!(sale.lines[1].service, Money::ZERO);
    // Takeaway: no service charge on this bill.
    let mut takeaway = cart(&s);
    takeaway.service_charge = false;
    let sale = pay(&mut s, &takeaway, PaymentMethod::Card, "283.50", 11)?;
    assert_eq!(sale.service_charge, Money::ZERO);
    Ok(())
}

#[test]
fn cash_rounds_down_but_cards_pay_to_the_satang() -> Result {
    let mut s = shop()?;
    restaurant(&mut s, 100)?;
    let c = cart(&s);
    // Cash: 310.35 → 310.00
    let cash = pay(&mut s, &c, PaymentMethod::Cash, "500", 10)?;
    assert_eq!((cash.total, cash.rounding), ("310".parse()?, -35));
    assert_eq!(cash.change, "190".parse()?);
    let nets: i64 = cash.lines.iter().map(|l| l.net().satang()).sum();
    assert_eq!(nets, cash.total.satang());
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.sales, "310".parse()?);
    // A card review must be the unrounded total.
    let c2 = cart(&s);
    assert!(pay(&mut s, &c2, PaymentMethod::Card, "310", 11).is_err());
    let card = pay(&mut s, &c2, PaymentMethod::Card, "310.35", 12)?;
    assert_eq!(card.rounding, 0);
    // Refunding everything returns exactly what was paid in cash.
    let mut back = Money::ZERO;
    for (line, quantity) in [(0, 2), (1, 1), (0, 1)] {
        let r = s.pos.refund_items(
            &RefundRequest {
                id: Uuid::new_v4(),
                sale_id: cash.id,
                refund_method: PaymentMethod::Cash,
                reason: "คืน".into(),
                lines: vec![RefundLine { line, quantity }],
                legacy_restock: None,
            },
            Some(s.shift),
            20,
        )?;
        back = back.checked_add(r.refund)?;
    }
    assert_eq!(back, cash.total);
    Ok(())
}

#[test]
fn prices_plus_plus_add_service_then_vat() -> Result {
    let mut s = shop()?;
    restaurant(&mut s, 0)?;
    let first12 = "010555512345";
    let sum: u32 = first12
        .chars()
        .filter_map(|c| c.to_digit(10))
        .enumerate()
        .map(|(i, d)| d * (13 - i as u32))
        .sum();
    s.pos.set_tax_settings(&TaxSettings {
        enabled: true,
        seller_name: "ร้านอาหาร".into(),
        tax_id: format!("{first12}{}", (11 - sum % 11) % 10),
        address: "กรุงเทพฯ".into(),
        prices_include_vat: false,
        ..Default::default()
    })?;
    let mut c = Cart::new();
    c.lines.push(CartLine {
        product_id: s.food.id,
        quantity: 2,
        ..Default::default()
    });
    // 179 + 17.90 service = 196.90; VAT 7% = 13.78 → 210.68
    let day = date!(2026 - 10 - 13);
    let at = day.midnight().assume_offset(offset!(+7)).unix_timestamp() + 3600;
    let sale = pay(&mut s, &c, PaymentMethod::Transfer, "210.68", at)?;
    assert_eq!(sale.vat_added, "13.78".parse()?);
    let tax = sale.tax.as_ref().ok_or("tax")?;
    assert_eq!(tax.base.satang() + tax.vat.satang(), sale.total.satang());
    let r = s.pos.daily_summary(day)?;
    assert_eq!(r.service_charges, "17.90".parse()?);
    assert_eq!(r.gross_sales, "210.68".parse()?);
    Ok(())
}
