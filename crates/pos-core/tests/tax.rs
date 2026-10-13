use pos_core::{Error, Pos};
use pos_domain::{
    Buyer, Cart, CartLine, DomainError, Money, PaymentMethod, PaymentRequest, Product,
    ProductInput, RefundLine, RefundRequest, Sale, TaxSettings,
};
use std::error::Error as StdError;
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
fn with_check(first12: &str) -> String {
    let sum: u32 = first12
        .chars()
        .filter_map(|c| c.to_digit(10))
        .enumerate()
        .map(|(i, d)| d * (13 - i as u32))
        .sum();
    format!("{first12}{}", (11 - sum % 11) % 10)
}
fn settings() -> TaxSettings {
    TaxSettings {
        enabled: true,
        seller_name: "บริษัท หัวใจ จำกัด".into(),
        tax_id: with_check("010555512345"),
        branch: "00000".into(),
        address: "1 ถนนสุขุมวิท กรุงเทพฯ".into(),
        pos_id: "E0123456789".into(),
        ..Default::default()
    }
}
fn buyer() -> Buyer {
    Buyer {
        name: "บริษัท ลูกค้า จำกัด".into(),
        tax_id: with_check("010555598765"),
        branch: "00001".into(),
        address: "เชียงใหม่".into(),
    }
}
struct Shop {
    pos: Pos,
    shift: Uuid,
    water: Product,
    _dir: tempfile::TempDir,
}
fn shop() -> Result<Shop> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let water = pos.save_product(ProductInput {
        sku: "W".into(),
        name: "น้ำ".into(),
        price: "107".parse()?,
        ..Default::default()
    })?;
    Ok(Shop {
        pos,
        shift,
        water,
        _dir: dir,
    })
}
fn pay(s: &mut Shop, qty: u32, at: i64) -> Result<Sale> {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: s.water.id,
        quantity: qty,
        ..Default::default()
    });
    Ok(s.pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Card,
            amount: Money::from_satang(10_700 * i64::from(qty))?,
        },
        Some(s.shift),
        at,
    )?)
}

#[test]
fn receipts_snapshot_vat_only_after_it_is_enabled() -> Result {
    let mut s = shop()?;
    let before = pay(&mut s, 1, 10)?;
    assert!(before.tax.is_none());
    let mut bad = settings();
    bad.tax_id = "1234567890123".into();
    assert!(matches!(
        s.pos.set_tax_settings(&bad),
        Err(Error::Domain(DomainError::InvalidTaxSettings))
    ));
    s.pos.set_tax_settings(&settings())?;
    let after = pay(&mut s, 2, 11)?;
    let tax = after.tax.as_ref().ok_or("tax")?;
    assert_eq!((tax.base, tax.vat), ("200".parse()?, "14".parse()?));
    assert_eq!(tax.pos_id, "E0123456789");
    // Later edits never rewrite earlier receipts.
    let mut renamed = settings();
    renamed.seller_name = "ชื่อใหม่".into();
    s.pos.set_tax_settings(&renamed)?;
    let stored = &s.pos.history(0)?[0].sale;
    assert_eq!(
        stored.tax.as_ref().map(|t| t.seller_name.as_str()),
        Some("บริษัท หัวใจ จำกัด")
    );
    assert_eq!(s.pos.tax_settings()?, renamed);
    Ok(())
}

#[test]
fn full_tax_invoices_number_separately_once_per_sale() -> Result {
    let mut s = shop()?;
    let plain = pay(&mut s, 1, 10)?;
    s.pos.set_tax_settings(&settings())?;
    let a = pay(&mut s, 1, 11)?;
    let b = pay(&mut s, 1, 12)?;
    assert!(matches!(
        s.pos.issue_tax_invoice(plain.id, &buyer(), 20),
        Err(Error::NotTaxReceipt)
    ));
    let mut wrong = buyer();
    wrong.tax_id = "0105555987650".into();
    if pos_domain::valid_tax_id(&wrong.tax_id) {
        wrong.tax_id = "0105555987651".into();
    }
    assert!(matches!(
        s.pos.issue_tax_invoice(a.id, &wrong, 20),
        Err(Error::Domain(DomainError::InvalidBuyer))
    ));
    let first = s.pos.issue_tax_invoice(b.id, &buyer(), 21)?;
    let second = s.pos.issue_tax_invoice(a.id, &buyer(), 22)?;
    assert_eq!((first.number, second.number), (1, 2));
    assert_eq!(first.sale_number, b.number);
    // Retrying returns the same invoice; another buyer is refused.
    assert_eq!(s.pos.issue_tax_invoice(b.id, &buyer(), 23)?, first);
    let mut other = buyer();
    other.name = "คนอื่น".into();
    assert!(matches!(
        s.pos.issue_tax_invoice(b.id, &other, 24),
        Err(Error::ConflictingRequest)
    ));
    let records = s.pos.history(0)?;
    let record = records.iter().find(|r| r.sale.id == b.id).ok_or("b")?;
    assert_eq!(record.tax_invoice.as_ref(), Some(&first));
    Ok(())
}

#[test]
fn refunded_sales_need_a_credit_note_and_reports_net_vat() -> Result {
    let mut s = shop()?;
    s.pos.set_tax_settings(&settings())?;
    let day = date!(2026 - 10 - 13);
    let at = day.midnight().assume_offset(offset!(+7)).unix_timestamp() + 3600;
    let sale = pay(&mut s, 2, at)?;
    s.pos.refund_items(
        &RefundRequest {
            id: Uuid::new_v4(),
            sale_id: sale.id,
            refund_method: PaymentMethod::Card,
            reason: "คืน".into(),
            lines: vec![RefundLine {
                line: 0,
                quantity: 1,
            }],
            legacy_restock: None,
        },
        Some(s.shift),
        at + 60,
    )?;
    assert!(matches!(
        s.pos.issue_tax_invoice(sale.id, &buyer(), at + 120),
        Err(Error::SaleRefunded)
    ));
    let r = s.pos.daily_summary(day)?;
    assert_eq!((r.vat_sales, r.vat_refunds), ("14".parse()?, "7".parse()?));
    Ok(())
}
