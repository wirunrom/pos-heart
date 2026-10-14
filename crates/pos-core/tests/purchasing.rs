use diesel::{Connection, connection::SimpleConnection};
use pos_core::{Error, Pos};
use pos_domain::*;
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn supplier() -> Supplier {
    Supplier {
        id: Uuid::new_v4(),
        name: "ผู้ขายทดสอบ".into(),
        contact: "0900000000".into(),
        note: String::new(),
        active: true,
    }
}
fn setup() -> Result<(tempfile::TempDir, Pos, PurchaseOrderRequest)> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let s = pos.save_supplier(&supplier(), 0)?;
    let p = pos.save_product(ProductInput {
        sku: "P".into(),
        name: "กล่อง".into(),
        price: "20".parse()?,
        stock: Some(2),
        ..Default::default()
    })?;
    let i = pos.save_ingredient(
        &Ingredient {
            id: Uuid::new_v4(),
            name: "นม".into(),
            unit: "ลิตร".into(),
            stock: -500,
            low: 0,
            cost: Some("15".parse()?),
            active: true,
        },
        0,
    )?;
    let r = PurchaseOrderRequest {
        id: Uuid::new_v4(),
        supplier_id: s.id,
        items: vec![
            PurchaseItemInput {
                target: PurchaseTarget::Product(p.id),
                quantity: 3000,
                unit_cost: "10.25".parse()?,
            },
            PurchaseItemInput {
                target: PurchaseTarget::Ingredient(i.id),
                quantity: 1250,
                unit_cost: "31.23".parse()?,
            },
        ],
        note: "รอบเช้า".into(),
    };
    Ok((dir, pos, r))
}
fn receive(o: &PurchaseOrder, items: Vec<(usize, i64)>) -> ReceivePurchaseRequest {
    ReceivePurchaseRequest {
        id: Uuid::new_v4(),
        order_id: o.id,
        revision: o.revision,
        items: items
            .into_iter()
            .map(|(line, quantity)| PurchaseReceiveItem { line, quantity })
            .collect(),
        note: String::new(),
    }
}
#[test]
fn partial_receive_retries_snapshots_cancel_and_backup() -> Result {
    let (dir, mut p, r) = setup()?;
    let o = p.create_purchase_order(&r, 1)?;
    assert_eq!(o.status(), PurchaseStatus::Open);
    assert_eq!(o.total()?.to_string(), "69.79");
    let req = receive(&o, vec![(0, 1000), (1, 250)]);
    let receipt = p.receive_purchase_order(&req, 2)?;
    assert_eq!(receipt.total.to_string(), "18.06");
    assert_eq!(p.receive_purchase_order(&req, 999)?, receipt);
    let mut conflict = req.clone();
    conflict.note = "different".into();
    assert!(matches!(
        p.receive_purchase_order(&conflict, 3),
        Err(Error::ConflictingRequest)
    ));
    let updated = p.purchase_order(o.id)?;
    assert_eq!(updated.status(), PurchaseStatus::Partial);
    assert_eq!(updated.revision, 1);
    assert_eq!(updated.received_total()?, receipt.total);
    let stale = receive(&o, vec![(0, 1000)]);
    assert!(matches!(
        p.receive_purchase_order(&stale, 3),
        Err(Error::PurchaseOrderChanged)
    ));
    let too = receive(&updated, vec![(0, 3000)]);
    assert!(matches!(
        p.receive_purchase_order(&too, 3),
        Err(Error::PurchaseExceedsOrder)
    ));
    let mut s = o.supplier.clone();
    s.name = "newname".into();
    s.active = false;
    p.save_supplier(&s, 3)?;
    assert_eq!(p.purchase_order(o.id)?.supplier.name, o.supplier.name);
    let req2 = receive(&updated, vec![(0, 2000), (1, 1000)]);
    let receipt2 = p.receive_purchase_order(&req2, 4)?;
    let done = p.purchase_order(o.id)?;
    assert_eq!(done.status(), PurchaseStatus::Received);
    assert_eq!(
        done.received_total()?,
        receipt.total.checked_add(receipt2.total)?
    );
    let product = match r.items[0].target {
        PurchaseTarget::Product(id) => p.product(id)?,
        _ => return Err("product".into()),
    };
    assert_eq!(product.stock, Some(5));
    assert_eq!(product.cost, None);
    let ingredients = p.ingredients()?;
    assert_eq!(ingredients[0].stock, 750);
    assert_eq!(ingredients[0].cost, Some("15".parse()?));
    assert!(matches!(
        p.receive_purchase_order(&receive(&done, vec![(0, 1000)]), 5),
        Err(Error::PurchaseOrderClosed)
    ));
    let mut r2 = r.clone();
    r2.id = Uuid::new_v4();
    s.active = true;
    p.save_supplier(&s, 6)?;
    let other = p.create_purchase_order(&r2, 6)?;
    let partial = p.receive_purchase_order(&receive(&other, vec![(1, 250)]), 7)?;
    let other = p.purchase_order(other.id)?;
    let cancel = CancelPurchaseRequest {
        id: Uuid::new_v4(),
        order_id: other.id,
        revision: other.revision,
        reason: "ร้านส่งไม่ครบ".into(),
    };
    let cancelled = p.cancel_purchase_order(&cancel, 8)?;
    assert_eq!(cancelled.status(), PurchaseStatus::Cancelled);
    assert_eq!(cancelled.received_total()?, partial.total);
    assert_eq!(p.cancel_purchase_order(&cancel, 99)?, cancelled);
    let mut wrong = cancel.clone();
    wrong.reason = "changed".into();
    assert!(matches!(
        p.cancel_purchase_order(&wrong, 9),
        Err(Error::ConflictingRequest)
    ));
    p.validate_recovery_data()?;
    p.backup(&dir.path().join("copy"))?;
    drop(p);
    let mut restored = Pos::open(dir.path().join("copy"))?;
    restored.validate_recovery_data()?;
    assert_eq!(restored.purchase_order(o.id)?, done);
    assert_eq!(restored.purchase_receipts(o.id, 0)?.len(), 2);
    Ok(())
}
#[test]
fn atomic_rollback_late_failure_and_overflow() -> Result {
    let (dir, mut p, r) = setup()?;
    let o = p.create_purchase_order(&r, 1)?;
    let mut db =
        diesel::SqliteConnection::establish(dir.path().join("db").to_str().ok_or("path")?)?;
    db.batch_execute("CREATE TRIGGER fail_receive BEFORE INSERT ON purchase_receipts BEGIN SELECT RAISE(ABORT,'injected'); END;")?;
    let request = receive(&o, vec![(0, 1000), (1, 250)]);
    assert!(p.receive_purchase_order(&request, 2).is_err());
    assert_eq!(p.purchase_order(o.id)?, o);
    assert_eq!(p.ingredients()?[0].stock, -500);
    if let PurchaseTarget::Product(id) = r.items[0].target {
        assert_eq!(p.product(id)?.stock, Some(2));
        assert_eq!(p.stock_movements(id, 0)?.len(), 1);
    }
    db.batch_execute("DROP TRIGGER fail_receive;")?;
    p.receive_purchase_order(&request, 3)?;
    let o = p.purchase_order(o.id)?;
    db.batch_execute("UPDATE ingredients SET stock_milli=9223372036854775807;")?;
    let request = receive(&o, vec![(0, 1000), (1, 1)]);
    assert!(matches!(
        p.receive_purchase_order(&request, 4),
        Err(Error::Domain(DomainError::Overflow))
    ));
    assert_eq!(p.purchase_order(o.id)?, o);
    if let PurchaseTarget::Product(id) = r.items[0].target {
        assert_eq!(p.product(id)?.stock, Some(3));
    }
    db.batch_execute("UPDATE ingredients SET stock_milli=-250;")?;
    p.validate_recovery_data()?;
    Ok(())
}
#[test]
fn validation_retry_before_authorization_and_migration() -> Result {
    let (dir, mut p, r) = setup()?;
    let mut bad = r.clone();
    bad.items[0].quantity = 500;
    assert!(p.create_purchase_order(&bad, 1).is_err());
    bad = r.clone();
    bad.items.push(bad.items[0].clone());
    assert!(p.create_purchase_order(&bad, 1).is_err());
    bad = r.clone();
    bad.items[1].quantity = i64::MAX;
    bad.items[1].unit_cost = Money::from_satang(i64::MAX)?;
    assert!(matches!(
        p.create_purchase_order(&bad, 1),
        Err(Error::Domain(DomainError::Overflow))
    ));
    let o = p.create_purchase_order(&r, 1)?;
    let receipt_request = receive(&o, vec![(1, 100)]);
    let receipt = p.receive_purchase_order(&receipt_request, 2)?;
    let owner = p.save_staff(
        &StaffInput {
            id: None,
            name: "Owner".into(),
            role: Some(OWNER.into()),
            pin: Some("9999".into()),
            active: true,
        },
        3,
    )?;
    p.sign_out();
    assert_eq!(p.create_purchase_order(&r, 4)?.id, o.id);
    assert_eq!(p.receive_purchase_order(&receipt_request, 4)?, receipt);
    let fresh = receive(&p.purchase_order(o.id)?, vec![(1, 100)]);
    assert!(matches!(
        p.receive_purchase_order(&fresh, 4),
        Err(Error::SignInRequired)
    ));
    p.sign_in(owner.id, "9999")?;
    let o = p.purchase_order(o.id)?;
    let cancel = CancelPurchaseRequest {
        id: Uuid::new_v4(),
        order_id: o.id,
        revision: o.revision,
        reason: "cancel".into(),
    };
    let cancelled = p.cancel_purchase_order(&cancel, 5)?;
    p.sign_out();
    assert_eq!(p.cancel_purchase_order(&cancel, 6)?, cancelled);
    drop(p);
    // A genuine schema22 database has none of the new tables.
    let mut db =
        diesel::SqliteConnection::establish(dir.path().join("db").to_str().ok_or("path")?)?;
    db.batch_execute("DROP TABLE purchase_cancellations; DROP TABLE purchase_receipts; DROP TABLE purchase_orders; DROP TABLE suppliers; PRAGMA user_version=22;")?;
    drop(db);
    let mut upgraded = Pos::open(dir.path().join("db"))?;
    assert!(upgraded.purchase_orders(0)?.is_empty());
    assert_eq!(upgraded.ingredients()?.len(), 1);
    upgraded.validate_recovery_data()?;
    Ok(())
}
#[test]
fn fractional_costs_and_future_recovery_corruption() -> Result {
    assert_eq!(purchase_cost(Money::from_satang(1)?, 500)?.satang(), 1);
    assert!(purchase_cost(Money::from_satang(i64::MAX)?, 2000).is_err());
    let (dir, mut p, r) = setup()?;
    let o = p.create_purchase_order(&r, 1)?;
    p.receive_purchase_order(&receive(&o, vec![(1, 1)]), 2)?;
    p.validate_recovery_data()?;
    let mut db =
        diesel::SqliteConnection::establish(dir.path().join("db").to_str().ok_or("path")?)?;
    db.batch_execute(
        "UPDATE purchase_orders SET order_json=json_set(order_json,'$.received_cost',999999);",
    )?;
    assert!(p.validate_recovery_data().is_err());
    Ok(())
}
#[test]
fn fractional_receives_allocate_exactly_and_changed_unit_rolls_back() -> Result {
    let (_dir, mut p, mut r) = setup()?;
    r.items[1].unit_cost = Money::from_satang(1)?;
    r.items[1].quantity = 1000;
    let o = p.create_purchase_order(&r, 1)?;
    let first = receive(&o, vec![(1, 500)]);
    let a = p.receive_purchase_order(&first, 2)?;
    assert_eq!(a.total.satang(), 1);
    let o = p.purchase_order(o.id)?;
    let second = receive(&o, vec![(1, 500)]);
    let b = p.receive_purchase_order(&second, 2)?;
    assert_eq!(b.total.satang(), 0);
    assert_eq!(a.total.checked_add(b.total)?.satang(), 1);
    assert_eq!(p.receive_purchase_order(&second, 3)?, b);
    p.validate_recovery_data()?;
    r.id = Uuid::new_v4();
    let o = p.create_purchase_order(&r, 4)?;
    let mut i = p.ingredients()?.remove(0);
    i.unit = "ml".into();
    p.save_ingredient(&i, 5)?;
    let request = receive(&o, vec![(0, 1000), (1, 500)]);
    assert!(matches!(
        p.receive_purchase_order(&request, 6),
        Err(Error::StockChanged)
    ));
    assert_eq!(p.purchase_order(o.id)?, o);
    if let PurchaseTarget::Product(id) = r.items[0].target {
        assert_eq!(p.product(id)?.stock, Some(2));
    }
    Ok(())
}
#[test]
fn cashier_requires_permission_and_retries_after_lock() -> Result {
    let (_dir, mut p, r) = setup()?;
    let o = p.create_purchase_order(&r, 1)?;
    let owner = p.save_staff(
        &StaffInput {
            id: None,
            name: "Owner".into(),
            role: Some(OWNER.into()),
            pin: Some("9999".into()),
            active: true,
        },
        2,
    )?;
    let cashier = p.save_staff(
        &StaffInput {
            id: None,
            name: "Cashier".into(),
            role: Some(CASHIER.into()),
            pin: Some("1111".into()),
            active: true,
        },
        2,
    )?;
    p.sign_in(cashier.id, "1111")?;
    let req = receive(&o, vec![(0, 1000)]);
    assert!(matches!(
        p.receive_purchase_order(&req, 3),
        Err(Error::ApprovalRequired(Permission::Stock))
    ));
    let mut other = r.clone();
    other.id = Uuid::new_v4();
    assert!(matches!(
        p.create_purchase_order(&other, 3),
        Err(Error::ApprovalRequired(Permission::Catalog))
    ));
    assert!(matches!(
        p.save_supplier(&supplier(), 3),
        Err(Error::ApprovalRequired(Permission::Catalog))
    ));
    assert_eq!(p.create_purchase_order(&r, 3)?, o);
    p.sign_in(owner.id, "9999")?;
    let result = p.receive_purchase_order(&req, 4)?;
    p.sign_out();
    assert_eq!(p.receive_purchase_order(&req, 5)?, result);
    Ok(())
}

#[test]
fn concurrent_same_receiving_identity_commits_once() -> Result {
    let (dir, mut p, r) = setup()?;
    let order = p.create_purchase_order(&r, 1)?;
    let request = receive(&order, vec![(0, 1000), (1, 250)]);
    let path = dir.path().join("db");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let path = path.clone();
        let request = request.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(
            move || -> pos_core::Result<PurchaseReceipt> {
                let mut pos = Pos::open(path)?;
                barrier.wait();
                pos.receive_purchase_order(&request, 2)
            },
        ));
    }
    let mut receipts = Vec::new();
    for handle in handles {
        receipts.push(handle.join().map_err(|_| "worker panicked")??);
    }
    assert_eq!(receipts[0], receipts[1]);
    assert_eq!(p.purchase_order(order.id)?.revision, 1);
    assert_eq!(p.purchase_receipts(order.id, 0)?.len(), 1);
    assert_eq!(p.ingredients()?[0].stock, -250);
    if let PurchaseTarget::Product(id) = r.items[0].target {
        assert_eq!(p.product(id)?.stock, Some(3));
        assert_eq!(p.stock_movements(id, 0)?.len(), 2);
    }
    p.validate_recovery_data()?;
    Ok(())
}
