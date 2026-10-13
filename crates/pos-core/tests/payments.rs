use diesel::{connection::SimpleConnection, prelude::*, sql_types::Text};
use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, CashKind, CashMovement, Money, PaymentMethod as Method, PaymentRequest,
    ProductInput, Sale, VoidRequest,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn item(pos: &mut Pos) -> Result<pos_domain::Product> {
    Ok(pos.save_product(ProductInput {
        id: None,
        sku: Uuid::new_v4().to_string(),
        name: "Payment fixture".into(),
        category: String::new(),
        price: "10.25".parse()?,
        stock: Some(20),
        ..Default::default()
    })?)
}
fn cart(id: Uuid) -> Cart {
    let mut c = Cart::new();
    c.lines.push(CartLine {
        product_id: id,
        quantity: 1,
        note: String::new(),
        ..Default::default()
    });
    c
}
fn pay(pos: &mut Pos, cart: &Cart, method: Method, shift: Uuid, now: i64) -> Result<Sale> {
    Ok(pos.checkout_with_payment(
        cart,
        PaymentRequest {
            method,
            amount: "10.25".parse()?,
        },
        Some(shift),
        now,
    )?)
}
fn refund(sale: &Sale) -> VoidRequest {
    VoidRequest {
        sale_id: sale.id,
        refund_method: sale.payment_method,
        reason: "Return".into(),
        legacy_restock: None,
    }
}
fn conn(path: &std::path::Path) -> Result<SqliteConnection> {
    Ok(SqliteConnection::establish(path.to_str().ok_or("path")?)?)
}
#[test]
fn all_methods_are_exact_and_only_cash_changes_the_drawer() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let p = item(&mut pos)?;
    let id = Uuid::new_v4();
    pos.open_shift(id, "100".parse()?, 0)?;
    let cash = pos.checkout_for_shift(&cart(p.id), "20".parse()?, Some(id), 1)?;
    let transfer = pay(&mut pos, &cart(p.id), Method::Transfer, id, 2)?;
    let card = pay(&mut pos, &cart(p.id), Method::Card, id, 3)?;
    assert_eq!(cash.change.to_string(), "9.75");
    for sale in [&transfer, &card] {
        assert_eq!(sale.cash, Money::ZERO);
        assert_eq!(sale.change, Money::ZERO);
    }
    let shift = pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.expected()?.to_string(), "110.25");
    assert_eq!(shift.sale_count, 3);
    assert_eq!(shift.transfer_sales, transfer.total);
    assert_eq!(shift.card_sales, card.total);
    assert_eq!(pos.products("", 0)?[0].stock, Some(17));
    let report = pos.daily_summary(time::macros::date!(1970 - 01 - 01))?;
    assert_eq!(report.gross_sales.to_string(), "30.75");
    assert_eq!(report.cash_received.to_string(), "20.00");
    assert_eq!(report.change_given.to_string(), "9.75");
    for method in [Method::Cash, Method::Transfer, Method::Card] {
        assert_eq!(report.payments.method(method).sales.to_string(), "10.25");
    }
    let ledger = pos.cash_movements(id, 0)?;
    assert_eq!(
        ledger
            .iter()
            .filter(|m| m.payment_method == Method::Cash)
            .count(),
        1
    );
    assert_eq!(
        ledger
            .iter()
            .filter(|m| m.payment_method == Method::Transfer)
            .count(),
        1
    );
    assert_eq!(
        ledger
            .iter()
            .filter(|m| m.payment_method == Method::Card)
            .count(),
        1
    );
    pos.backup(&dir.path().join("backup"))?;
    assert_eq!(
        Pos::open(dir.path().join("backup"))?.history(0)?[0]
            .sale
            .payment_method,
        Method::Card
    );
    drop(pos);
    assert_eq!(
        Pos::open(path)?
            .active_shift()?
            .ok_or("shift")?
            .expected()?
            .to_string(),
        "110.25"
    );
    Ok(())
}
#[test]
fn original_method_refunds_land_in_current_shift_and_activity_day_with_safe_retries() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let p = item(&mut pos)?;
    let id = Uuid::new_v4();
    pos.open_shift(id, "100".parse()?, 0)?;
    let c = cart(p.id);
    let sale = pay(&mut pos, &c, Method::Transfer, id, 1)?;
    let old = pos.active_shift()?.ok_or("shift")?;
    let closed = pos.close_shift(id, old.revision, "100".parse()?, 2)?;
    // Cart identity returns its original immutable payment, even after closing.
    assert_eq!(
        pay(&mut pos, &c, Method::Card, id, 3)?.payment_method,
        Method::Transfer
    );
    let next = Uuid::new_v4();
    pos.open_shift(next, "50".parse()?, 86400)?;
    let mut request = refund(&sale);
    request.refund_method = Method::Cash;
    assert!(matches!(
        pos.void_sale_for_shift(&request, Some(next), 86401),
        Err(Error::InvalidRefundMethod)
    ));
    assert_eq!(pos.products("", 0)?[0].stock, Some(19));
    request.refund_method = Method::Transfer;
    let audit = pos.void_sale_for_shift(&request, Some(next), 86401)?;
    request.refund_method = Method::Card;
    assert_eq!(
        pos.void_sale_for_shift(&request, Some(next), 86402)?.id,
        audit.id
    );
    let current = pos.active_shift()?.ok_or("shift")?;
    assert_eq!(current.transfer_refunds.to_string(), "10.25");
    assert_eq!(current.expected()?.to_string(), "50.00");
    assert_eq!(current.refund_count, 1);
    assert_eq!(pos.shift_history(0)?[0], closed);
    let report = pos.daily_summary(time::macros::date!(1970 - 01 - 02))?;
    assert_eq!(report.net.to_string(), "-10.25");
    assert_eq!(report.payments.transfer.net().to_string(), "-10.25");
    assert_eq!(report.payments.cash.refunds, Money::ZERO);
    assert_eq!(pos.products("", 0)?[0].stock, Some(20));
    assert_eq!(pos.cash_movements(next, 0)?.len(), 1);
    Ok(())
}
#[test]
fn gates_and_stale_review_roll_back_and_manual_cash_cannot_be_noncash() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let p = item(&mut pos)?;
    let c = cart(p.id);
    let id = Uuid::new_v4();
    assert!(
        matches!(pay(&mut pos,&c,Method::Card,id,1),Err(e) if e.downcast_ref::<Error>().is_some_and(|e|matches!(e,Error::NoOpenShift)))
    );
    pos.open_shift(id, Money::ZERO, 0)?;
    pos.hold(&c, 1)?;
    assert!(matches!(
        pos.checkout_with_payment(
            &c,
            PaymentRequest {
                method: Method::Card,
                amount: p.price
            },
            Some(Uuid::new_v4()),
            1
        ),
        Err(Error::ShiftChanged)
    ));
    let updated = pos.save_product(ProductInput {
        id: Some(p.id),
        sku: p.sku,
        name: p.name,
        category: p.category,
        price: "12.50".parse()?,
        stock: p.stock,
        ..Default::default()
    })?;
    assert!(matches!(
        pos.checkout_with_payment(
            &c,
            PaymentRequest {
                method: Method::Transfer,
                amount: p.price
            },
            Some(id),
            2
        ),
        Err(Error::PaymentTotalChanged)
    ));
    assert_eq!(pos.products("", 0)?[0].stock, Some(20));
    assert_eq!(pos.held()?.len(), 1);
    assert!(pos.history(0)?.is_empty());
    assert_eq!(pos.active_shift()?.ok_or("shift")?.revision, 0);
    let movement = CashMovement {
        id: Uuid::new_v4(),
        shift_id: id,
        created_at: 2,
        kind: CashKind::CashIn,
        amount: "1".parse()?,
        reason: "cash in".into(),
        payment_method: Method::Card,
        staff: Default::default(),
    };
    assert!(matches!(
        pos.move_cash(&movement),
        Err(Error::InvalidCashMovement)
    ));
    pos.checkout_with_payment(
        &c,
        PaymentRequest {
            method: Method::Transfer,
            amount: updated.price,
        },
        Some(id),
        3,
    )?;
    assert!(matches!(
        pos.close_shift(id, 0, Money::ZERO, 4),
        Err(Error::ShiftChanged)
    ));
    Ok(())
}
#[test]
fn noncash_ledger_failures_roll_back_checkout_and_refund_together() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let p = item(&mut pos)?;
    let c = cart(p.id);
    let id = Uuid::new_v4();
    pos.open_shift(id, Money::ZERO, 0)?;
    pos.hold(&c, 0)?;
    let mut db = conn(&path)?;
    db.batch_execute("CREATE TRIGGER fail_ledger BEFORE INSERT ON cash_movements BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;
    assert!(pay(&mut pos, &c, Method::Card, id, 1).is_err());
    assert_eq!(pos.products("", 0)?[0].stock, Some(20));
    assert_eq!(pos.held()?.len(), 1);
    assert!(pos.history(0)?.is_empty());
    assert_eq!(pos.active_shift()?.ok_or("shift")?.card_sales, Money::ZERO);
    db.batch_execute("DROP TRIGGER fail_ledger;")?;
    let sale = pay(&mut pos, &c, Method::Card, id, 2)?;
    db.batch_execute("CREATE TRIGGER fail_ledger BEFORE INSERT ON cash_movements BEGIN SELECT RAISE(ABORT,'fixture'); END;")?;
    assert!(
        pos.void_sale_for_shift(&refund(&sale), Some(id), 3)
            .is_err()
    );
    assert_eq!(pos.products("", 0)?[0].stock, Some(19));
    assert!(pos.history(0)?[0].cancellation.is_none());
    assert_eq!(
        pos.active_shift()?.ok_or("shift")?.card_refunds,
        Money::ZERO
    );
    db.batch_execute("DROP TRIGGER fail_ledger;")?;
    pos.void_sale_for_shift(&refund(&sale), Some(id), 4)?;
    assert_eq!(pos.products("", 0)?[0].stock, Some(20));
    assert_eq!(pos.cash_movements(id, 0)?.len(), 2);
    Ok(())
}
#[test]
fn version_three_json_stays_byte_identical_and_legacy_defaults_to_cash() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let p = item(&mut pos)?;
    let id = Uuid::new_v4();
    pos.open_shift(id, Money::ZERO, 0)?;
    let sale = pos.checkout(&cart(p.id), p.price, 1)?;
    pos.void_sale(&refund(&sale), 2)?;
    let shift = pos.active_shift()?.ok_or("shift")?;
    pos.close_shift(id, shift.revision, Money::ZERO, 3)?;
    drop(pos);
    let legacy_path = dir.path().join("legacy");
    let mut db = conn(&legacy_path)?;
    db.batch_execute(include_str!("../migrations/001_initial.sql"))?;
    db.batch_execute(include_str!("../migrations/002_sale_voids.sql"))?;
    db.batch_execute(include_str!("../migrations/003_shifts.sql"))?;
    diesel::sql_query("ATTACH DATABASE ? AS fixture")
        .bind::<Text, _>(path.to_str().ok_or("path")?)
        .execute(&mut db)?;
    db.batch_execute("INSERT INTO products SELECT id,sku,name,category,price_satang,stock,active FROM fixture.products;
        INSERT INTO sales SELECT number,cart_id,created_at,label,total_satang,cash_satang,change_satang,json_remove(receipt_json,'$.payment_method') FROM fixture.sales;
        INSERT INTO sale_voids SELECT sale_id,id,created_at,reason,refund_satang,json_remove(void_json,'$.refund_method') FROM fixture.sale_voids;
        INSERT INTO shifts SELECT id,opened_at,closed_at,json_remove(shift_json,'$.transfer_sales','$.card_sales','$.transfer_refunds','$.card_refunds') FROM fixture.shifts;
        INSERT INTO cash_movements SELECT id,shift_id,created_at,json_remove(movement_json,'$.payment_method') FROM fixture.cash_movements;
        DETACH DATABASE fixture;")?;
    #[derive(QueryableByName)]
    struct Json {
        #[diesel(sql_type=Text)]
        json: String,
    }
    fn snapshots(db: &mut SqliteConnection) -> Result<Vec<String>> {
        Ok(diesel::sql_query("SELECT receipt_json AS json FROM sales UNION ALL SELECT void_json FROM sale_voids UNION ALL SELECT shift_json FROM shifts UNION ALL SELECT movement_json FROM cash_movements").load::<Json>(db)?.into_iter().map(|j|j.json).collect())
    }
    db.batch_execute("UPDATE sqlite_sequence SET seq=80 WHERE name='sales';")?;
    let before = snapshots(&mut db)?;
    let mut pos = Pos::open(&legacy_path)?;
    let record = &pos.history(0)?[0];
    assert_eq!(record.sale.payment_method, Method::Cash);
    assert_eq!(
        record.cancellation.as_ref().ok_or("void")?.refund_method,
        Method::Cash
    );
    assert_eq!(pos.shift_history(0)?[0].card_sales, Money::ZERO);
    assert!(
        pos.cash_movements(id, 0)?
            .iter()
            .all(|m| m.payment_method == Method::Cash)
    );
    assert_eq!(snapshots(&mut db)?, before);
    assert!(
        db.batch_execute("UPDATE sales SET payment_method='crypto'")
            .is_err()
    );
    assert!(
        db.batch_execute("UPDATE sale_voids SET refund_method='crypto'")
            .is_err()
    );
    assert!(
        db.batch_execute("UPDATE cash_movements SET payment_method='crypto'")
            .is_err()
    );
    let next = Uuid::new_v4();
    pos.open_shift(next, Money::ZERO, 4)?;
    assert_eq!(pos.checkout(&cart(p.id), p.price, 5)?.number, 81);
    // A newly written void still resolves the rebuilt foreign key.
    let last = pos.history(0)?.remove(0).sale;
    pos.void_sale(&refund(&last), 6)?;
    Ok(())
}

#[test]
fn noncash_bucket_overflow_refuses_without_partial_stock_or_ledger() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let id = Uuid::new_v4();
    pos.open_shift(id, Money::ZERO, 0)?;
    let large = pos.save_product(ProductInput {
        id: None,
        sku: "MAX".into(),
        name: "Max".into(),
        category: String::new(),
        price: Money::from_satang(i64::MAX)?,
        stock: Some(2),
        ..Default::default()
    })?;
    pos.checkout_with_payment(
        &cart(large.id),
        PaymentRequest {
            method: Method::Card,
            amount: large.price,
        },
        Some(id),
        1,
    )?;
    let cent = pos.save_product(ProductInput {
        id: None,
        sku: "CENT".into(),
        name: "Cent".into(),
        category: String::new(),
        price: "0.01".parse()?,
        stock: Some(2),
        ..Default::default()
    })?;
    assert!(matches!(
        pos.checkout_with_payment(
            &cart(cent.id),
            PaymentRequest {
                method: Method::Card,
                amount: cent.price
            },
            Some(id),
            2
        ),
        Err(Error::Domain(pos_domain::DomainError::Overflow))
    ));
    let shift = pos.active_shift()?.ok_or("shift")?;
    assert_eq!(shift.card_sales, large.price);
    assert_eq!(shift.expected()?, pos_domain::SignedMoney::from_satang(0));
    assert_eq!(shift.sale_count, 1);
    assert_eq!(pos.cash_movements(id, 0)?.len(), 1);
    assert_eq!(pos.products("CENT", 0)?[0].stock, Some(2));
    Ok(())
}
