use diesel::{connection::SimpleConnection, prelude::*};
use pos_core::{Error, Pos};
use pos_domain::{Cart, CartLine, CashKind, CashMovement, Money, ProductInput, VoidRequest};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn bill(pos: &mut Pos) -> Result<Cart> {
    let p = pos.save_product(ProductInput {
        id: None,
        sku: Uuid::new_v4().to_string(),
        name: "สินค้า".into(),
        category: String::new(),
        price: "10.25".parse()?,
        stock: Some(5),
        ..Default::default()
    })?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: p.id,
        quantity: 1,
        note: String::new(),
        ..Default::default()
    });
    Ok(cart)
}
fn movement(shift_id: Uuid, kind: CashKind, amount: &str) -> Result<CashMovement> {
    Ok(CashMovement {
        id: Uuid::new_v4(),
        shift_id,
        created_at: 102,
        kind,
        amount: amount.parse()?,
        reason: "เติมทอน / ค่าใช้จ่าย".into(),
        payment_method: pos_domain::PaymentMethod::Cash,
        staff: Default::default(),
    })
}
fn refund(sale_id: Uuid) -> VoidRequest {
    VoidRequest {
        sale_id,
        refund_method: pos_domain::PaymentMethod::Cash,
        reason: "ผิดรายการ".into(),
        legacy_restock: None,
    }
}
#[test]
fn cash_gate_retained_change_reconciliation_and_backup_are_exact() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let cart = bill(&mut pos)?;
    pos.hold(&cart, 90)?;
    assert!(matches!(
        pos.checkout(&cart, "100".parse()?, 100),
        Err(Error::NoOpenShift)
    ));
    assert_eq!(pos.products("", 0)?[0].stock, Some(5));
    assert_eq!(pos.held()?.len(), 1);
    let id = Uuid::new_v4();
    let initial = pos.open_shift(id, "500".parse()?, 100)?;
    assert_eq!(pos.open_shift(id, initial.opening, 101)?.id, id);
    assert!(matches!(
        pos.open_shift(id, "501".parse()?, 101),
        Err(Error::ConflictingRequest)
    ));
    assert!(matches!(
        pos.open_shift(Uuid::new_v4(), Money::ZERO, 101),
        Err(Error::ShiftAlreadyOpen)
    ));
    let sale = pos.checkout_for_shift(&cart, "100".parse()?, Some(id), 101)?;
    assert_eq!(sale.change.to_string(), "89.75");
    let cash_in = movement(id, CashKind::CashIn, "20.50")?;
    let cash_out = movement(id, CashKind::CashOut, "10")?;
    pos.move_cash(&cash_in)?;
    pos.move_cash(&cash_in)?;
    pos.move_cash(&cash_out)?;
    let shift = pos.active_shift()?.ok_or("no shift")?;
    assert_eq!(shift.expected()?.to_string(), "520.75");
    assert_eq!(shift.sale_count, 1);
    assert_eq!(pos.cash_movements(id, 0)?.len(), 3);
    let closed = pos.close_shift(id, shift.revision, "520".parse()?, 103)?;
    assert_eq!(
        closed
            .closing
            .as_ref()
            .ok_or("not closed")?
            .difference
            .to_string(),
        "-0.75"
    );
    assert_eq!(
        pos.close_shift(id, shift.revision, "520".parse()?, 104)?,
        closed
    );
    assert!(matches!(
        pos.close_shift(id, shift.revision, "521".parse()?, 104),
        Err(Error::ConflictingRequest)
    ));
    assert_eq!(pos.checkout(&cart, Money::ZERO, 105)?.id, sale.id); // lost payment response after closing
    assert!(matches!(
        pos.void_sale(&refund(sale.id), 105),
        Err(Error::NoOpenShift)
    ));
    pos.move_cash(&cash_in)?; // lost movement response after closing
    assert!(
        pos.move_cash(&movement(id, CashKind::CashIn, "1")?)
            .is_err()
    );
    assert!(pos.active_shift()?.is_none());
    let backup = dir.path().join("backup");
    pos.backup(&backup)?;
    assert_eq!(Pos::open(backup)?.shift_history(0)?[0], closed);
    assert_eq!(Pos::open(path)?.cash_movements(id, 0)?.len(), 3);
    Ok(())
}
#[test]
fn prior_shift_refund_belongs_to_current_and_retries_never_double_count() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let cart = bill(&mut pos)?;
    pos.open_shift(Uuid::new_v4(), Money::ZERO, 100)?;
    let sale = pos.checkout(&cart, "20".parse()?, 101)?;
    let first = pos.active_shift()?.ok_or("missing")?;
    pos.close_shift(first.id, first.revision, sale.total, 102)?;
    let second = pos.open_shift(Uuid::new_v4(), "100".parse()?, 103)?;
    assert!(matches!(
        pos.checkout_for_shift(
            &bill(&mut Pos::open(dir.path().join("db"))?)?,
            "20".parse()?,
            Some(first.id),
            104
        ),
        Err(Error::ShiftChanged)
    ));
    let void = pos.void_sale_for_shift(&refund(sale.id), Some(second.id), 104)?;
    let current = pos.active_shift()?.ok_or("missing")?;
    assert_eq!(current.sales, Money::ZERO);
    assert_eq!(current.refunds, sale.total);
    assert_eq!(current.expected()?.to_string(), "89.75");
    pos.close_shift(current.id, current.revision, "90".parse()?, 105)?;
    assert_eq!(pos.void_sale(&refund(sale.id), 106)?.id, void.id);
    assert_eq!(pos.cash_movements(second.id, 0)?.len(), 1);
    assert_eq!(
        pos.shift_history(0)?[0]
            .closing
            .as_ref()
            .ok_or("missing")?
            .difference
            .to_string(),
        "0.25"
    );
    Ok(())
}
#[test]
fn stale_close_invalid_reason_clock_and_overflow_are_recoverable() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = pos.open_shift(Uuid::new_v4(), Money::from_satang(i64::MAX)?, 100)?;
    let mut m = movement(shift.id, CashKind::CashIn, "0.01")?;
    assert!(matches!(
        pos.move_cash(&m),
        Err(Error::Domain(pos_domain::DomainError::Overflow))
    ));
    assert!(pos.cash_movements(shift.id, 0)?.is_empty());
    m.kind = CashKind::CashOut;
    m.reason.clear();
    assert!(matches!(pos.move_cash(&m), Err(Error::InvalidCashMovement)));
    m.reason = "เหตุผล".into();
    m.created_at = 99;
    assert!(matches!(pos.move_cash(&m), Err(Error::InvalidShiftTime)));
    m.created_at = 102;
    pos.move_cash(&m)?;
    assert!(matches!(
        pos.close_shift(shift.id, 0, Money::ZERO, 103),
        Err(Error::ShiftChanged)
    ));
    assert!(matches!(
        pos.close_shift(shift.id, 1, Money::ZERO, 101),
        Err(Error::InvalidShiftTime)
    ));
    let mut conflict = m.clone();
    conflict.amount = "1".parse()?;
    assert!(matches!(
        pos.move_cash(&conflict),
        Err(Error::ConflictingRequest)
    ));
    pos.close_shift(shift.id, 1, Money::ZERO, 103)?;
    assert!(matches!(
        pos.open_shift(Uuid::new_v4(), Money::ZERO, 102),
        Err(Error::InvalidShiftTime)
    ));
    Ok(())
}
#[test]
fn ledger_failure_rolls_back_receipt_stock_held_bill_and_refund() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let cart = bill(&mut pos)?;
    let shift = pos.open_shift(Uuid::new_v4(), "100".parse()?, 100)?;
    pos.hold(&cart, 100)?;
    let mut conn = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    conn.batch_execute("CREATE TRIGGER reject_cash BEFORE INSERT ON cash_movements BEGIN SELECT RAISE(ABORT, 'test'); END;")?;
    assert!(pos.checkout(&cart, "20".parse()?, 101).is_err());
    assert!(pos.history(0)?.is_empty());
    assert_eq!(pos.held()?.len(), 1);
    assert_eq!(pos.products("", 0)?[0].stock, Some(5));
    assert_eq!(pos.active_shift()?.ok_or("missing")?, shift);
    conn.batch_execute("DROP TRIGGER reject_cash;")?;
    let sale = pos.checkout(&cart, "20".parse()?, 101)?;
    conn.batch_execute("CREATE TRIGGER reject_cash BEFORE INSERT ON cash_movements BEGIN SELECT RAISE(ABORT, 'test'); END;")?;
    assert!(pos.void_sale(&refund(sale.id), 102).is_err());
    assert!(pos.history(0)?[0].cancellation.is_none());
    assert_eq!(pos.products("", 0)?[0].stock, Some(4));
    assert_eq!(pos.active_shift()?.ok_or("missing")?.refund_count, 0);
    Ok(())
}
#[test]
fn concurrent_open_and_close_payment_race_preserve_one_drawer() -> Result {
    use std::sync::{Arc, Barrier};
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let cart = bill(&mut pos)?;
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = vec![];
    for _ in 0..2 {
        let path = path.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || -> pos_core::Result<_> {
            let mut pos = Pos::open(path)?;
            barrier.wait();
            pos.open_shift(Uuid::new_v4(), Money::ZERO, 100)
        }));
    }
    let results: Vec<_> = workers.into_iter().map(|w| w.join()).collect();
    assert_eq!(results.iter().filter(|r| matches!(r, Ok(Ok(_)))).count(), 1);
    let shift = pos.active_shift()?.ok_or("missing")?;
    let barrier = Arc::new(Barrier::new(2));
    let p = path.clone();
    let b = barrier.clone();
    let id = shift.id;
    let payment = std::thread::spawn(move || -> pos_core::Result<_> {
        let mut pos = Pos::open(p)?;
        b.wait();
        pos.checkout_for_shift(&cart, "20".parse()?, Some(id), 101)
    });
    let mut closer = Pos::open(&path)?;
    barrier.wait();
    let closed = closer.close_shift(id, 0, Money::ZERO, 101);
    let paid = payment.join().map_err(|_| "panic")?;
    assert_ne!(closed.is_ok(), paid.is_ok());
    if closed.is_ok() {
        assert!(pos.history(0)?.is_empty());
        assert!(pos.active_shift()?.is_none());
    } else {
        assert!(matches!(closed, Err(Error::ShiftChanged)));
        assert_eq!(pos.active_shift()?.ok_or("missing")?.sale_count, 1);
    }
    Ok(())
}
#[test]
fn history_and_cash_records_page_without_losing_old_audit() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    for t in 0..51 {
        let shift = pos.open_shift(Uuid::new_v4(), Money::ZERO, t)?;
        pos.close_shift(shift.id, 0, Money::ZERO, t)?;
    }
    assert_eq!(pos.shift_history(0)?.len(), 50);
    assert_eq!(pos.shift_history(50)?.len(), 1);
    let shift = pos.open_shift(Uuid::new_v4(), Money::ZERO, 100)?;
    for _ in 0..51 {
        pos.move_cash(&movement(shift.id, CashKind::CashIn, "0.01")?)?;
    }
    assert_eq!(pos.cash_movements(shift.id, 0)?.len(), 50);
    assert_eq!(pos.cash_movements(shift.id, 50)?.len(), 1);
    assert_eq!(
        pos.active_shift()?
            .ok_or("missing")?
            .expected()?
            .to_string(),
        "0.51"
    );
    Ok(())
}

#[test]
fn version_two_migration_keeps_old_receipt_bytes_and_does_not_fabricate_shifts() -> Result {
    use diesel::sql_types::Text;
    #[derive(QueryableByName)]
    struct Receipt {
        #[diesel(sql_type = Text)]
        receipt_json: String,
    }
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("v2");
    let mut conn = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    conn.batch_execute(include_str!("../migrations/001_initial.sql"))?;
    conn.batch_execute(include_str!("../migrations/002_sale_voids.sql"))?;
    let id = Uuid::new_v4();
    let json = serde_json::json!({"id":id,"number":1,"created_at":1,"label":"legacy",
        "lines":[{"product_id":Uuid::new_v4(),"sku":"OLD","name":"Legacy food", "unit_price":1025,"quantity":1,"note":"", "total":1025,"stock_tracked":false}],"total":1025,"cash":2000,"change":975})
    .to_string();
    diesel::sql_query("INSERT INTO sales(cart_id,created_at,label,total_satang,cash_satang,change_satang,receipt_json) VALUES(?,1,'legacy',1025,2000,975,?)")
        .bind::<Text,_>(id.to_string()).bind::<Text,_>(&json).execute(&mut conn)?;
    let mut pos = Pos::open(&path)?;
    assert!(pos.active_shift()?.is_none());
    assert!(pos.shift_history(0)?.is_empty());
    assert_eq!(
        diesel::sql_query("SELECT receipt_json FROM sales")
            .get_result::<Receipt>(&mut conn)?
            .receipt_json,
        json
    );
    let shift = pos.open_shift(Uuid::new_v4(), Money::ZERO, 100)?;
    pos.void_sale(&refund(id), 101)?;
    assert_eq!(
        pos.active_shift()?
            .ok_or("missing")?
            .expected()?
            .to_string(),
        "-10.25"
    );
    let closed = pos.close_shift(shift.id, 1, Money::ZERO, 102)?;
    assert_eq!(
        closed.closing.ok_or("closing")?.difference.to_string(),
        "10.25"
    );
    Ok(())
}

#[test]
fn each_register_has_its_own_shift_and_drawer()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use pos_domain::{Cart, CartLine, MAIN_REGISTER, PaymentMethod, PaymentRequest};
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shop.sqlite3");
    let mut pos = Pos::open(&path)?;
    assert_eq!(pos.register(), MAIN_REGISTER);
    assert_eq!(pos.registers()?.len(), 1);
    let second = uuid::Uuid::new_v4();
    let added = pos.add_register(second, "เคาน์เตอร์ 2", 10)?;
    // A retried add returns the same register; another name is a conflict.
    assert_eq!(pos.add_register(second, "เคาน์เตอร์ 2", 11)?.id, added.id);
    assert!(matches!(
        pos.add_register(second, "อื่น", 12),
        Err(pos_core::Error::ConflictingRequest)
    ));
    assert!(matches!(
        pos.use_register(uuid::Uuid::new_v4()),
        Err(pos_core::Error::RegisterNotFound)
    ));
    let product = pos.save_product(pos_domain::ProductInput {
        sku: "A".into(),
        name: "a".into(),
        price: "10".parse()?,
        stock: Some(5),
        ..Default::default()
    })?;
    let main_shift = uuid::Uuid::new_v4();
    pos.open_shift(main_shift, "100".parse()?, 20)?;
    assert!(matches!(
        pos.open_shift(uuid::Uuid::new_v4(), "1".parse()?, 21),
        Err(pos_core::Error::ShiftAlreadyOpen)
    ));
    // The second register opens its own shift while the main one is open.
    pos.use_register(second)?;
    assert!(pos.active_shift()?.is_none());
    let second_shift = uuid::Uuid::new_v4();
    pos.open_shift(second_shift, "50".parse()?, 22)?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: product.id,
        quantity: 2,
        ..Default::default()
    });
    pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: "20".parse()?,
        },
        Some(second_shift),
        23,
    )?;
    // Paying against the other register's shift is refused.
    let mut other = Cart::new();
    other.lines = cart.lines.clone();
    assert!(
        pos.checkout_with_payment(
            &other,
            PaymentRequest {
                method: PaymentMethod::Cash,
                amount: "20".parse()?,
            },
            Some(main_shift),
            24,
        )
        .is_err()
    );
    let s2 = pos.active_shift()?.ok_or("second shift")?;
    assert_eq!(s2.register_id, second);
    assert_eq!(s2.expected()?.to_string(), "70.00");
    pos.use_register(MAIN_REGISTER)?;
    let s1 = pos.active_shift()?.ok_or("main shift")?;
    assert_eq!(s1.id, main_shift);
    assert_eq!(s1.expected()?.to_string(), "100.00");
    // Stock is shared by the shop.
    assert_eq!(pos.products("", 0)?[0].stock, Some(3));
    // A reopened connection sells for the main register again.
    drop(pos);
    let mut reopened = Pos::open(&path)?;
    assert_eq!(reopened.active_shift()?.ok_or("main")?.id, main_shift);
    Ok(())
}

#[test]
fn pre_register_shift_snapshots_belong_to_the_main_register()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    // A snapshot written before registers existed has no register_id.
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let current = pos.open_shift(Uuid::new_v4(), Money::ZERO, 1)?;
    let mut json = serde_json::to_value(&current)?;
    json.as_object_mut().ok_or("object")?.remove("register_id");
    let legacy = serde_json::to_string(&json)?;
    let shift: pos_domain::Shift = serde_json::from_str(&legacy)?;
    assert_eq!(shift.register_id, pos_domain::MAIN_REGISTER);
    Ok(())
}
