use diesel::{connection::SimpleConnection, prelude::*};
use pos_core::{ActivityKind, ActivityQuery, Pos};
use pos_domain::{
    Cart, CartLine, Discount, Money, Permission, ProductInput, RefundLine, RefundRequest,
    StaffInput, StockKind, StockRequest, VoidRequest,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn events(pos: &mut Pos, kind: ActivityKind) -> pos_core::Result<Vec<pos_core::ActivityEvent>> {
    pos.activity(&ActivityQuery {
        kind: Some(kind),
        ..Default::default()
    })
}
fn product() -> Result<ProductInput> {
    Ok(ProductInput {
        sku: "W_%".into(),
        name: "น้ำดื่ม".into(),
        stock: Some(20),
        price: "10.15".parse()?,
        ..Default::default()
    })
}
#[test]
fn activity_is_persistent_paged_and_filters_are_literal() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let p = pos.save_product_at(product()?, 1)?;
    for i in 0..53 {
        pos.set_category(&[p.id], &format!("หมวด {i}"))?;
    }
    let first = events(&mut pos, ActivityKind::Product)?;
    assert_eq!(first.len(), 50);
    let older = pos.activity(&ActivityQuery {
        kind: Some(ActivityKind::Product),
        offset: 50,
        ..Default::default()
    })?;
    assert_eq!(older.len(), 4);
    assert!(first.last().ok_or("event")?.number > older[0].number);
    let literal = pos.activity(&ActivityQuery {
        search: "_%".into(),
        ..Default::default()
    })?;
    assert_eq!(literal.len(), 1); // SKU snapshot, not LIKE wildcards.
    let saved = literal[0].clone();
    assert_eq!(saved.staff, Default::default());
    drop(pos);
    let mut pos = Pos::open(&path)?;
    let restored = pos.activity(&ActivityQuery {
        search: "_%".into(),
        ..Default::default()
    })?;
    assert_eq!(restored, [saved]);
    assert_eq!(restored[0].detail["after"]["price"], 1015);
    Ok(())
}
#[test]
fn money_and_stock_retries_add_one_activity_each() -> Result {
    let mut pos = Pos::open(":memory:")?;
    let p = pos.save_product_at(product()?, 1)?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 2)?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: p.id,
        quantity: 3,
        ..Default::default()
    });
    cart.discount = Some(Discount::Amount("0.01".parse()?));
    pos.hold(&cart, 3)?;
    let sale = pos.checkout(&cart, "50".parse()?, 4)?;
    assert_eq!(pos.checkout(&cart, "50".parse()?, 5)?.number, sale.number);
    let discounts = events(&mut pos, ActivityKind::Discount)?;
    assert_eq!(discounts.len(), 1);
    assert_eq!(discounts[0].detail["amount_satang"], 1);
    let request = RefundRequest {
        id: Uuid::new_v4(),
        sale_id: sale.id,
        refund_method: sale.payment_method,
        reason: "คืน".into(),
        lines: vec![RefundLine {
            line: 0,
            quantity: 1,
        }],
        legacy_restock: None,
    };
    let refund = pos.refund_items(&request, Some(shift), 6)?;
    assert_eq!(pos.refund_items(&request, Some(shift), 7)?, refund);
    assert_eq!(events(&mut pos, ActivityKind::Refund)?.len(), 1);
    let stock = StockRequest {
        id: Uuid::new_v4(),
        product_id: p.id,
        kind: StockKind::Receive,
        quantity: 4,
        expected: None,
        reason: "รับเข้า".into(),
    };
    pos.adjust_stock(&stock, 8)?;
    pos.adjust_stock(&stock, 9)?;
    assert_eq!(events(&mut pos, ActivityKind::Stock)?.len(), 2); // initial + receive
    let mut second = Cart::new();
    second.lines.push(CartLine {
        product_id: p.id,
        quantity: 1,
        ..Default::default()
    });
    let sale = pos.checkout(&second, "50".parse()?, 10)?;
    let void = VoidRequest {
        sale_id: sale.id,
        reason: "ยกเลิก".into(),
        refund_method: sale.payment_method,
        legacy_restock: None,
    };
    pos.void_sale(&void, 11)?;
    pos.void_sale(&void, 12)?;
    assert_eq!(events(&mut pos, ActivityKind::Void)?.len(), 1);
    Ok(())
}
#[test]
fn audit_failure_rolls_back_checkout_stock_held_and_ledger() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let p = pos.save_product_at(product()?, 1)?;
    pos.open_shift(Uuid::new_v4(), Money::ZERO, 2)?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: p.id,
        quantity: 2,
        ..Default::default()
    });
    cart.discount = Some(Discount::Amount("1.00".parse()?));
    pos.hold(&cart, 3)?;
    let mut conn = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    conn.batch_execute("CREATE TRIGGER reject_audit BEFORE INSERT ON activity_events BEGIN SELECT RAISE(ABORT, 'audit failed'); END;")?;
    assert!(pos.checkout(&cart, "50".parse()?, 4).is_err());
    assert_eq!(pos.product(p.id)?.stock, Some(20));
    assert_eq!(pos.held()?.len(), 1);
    assert!(pos.history(0)?.is_empty());
    assert_eq!(pos.active_shift()?.ok_or("shift")?.sales, Money::ZERO);
    assert!(events(&mut pos, ActivityKind::Discount)?.is_empty());
    conn.batch_execute("DROP TRIGGER reject_audit;")?;
    pos.checkout(&cart, "50".parse()?, 5)?;
    assert_eq!(pos.product(p.id)?.stock, Some(18));
    assert!(pos.held()?.is_empty());
    Ok(())
}
#[test]
fn staff_and_approver_are_snapshots_without_pin_material() -> Result {
    let mut pos = Pos::open(":memory:")?;
    let p = pos.save_product_at(product()?, 1)?;
    let owner = pos.save_staff(
        &StaffInput {
            id: None,
            name: "Owner".into(),
            role: Some(pos_domain::OWNER.into()),
            pin: Some("9999".into()),
            active: true,
        },
        2,
    )?;
    let cashier = pos.save_staff(
        &StaffInput {
            id: None,
            name: "Cashier".into(),
            role: Some(pos_domain::CASHIER.into()),
            pin: Some("1234".into()),
            active: true,
        },
        3,
    )?;
    pos.sign_in(cashier.id, "1234")?;
    pos.approve(owner.id, "9999", Permission::Stock)?;
    pos.adjust_stock(
        &StockRequest {
            id: Uuid::new_v4(),
            product_id: p.id,
            kind: StockKind::Remove,
            quantity: 1,
            expected: None,
            reason: "แตก".into(),
        },
        4,
    )?;
    pos.sign_in(owner.id, "9999")?;
    pos.save_staff(
        &StaffInput {
            id: Some(cashier.id),
            name: "Renamed".into(),
            role: Some(pos_domain::CASHIER.into()),
            pin: None,
            active: true,
        },
        5,
    )?;
    let stock = events(&mut pos, ActivityKind::Stock)?;
    assert_eq!(stock[0].staff.by.as_ref().ok_or("actor")?.name, "Cashier");
    assert_eq!(
        stock[0].staff.approved_by.as_ref().ok_or("approver")?.name,
        "Owner"
    );
    let json = serde_json::to_string(&pos.activity(&ActivityQuery::default())?)?;
    assert!(!json.contains("9999"));
    assert!(!json.contains("1234"));
    assert!(!json.contains("pin_hash"));
    let mut manager = pos
        .roles()?
        .into_iter()
        .find(|r| r.id == pos_domain::MANAGER)
        .ok_or("role")?;
    manager.name = "Supervisor".into();
    pos.save_role(&manager)?;
    assert_eq!(
        events(&mut pos, ActivityKind::Role)?[0].detail["after"]["name"],
        "Supervisor"
    );
    Ok(())
}
