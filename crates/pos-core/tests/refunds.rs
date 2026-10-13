use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, DomainError, LineOption, Money, OptionChoice, OptionGroup, PaymentMethod,
    PaymentRequest, Product, ProductInput, RefundLine, RefundRequest, Sale, VoidRequest,
};
use std::error::Error as StdError;
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
struct Shop {
    pos: Pos,
    shift: Uuid,
    water: Product,
    tea: Product,
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
        price: "10".parse()?,
        stock: Some(20),
        ..Default::default()
    })?;
    let tea = pos.save_product(ProductInput {
        sku: "T".into(),
        name: "ชา".into(),
        price: "45".parse()?,
        options: vec![OptionGroup {
            id: Uuid::from_u128(1),
            name: "เพิ่ม".into(),
            required: false,
            multiple: true,
            choices: vec![OptionChoice {
                id: Uuid::from_u128(2),
                name: "ไข่มุก".into(),
                price: "10".parse()?,
            }],
        }],
        ..Default::default()
    })?;
    Ok(Shop {
        pos,
        shift,
        water,
        tea,
        _dir: dir,
    })
}
fn sale(s: &mut Shop, method: PaymentMethod, at: i64) -> Result<Sale> {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: s.water.id,
        quantity: 5,
        ..Default::default()
    });
    cart.lines.push(CartLine {
        product_id: s.tea.id,
        quantity: 2,
        options: vec![LineOption {
            group_id: Uuid::from_u128(1),
            choice_id: Uuid::from_u128(2),
        }],
        ..Default::default()
    });
    // 5 × 10 + 2 × 55 = 160
    let amount = if method == PaymentMethod::Cash {
        "200".parse()?
    } else {
        "160".parse()?
    };
    Ok(s.pos
        .checkout_with_payment(&cart, PaymentRequest { method, amount }, Some(s.shift), at)?)
}
fn refund(sale: &Sale, lines: &[(usize, u32)]) -> RefundRequest {
    RefundRequest {
        id: Uuid::new_v4(),
        sale_id: sale.id,
        refund_method: sale.payment_method,
        reason: "ลูกค้าคืน".into(),
        lines: lines
            .iter()
            .map(|&(line, quantity)| RefundLine { line, quantity })
            .collect(),
        legacy_restock: None,
    }
}

#[test]
fn partial_refunds_price_options_return_stock_and_hit_the_drawer() -> Result {
    let mut s = shop()?;
    let sold = sale(&mut s, PaymentMethod::Cash, 10)?;
    assert_eq!(sold.total, "160".parse()?);
    assert_eq!(s.pos.product(s.water.id)?.stock, Some(15));
    let first = s
        .pos
        .refund_items(&refund(&sold, &[(0, 2), (1, 1)]), Some(s.shift), 20)?;
    assert_eq!(first.refund, "75".parse()?); // 2 × 10 + 1 × (45 + 10)
    assert_eq!(first.lines[0].restocked, 2);
    assert_eq!(first.lines[1].restocked, 0); // untracked tea
    assert_eq!(s.pos.product(s.water.id)?.stock, Some(17));
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.refunds, "75".parse()?);
    assert_eq!(shift.expected()?.satang(), 10_000 + 16_000 - 7_500);
    // A second refund may take what is left, but not more.
    assert!(matches!(
        s.pos
            .refund_items(&refund(&sold, &[(0, 4)]), Some(s.shift), 21),
        Err(Error::RefundExceedsSale)
    ));
    s.pos
        .refund_items(&refund(&sold, &[(0, 3), (1, 1)]), Some(s.shift), 22)?;
    let record = &s.pos.history(0)?[0];
    assert_eq!(record.refunds.len(), 2);
    assert_eq!(record.refunded_units(), vec![5, 2]);
    assert!(record.fully_refunded());
    assert_eq!(record.refunded_total(), "160".parse()?);
    assert!(record.cancellation.is_none());
    assert!(matches!(
        s.pos
            .refund_items(&refund(&sold, &[(1, 1)]), Some(s.shift), 23),
        Err(Error::RefundExceedsSale)
    ));
    // The receipt itself is unchanged.
    assert_eq!(record.sale.total, sold.total);
    Ok(())
}

#[test]
fn retries_conflicts_method_and_void_rules() -> Result {
    let mut s = shop()?;
    let sold = sale(&mut s, PaymentMethod::Transfer, 10)?;
    let request = refund(&sold, &[(0, 1)]);
    let first = s.pos.refund_items(&request, Some(s.shift), 20)?;
    assert_eq!(s.pos.refund_items(&request, Some(s.shift), 21)?, first);
    assert_eq!(s.pos.product(s.water.id)?.stock, Some(16));
    let changed = RefundRequest {
        lines: vec![RefundLine {
            line: 0,
            quantity: 2,
        }],
        ..request.clone()
    };
    assert!(matches!(
        s.pos.refund_items(&changed, Some(s.shift), 22),
        Err(Error::ConflictingRequest)
    ));
    let mut wrong = refund(&sold, &[(0, 1)]);
    wrong.refund_method = PaymentMethod::Cash;
    assert!(matches!(
        s.pos.refund_items(&wrong, Some(s.shift), 22),
        Err(Error::InvalidRefundMethod)
    ));
    let mut empty = refund(&sold, &[(0, 1)]);
    empty.reason = " ".into();
    assert!(matches!(
        s.pos.refund_items(&empty, Some(s.shift), 22),
        Err(Error::Domain(DomainError::MissingVoidReason))
    ));
    assert!(matches!(
        s.pos
            .refund_items(&refund(&sold, &[(0, 0)]), Some(s.shift), 22),
        Err(Error::Domain(DomainError::InvalidQuantity))
    ));
    // A whole-sale void would refund the returned water twice.
    let void = VoidRequest {
        sale_id: sold.id,
        refund_method: PaymentMethod::Transfer,
        reason: "ยกเลิก".into(),
        legacy_restock: None,
    };
    assert!(matches!(
        s.pos.void_sale_for_shift(&void, Some(s.shift), 23),
        Err(Error::PartiallyRefunded)
    ));
    // Conversely a voided sale takes no partial refunds.
    let other = sale(&mut s, PaymentMethod::Transfer, 30)?;
    s.pos.void_sale_for_shift(
        &VoidRequest {
            sale_id: other.id,
            ..void
        },
        Some(s.shift),
        31,
    )?;
    assert!(matches!(
        s.pos
            .refund_items(&refund(&other, &[(0, 1)]), Some(s.shift), 32),
        Err(Error::SaleVoided)
    ));
    // Transfer refunds never touch the cash drawer.
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.refunds, Money::ZERO);
    assert_eq!(shift.transfer_refunds, "170".parse()?);
    Ok(())
}

#[test]
fn refunds_belong_to_the_refunding_shift_and_retry_after_closing() -> Result {
    let mut s = shop()?;
    let sold = sale(&mut s, PaymentMethod::Cash, 10)?;
    let shift = s.pos.active_shift()?.ok_or("shift")?;
    let counted = Money::from_satang(shift.expected()?.satang())?;
    s.pos.close_shift(s.shift, shift.revision, counted, 11)?;
    let request = refund(&sold, &[(0, 1)]);
    assert!(matches!(
        s.pos.refund_items(&request, None, 12),
        Err(Error::NoOpenShift)
    ));
    let next = Uuid::new_v4();
    s.pos.open_shift(next, Money::ZERO, 13)?;
    let done = s.pos.refund_items(&request, Some(next), 14)?;
    let current = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(current.refunds, "10".parse()?);
    assert_eq!(current.expected()?.satang(), -1_000);
    let closed = s.pos.shift_history(0)?;
    assert_eq!(closed[0].refunds, Money::ZERO);
    // Retrying after this shift closes returns the original refund.
    let current_count = Money::ZERO;
    s.pos.close_shift_with(
        next,
        current.revision,
        &pos_domain::ShiftCount {
            counted: current_count,
            reason: "ติดลบจากคืนเงิน".into(),
            denominations: Vec::new(),
        },
        15,
    )?;
    assert_eq!(s.pos.refund_items(&request, None, 16)?, done);
    Ok(())
}

#[test]
fn reports_count_partial_refunds_on_their_day() -> Result {
    let mut s = shop()?;
    let d1 = date!(2026 - 10 - 12);
    let d2 = date!(2026 - 10 - 13);
    let midnight = |d: time::Date| d.midnight().assume_offset(offset!(+7)).unix_timestamp();
    let sold = sale(&mut s, PaymentMethod::Card, midnight(d1) + 3600)?;
    s.pos.refund_items(
        &refund(&sold, &[(1, 1)]),
        Some(s.shift),
        midnight(d2) + 3600,
    )?;
    let day1 = s.pos.daily_summary(d1)?;
    assert_eq!(
        (day1.gross_sales, day1.refunds),
        ("160".parse()?, Money::ZERO)
    );
    let day2 = s.pos.daily_summary(d2)?;
    assert_eq!(day2.refunds, "55".parse()?);
    assert_eq!(day2.net.satang(), -5_500);
    assert_eq!((day2.refund_count, day2.void_count), (1, 0));
    assert_eq!(day2.payments.card.refunds, "55".parse()?);
    let tea = day2
        .top_products
        .iter()
        .find(|p| p.product_id == s.tea.id)
        .ok_or("tea")?;
    assert_eq!((tea.net_quantity, tea.net_sales.satang()), (-1, -5_500));
    let both = s.pos.summary(d1, d2)?;
    assert_eq!(both.net, pos_domain::SignedMoney::from_satang(10_500));
    assert_eq!(both.days[1].refunds, "55".parse()?);
    Ok(())
}

#[test]
fn version_seven_databases_gain_the_refund_table() -> Result {
    use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    {
        let mut pos = Pos::open(&path)?;
        pos.save_product(ProductInput {
            sku: "A".into(),
            name: "a".into(),
            price: "1".parse()?,
            ..Default::default()
        })?;
    }
    let mut db = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    db.batch_execute(
        "DROP TABLE promotions; DROP TABLE credit_notes; DROP TABLE kitchen_tickets; DROP TABLE dining_tables; ALTER TABLE products DROP COLUMN kitchen;
         DROP TABLE activity_events; DROP TABLE roles; DROP TABLE tax_invoices; DROP TABLE sale_refunds; DROP TABLE staff; DROP TABLE app_settings;
         ALTER TABLE stock_movements DROP COLUMN staff_json;
         ALTER TABLE products DROP COLUMN cost_satang; ALTER TABLE products DROP COLUMN service_charge; PRAGMA user_version=7;",
    )?;
    drop(db);
    let mut pos = Pos::open(&path)?;
    assert!(pos.history(0)?.is_empty());
    assert_eq!(pos.products("", 0)?.len(), 1);
    Ok(())
}
