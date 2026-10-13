//! Synthetic integration of the restaurant, payment, tax and recovery boundaries.
//! No printer, scanner, bank connection or other physical device is exercised.
use diesel::{connection::SimpleConnection, prelude::*, sql_types::Text};
use pos_core::{ActivityKind, ActivityQuery, Error, Pos};
use pos_domain::{
    Buyer, Cart, CartLine, CashRounding, DiningTable, Discount, LineOption, Money, OptionChoice,
    OptionGroup, PaymentMethod as Method, PaymentRequest, PricingSettings, Product, ProductInput,
    PromoRule, PromoTarget, Promotion, RefundLine, RefundRequest, RoundMode, ShiftCount,
    SplitPayment, TaxSettings, Tender,
};
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn money(value: &str) -> Result<Money> {
    Ok(value.parse()?)
}
fn checked_tax_id(prefix: &str) -> String {
    let sum: u32 = prefix
        .bytes()
        .enumerate()
        .map(|(i, d)| u32::from(d - b'0') * (13 - i as u32))
        .sum();
    format!("{prefix}{}", (11 - sum % 11) % 10)
}
fn tax() -> TaxSettings {
    TaxSettings {
        enabled: true,
        prices_include_vat: false,
        seller_name: "ร้านทดสอบเส้นทางพัฒนา".into(),
        tax_id: checked_tax_id("010555512345"),
        branch: "00000".into(),
        address: "ที่อยู่สมมติ กรุงเทพฯ".into(),
        ..Default::default()
    }
}
fn buyer() -> Buyer {
    Buyer {
        name: "ลูกค้าทดสอบ".into(),
        tax_id: checked_tax_id("010555598765"),
        branch: "00000".into(),
        address: "ที่อยู่สมมติ เชียงใหม่".into(),
    }
}
struct Shop {
    pos: Pos,
    meal: Product,
    shift: Uuid,
    coupon: Promotion,
    cart: Cart,
    at: i64,
    dir: tempfile::TempDir,
}
fn shop() -> Result<Shop> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let at = date!(2026 - 10 - 13)
        .midnight()
        .assume_offset(offset!(+7))
        .unix_timestamp()
        + 23 * 3600
        + 59 * 60;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, money("100")?, at - 60)?;
    pos.set_tax_settings(&tax())?;
    pos.set_pricing_settings(&PricingSettings {
        service_bp: 1000,
        rounding: CashRounding {
            step: 100,
            mode: RoundMode::Down,
        },
    })?;
    let group = Uuid::new_v4();
    let choice = Uuid::new_v4();
    let meal = pos.save_product_at(
        ProductInput {
            sku: "SYNTHETIC-MEAL".into(),
            name: "อาหารทดสอบ".into(),
            price: money("100.01")?,
            cost: Some(money("80.25")?),
            stock: Some(20),
            options: vec![OptionGroup {
                id: group,
                name: "เพิ่ม".into(),
                required: true,
                multiple: false,
                choices: vec![OptionChoice {
                    id: choice,
                    name: "ไข่ทดสอบ".into(),
                    price: money("9.99")?,
                }],
            }],
            ..Default::default()
        },
        at - 50,
    )?;
    let mut coupon = Promotion::new(
        "คูปองทดสอบ 10%",
        PromoRule::Off {
            discount: Discount::Percent(1000),
        },
        PromoTarget::Product(meal.id),
    );
    coupon.coupon = "FLOW10".into();
    coupon.max_uses = Some(2);
    pos.save_promotion(&coupon, at - 40)?;
    let table = pos.save_table(&DiningTable {
        id: Uuid::new_v4(),
        name: "โต๊ะทดสอบ".into(),
        zone: "ทดสอบ".into(),
        seats: 5,
        active: true,
    })?;
    let mut cart = Cart::new();
    cart.table = Some(table.id);
    cart.guests = 5;
    cart.coupons.push("flow10".into());
    cart.lines.push(CartLine {
        product_id: meal.id,
        quantity: 5,
        note: "ไม่เผ็ด".into(),
        options: vec![LineOption {
            group_id: group,
            choice_id: choice,
        }],
        ..Default::default()
    });
    Ok(Shop {
        pos,
        meal,
        shift,
        coupon,
        cart,
        at,
        dir,
    })
}
fn split_payment() -> Result<SplitPayment> {
    Ok(SplitPayment {
        tenders: vec![
            Tender {
                method: Method::Cash,
                amount: money("116.52")?,
            },
            Tender {
                method: Method::Transfer,
                amount: money("349.57")?,
            },
        ],
        cash_given: money("120")?,
    })
}
fn request(sale: Uuid, method: Method, quantity: u32) -> RefundRequest {
    RefundRequest {
        id: Uuid::new_v4(),
        sale_id: sale,
        refund_method: method,
        reason: "คืนสินค้าทดสอบ".into(),
        lines: vec![RefundLine { line: 0, quantity }],
        legacy_restock: None,
    }
}
#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type = Text)]
    receipt_json: String,
}
fn receipt_bytes(conn: &mut SqliteConnection, sale: Uuid) -> Result<String> {
    Ok(
        diesel::sql_query("SELECT receipt_json FROM sales WHERE cart_id = ?")
            .bind::<Text, _>(sale.to_string())
            .get_result::<JsonRow>(conn)?
            .receipt_json,
    )
}
fn close(pos: &mut Pos, id: Uuid, counted: &str, at: i64) -> Result<pos_domain::Shift> {
    let revision = pos.active_shift()?.ok_or("open shift missing")?.revision;
    Ok(pos.close_shift_with(
        id,
        revision,
        &ShiftCount {
            counted: money(counted)?,
            reason: String::new(),
            denominations: vec![],
        },
        at,
    )?)
}

#[test]
fn sent_table_coupon_split_tenders_invoice_refunds_close_and_backup_reopen() -> Result {
    let mut s = shop()?;
    let (sent, ticket) = s.pos.send_to_kitchen(&s.cart, s.at)?;
    let ticket = ticket.ok_or("kitchen ticket")?;
    assert_eq!(ticket.lines[0].quantity, 5);
    assert!(s.pos.send_to_kitchen(&s.cart, s.at + 1)?.1.is_none());
    s.pos.hold(&s.cart, s.at + 2)?; // queued stale autosave cannot erase sent state
    let id = Uuid::new_v4();
    let (rest, moved) = s.pos.split_bill(&s.cart, &[(0, 1)], id, s.at + 3)?;
    assert_eq!((rest.lines[0].quantity, moved.lines[0].quantity), (4, 1));
    assert_eq!((rest.sent[0].quantity, moved.sent[0].quantity), (4, 1));
    assert_eq!(
        rest.sent[0].quantity + moved.sent[0].quantity,
        sent.sent[0].quantity
    );
    assert_eq!(rest.table, s.cart.table);
    assert!(moved.table.is_none());
    assert_eq!(moved.coupons, s.cart.coupons);
    let retry = s.pos.split_bill(&s.cart, &[(0, 1)], id, s.at + 4)?;
    assert_eq!(retry, (rest.clone(), moved.clone()));
    assert!(s.pos.send_to_kitchen(&rest, s.at + 5)?.1.is_none());
    assert!(s.pos.send_to_kitchen(&moved, s.at + 5)?.1.is_none());
    assert_eq!(s.pos.kitchen_tickets()?.len(), 1);
    let small = s.pos.checkout_with_payment(
        &moved,
        PaymentRequest {
            method: Method::Card,
            amount: money("116.52")?,
        },
        Some(s.shift),
        s.at + 6,
    )?;
    let payment = split_payment()?;
    let sale = s
        .pos
        .checkout_split(&rest, &payment, Some(s.shift), s.at + 7)?;
    // Four option-inclusive units: 440 − 44 + 39.60 service + 30.49 VAT.
    assert_eq!(sale.total, money("466.09")?);
    assert_eq!(sale.rounding, 0); // split payment ignores the configured cash rounding
    assert_eq!(sale.lines[0].unit_price, money("110")?);
    assert_eq!(sale.lines[0].unit_cost, Some(money("80.25")?));
    assert_eq!(sale.lines[0].options[0].name, "ไข่ทดสอบ");
    assert_eq!(sale.lines[0].net(), sale.total);
    assert_eq!(sale.service_charge, money("39.60")?);
    assert_eq!(sale.tax.as_ref().ok_or("tax")?.vat, money("30.49")?);
    assert_eq!(sale.promotions[0].amount, money("44")?);
    assert_eq!((sale.cash, sale.change), (money("120")?, money("3.48")?));
    assert!(s.pos.held()?.is_empty()); // paid table becomes free
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(15));
    assert_eq!(
        s.pos
            .checkout_split(&rest, &payment, Some(s.shift), s.at + 8)?
            .number,
        sale.number
    );
    assert_eq!(s.pos.promotions()?[0].uses, 2);
    assert!(
        s.pos
            .activity(&ActivityQuery {
                kind: Some(ActivityKind::Discount),
                ..Default::default()
            })?
            .is_empty()
    );
    let invoice = s.pos.issue_tax_invoice(sale.id, &buyer(), s.at + 9)?;
    assert_eq!(
        s.pos.issue_tax_invoice(sale.id, &buyer(), s.at + 10)?,
        invoice
    );
    let mut conn = SqliteConnection::establish(s.dir.path().join("db").to_str().ok_or("path")?)?;
    let immutable = receipt_bytes(&mut conn, sale.id)?;
    let first_shift = s.pos.active_shift()?.ok_or("shift")?;
    assert_eq!(first_shift.sale_count, 2);
    assert_eq!(first_shift.sales, money("116.52")?);
    assert_eq!(first_shift.transfer_sales, money("349.57")?);
    assert_eq!(first_shift.card_sales, small.total);
    assert_eq!(s.pos.cash_movements(s.shift, 0)?.len(), 3);
    let closed = close(&mut s.pos, s.shift, "216.52", s.at + 11)?;
    assert_eq!(
        closed
            .closing
            .as_ref()
            .ok_or("closing")?
            .difference
            .satang(),
        0
    );
    // An already committed payment returns its immutable result after the shift closes.
    assert_eq!(
        s.pos
            .checkout_split(&rest, &payment, Some(s.shift), s.at + 12)?
            .number,
        sale.number
    );

    let tomorrow = s.at + 61; // crosses Bangkok midnight while UTC is still the prior date
    let refund_shift = Uuid::new_v4();
    s.pos.open_shift(refund_shift, money("200")?, tomorrow)?;
    assert!(matches!(
        s.pos.refund_items(
            &request(sale.id, Method::Card, 1),
            Some(refund_shift),
            tomorrow + 1
        ),
        Err(Error::InvalidRefundMethod)
    ));
    let cash = request(sale.id, Method::Cash, 1);
    let first = s
        .pos
        .refund_items(&cash, Some(refund_shift), tomorrow + 2)?;
    assert_eq!(first.refund, money("116.52")?);
    assert!(matches!(
        s.pos.refund_items(
            &request(sale.id, Method::Cash, 1),
            Some(refund_shift),
            tomorrow + 3
        ),
        Err(Error::RefundExceedsMethod)
    ));
    let transfer = request(sale.id, Method::Transfer, 3);
    let second = s
        .pos
        .refund_items(&transfer, Some(refund_shift), tomorrow + 4)?;
    assert_eq!(second.refund, money("349.57")?);
    assert_eq!(first.refund.checked_add(second.refund)?, sale.total);
    let records = s.pos.history(0)?;
    let record = records
        .iter()
        .find(|r| r.sale.id == sale.id)
        .ok_or("sale")?;
    assert_eq!(record.credit_notes.len(), 2);
    let notes = &record.credit_notes;
    assert_eq!((notes[0].number, notes[1].number), (1, 2));
    assert_eq!(notes[0].source_id, cash.id);
    assert_eq!(notes[1].source_id, transfer.id);
    assert_eq!(notes[0].buyer, invoice.buyer);
    assert_eq!(notes[0].invoice_number, invoice.number);
    assert_eq!(notes[0].total.checked_add(notes[1].total)?, sale.total);
    assert_eq!(notes[0].vat.checked_add(notes[1].vat)?, money("30.49")?);
    assert_eq!(
        notes[0].difference.checked_add(notes[1].difference)?,
        money("435.60")?
    );
    assert_eq!(notes[1].correct_value, Money::ZERO);
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(19));
    assert_eq!(s.pos.promotions()?[0].uses, 2); // refunds do not replenish coupons
    let before = s.pos.daily_summary(date!(2026 - 10 - 13))?;
    let after = s.pos.daily_summary(date!(2026 - 10 - 14))?;
    assert_eq!(
        (before.gross_sales, before.refunds),
        (money("582.61")?, Money::ZERO)
    );
    assert_eq!(
        (before.vat_sales, before.profit.gross_profit.satang()),
        (money("38.11")?, 14325)
    );
    assert_eq!(
        (after.gross_sales, after.refunds, after.net.satang()),
        (Money::ZERO, sale.total, -46609)
    );
    assert_eq!(
        (after.vat_refunds, after.profit.gross_profit.satang()),
        (money("30.49")?, -11460)
    );
    assert_eq!(after.payments.method(Method::Cash).refunds, first.refund);
    assert_eq!(
        after.payments.method(Method::Transfer).refunds,
        second.refund
    );
    assert_eq!(s.pos.cash_movements(refund_shift, 0)?.len(), 2);
    let second_closed = close(&mut s.pos, refund_shift, "83.48", tomorrow + 5)?;
    assert_eq!(second_closed.refund_count, 2);
    assert_eq!(
        s.pos
            .refund_items(&cash, Some(refund_shift), tomorrow + 6)?,
        first
    );
    assert_eq!(
        s.pos
            .refund_items(&transfer, Some(refund_shift), tomorrow + 6)?,
        second
    );
    // Future settings and catalog changes cannot alter receipts or credit notes.
    let notes_before = record.credit_notes.clone();
    s.pos.delete_promotion(s.coupon.id)?;
    s.pos.set_tax_settings(&TaxSettings::default())?;
    s.pos.set_pricing_settings(&PricingSettings::default())?;
    s.pos.save_product_at(
        ProductInput {
            id: Some(s.meal.id),
            sku: s.meal.sku.clone(),
            name: "เปลี่ยนชื่อภายหลัง".into(),
            price: money("999")?,
            cost: Some(money("1")?),
            stock: Some(999),
            keep_tracked_stock: true,
            ..Default::default()
        },
        tomorrow + 7,
    )?;
    assert_eq!(receipt_bytes(&mut conn, sale.id)?, immutable);
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(19));
    let backup = s.dir.path().join("snapshot.sqlite3");
    s.pos.backup(&backup)?;
    let info = pos_core::inspect_database(&backup)?;
    assert_eq!(
        (info.schema, info.sales, info.products, info.held),
        (i64::from(pos_core::SCHEMA_VERSION), 2, 1, 0)
    );
    let mut reopened = Pos::open(&backup)?;
    reopened.validate_recovery_data()?;
    assert!(reopened.active_shift()?.is_none());
    assert_eq!(reopened.shift_history(0)?.len(), 2);
    let restored = reopened.history(0)?;
    let restored = restored
        .iter()
        .find(|r| r.sale.id == sale.id)
        .ok_or("restored")?;
    assert_eq!(restored.credit_notes, notes_before);
    assert_eq!(restored.sale.promotions[0].name, "คูปองทดสอบ 10%");
    assert_eq!(
        reopened.refund_items(&transfer, Some(refund_shift), tomorrow + 8)?,
        second
    );
    let mut snapshot_conn = SqliteConnection::establish(backup.to_str().ok_or("backup path")?)?;
    assert_eq!(receipt_bytes(&mut snapshot_conn, sale.id)?, immutable);
    assert_eq!(
        reopened.daily_summary(date!(2026 - 10 - 14))?.profit,
        after.profit
    );
    Ok(())
}

#[test]
fn credit_note_failure_rolls_back_refund_stock_ledger_and_activity_then_retries_once() -> Result {
    let mut s = shop()?;
    let (_, moved) = s.pos.split_bill(&s.cart, &[(0, 1)], Uuid::new_v4(), s.at)?;
    let sale = s.pos.checkout_with_payment(
        &moved,
        PaymentRequest {
            method: Method::Card,
            amount: money("116.52")?,
        },
        Some(s.shift),
        s.at + 1,
    )?;
    s.pos.issue_tax_invoice(sale.id, &buyer(), s.at + 2)?;
    let back = request(sale.id, Method::Card, 1);
    let before = s.pos.active_shift()?.ok_or("shift")?;
    let count = s.pos.cash_movements(s.shift, 0)?.len();
    let mut conn = SqliteConnection::establish(s.dir.path().join("db").to_str().ok_or("path")?)?;
    let bytes = receipt_bytes(&mut conn, sale.id)?;
    conn.batch_execute("CREATE TRIGGER reject_credit_note BEFORE INSERT ON credit_notes BEGIN SELECT RAISE(ABORT, 'synthetic credit-note failure'); END;")?;
    assert!(s.pos.refund_items(&back, Some(s.shift), s.at + 3).is_err());
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(19));
    assert_eq!(s.pos.active_shift()?.ok_or("shift")?, before);
    assert_eq!(s.pos.cash_movements(s.shift, 0)?.len(), count);
    let record = s.pos.history(0)?.remove(0);
    assert!(record.refunds.is_empty());
    assert!(record.credit_notes.is_empty());
    assert!(
        s.pos
            .activity(&ActivityQuery {
                kind: Some(ActivityKind::Refund),
                ..Default::default()
            })?
            .is_empty()
    );
    conn.batch_execute("DROP TRIGGER reject_credit_note")?;
    let refunded = s.pos.refund_items(&back, Some(s.shift), s.at + 4)?;
    assert_eq!(
        s.pos.refund_items(&back, Some(s.shift), s.at + 5)?,
        refunded
    );
    let record = s.pos.history(0)?.remove(0);
    assert_eq!(record.credit_notes.len(), 1);
    assert_eq!(record.credit_notes[0].number, 1);
    assert_eq!(record.credit_notes[0].total, sale.total);
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(20));
    assert_eq!(s.pos.cash_movements(s.shift, 0)?.len(), count + 1);
    assert_eq!(
        s.pos
            .activity(&ActivityQuery {
                kind: Some(ActivityKind::Refund),
                ..Default::default()
            })?
            .len(),
        1
    );
    assert_eq!(receipt_bytes(&mut conn, sale.id)?, bytes);
    Ok(())
}

#[test]
fn late_coupon_usage_failure_rolls_back_split_receipt_drawer_stock_and_held_kitchen_state() -> Result
{
    let mut s = shop()?;
    s.pos.send_to_kitchen(&s.cart, s.at)?;
    let (rest, moved) = s
        .pos
        .split_bill(&s.cart, &[(0, 1)], Uuid::new_v4(), s.at + 1)?;
    let before = s.pos.active_shift()?.ok_or("shift")?;
    let mut conn = SqliteConnection::establish(s.dir.path().join("db").to_str().ok_or("path")?)?;
    // This failure occurs after receipt insertion, held deletion and every tender's ledger row.
    conn.batch_execute("CREATE TRIGGER reject_coupon_use BEFORE UPDATE OF uses ON promotions BEGIN SELECT RAISE(ABORT, 'synthetic coupon usage failure'); END;")?;
    assert!(
        s.pos
            .checkout_split(&rest, &split_payment()?, Some(s.shift), s.at + 2)
            .is_err()
    );
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(20));
    assert_eq!(s.pos.active_shift()?.ok_or("shift")?, before);
    assert!(s.pos.history(0)?.is_empty());
    assert!(s.pos.cash_movements(s.shift, 0)?.is_empty());
    assert_eq!(s.pos.promotions()?[0].uses, 0);
    let held = s.pos.held()?;
    assert_eq!(held.len(), 2);
    assert_eq!(held.iter().find(|c| c.id == rest.id).ok_or("rest")?, &rest);
    assert_eq!(
        held.iter().find(|c| c.id == moved.id).ok_or("moved")?,
        &moved
    );
    assert!(s.pos.send_to_kitchen(&rest, s.at + 3)?.1.is_none());
    assert_eq!(s.pos.kitchen_tickets()?.len(), 1);
    conn.batch_execute("DROP TRIGGER reject_coupon_use")?;
    let sale = s
        .pos
        .checkout_split(&rest, &split_payment()?, Some(s.shift), s.at + 4)?;
    assert_eq!((sale.number, sale.total), (1, money("466.09")?));
    assert_eq!(s.pos.promotions()?[0].uses, 1);
    assert_eq!(s.pos.product(s.meal.id)?.stock, Some(16));
    assert_eq!(s.pos.cash_movements(s.shift, 0)?.len(), 2);
    assert_eq!(s.pos.held()?, [moved]);
    assert_eq!(
        s.pos
            .checkout_split(&rest, &split_payment()?, Some(s.shift), s.at + 5)?
            .number,
        1
    );
    assert_eq!(s.pos.promotions()?[0].uses, 1);
    Ok(())
}
