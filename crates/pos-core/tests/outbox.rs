//! Cloud outbox: writes queue their records in the same transaction,
//! mutable records keep one pending row, acknowledged rows leave, deleted
//! records are sent as None, PIN hashes never leave, and a relink requeues
//! everything.
use pos_core::{OutboxItem, Pos};
use pos_domain::{Cart, CartLine, PaymentMethod, PaymentRequest, ProductInput};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn kinds(items: &[OutboxItem]) -> Vec<&str> {
    items.iter().map(|i| i.kind.as_str()).collect()
}

#[test]
fn writes_queue_their_records_for_the_cloud() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    // A new database queues its reference data (the main register).
    let first = pos.outbox_batch(100)?;
    assert_eq!(kinds(&first), vec!["register"]);
    pos.outbox_ack(&first.iter().map(|i| i.seq).collect::<Vec<_>>())?;
    assert_eq!(pos.outbox_len()?, 0);

    let product = pos.save_product(ProductInput {
        sku: "A".into(),
        name: "ชาไทย".into(),
        category: "เครื่องดื่ม".into(),
        price: "45".parse()?,
        stock: Some(10),
        ..Default::default()
    })?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, "100".parse()?, 10)?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: product.id,
        quantity: 2,
        ..Default::default()
    });
    let sale = pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: "100".parse()?,
        },
        Some(shift),
        11,
    )?;
    let batch = pos.outbox_batch(100)?;
    let names = kinds(&batch);
    for kind in [
        "category",
        "product",
        "shift",
        "sale",
        "cash_movement",
        "stock_movement",
    ] {
        assert!(names.contains(&kind), "{kind} queued: {names:?}");
    }
    // The product was saved, then its stock changed at checkout: one row,
    // carrying the latest stock.
    let products: Vec<_> = batch.iter().filter(|i| i.kind == "product").collect();
    assert_eq!(products.len(), 1);
    assert_eq!(
        products[0].record.as_ref().ok_or("product")?["stock"],
        serde_json::json!(8)
    );
    let receipt = batch
        .iter()
        .find(|i| i.kind == "sale")
        .and_then(|i| i.record.clone())
        .ok_or("sale")?;
    assert_eq!(receipt["number"], serde_json::json!(sale.number));

    // Acknowledged rows leave; a record changed after the read stays queued.
    pos.archive_product(product.id)?;
    pos.outbox_ack(&batch.iter().map(|i| i.seq).collect::<Vec<_>>())?;
    let after = pos.outbox_batch(100)?;
    assert_eq!(kinds(&after), vec!["product", "activity"]);
    assert_eq!(
        after[0].record.as_ref().ok_or("archived")?["active"],
        serde_json::json!(false)
    );
    pos.outbox_ack(&after.iter().map(|i| i.seq).collect::<Vec<_>>())?;

    // A deleted category is sent as None; staff never carry the PIN hash.
    pos.add_category("ของหวาน", 12)?;
    pos.delete_category("ของหวาน", None, 13)?;
    pos.save_staff(
        &pos_domain::StaffInput {
            name: "เจ้าของ".into(),
            role: Some("owner".into()),
            pin: Some("1234".into()),
            active: true,
            ..Default::default()
        },
        14,
    )?;
    let batch = pos.outbox_batch(100)?;
    let category = batch
        .iter()
        .find(|i| i.kind == "category" && i.id == "ของหวาน")
        .ok_or("category")?;
    assert_eq!(category.record, None);
    let staff = batch
        .iter()
        .find(|i| i.kind == "staff")
        .and_then(|i| i.record.clone())
        .ok_or("staff")?;
    assert_eq!(staff["name"], "เจ้าของ");
    assert!(!staff.to_string().contains("pin"));

    // A relink queues everything again.
    pos.requeue_all()?;
    let all = pos.outbox_batch(1000)?;
    assert!(kinds(&all).contains(&"sale"));
    assert!(all.iter().filter(|i| i.kind == "product").count() == 1);
    assert!(all.iter().all(|i| i.record.is_some()));
    Ok(())
}

#[test]
fn owner_edits_from_the_cloud_apply_as_cloud() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let tea = pos.save_product(ProductInput {
        sku: "T1".into(),
        name: "ชาไทย".into(),
        price: "45".parse()?,
        stock: Some(5),
        ..Default::default()
    })?;
    pos.save_product(ProductInput {
        sku: "C1".into(),
        name: "กาแฟ".into(),
        price: "50".parse()?,
        ..Default::default()
    })?;
    // A staffed shop with nobody signed in: the cloud still applies.
    pos.save_staff(
        &pos_domain::StaffInput {
            name: "เจ้าของ".into(),
            role: Some("owner".into()),
            pin: Some("1234".into()),
            active: true,
            ..Default::default()
        },
        1,
    )?;
    pos.sign_out();
    let patch = |v: serde_json::Value| -> Result<serde_json::Map<String, serde_json::Value>> {
        Ok(serde_json::from_value(v)?)
    };
    let id = tea.id.to_string();
    pos.apply_cloud_change(
        "product",
        &id,
        &patch(serde_json::json!({"price": 5500, "name": "ชาไทยเย็น"}))?,
        10,
    )?;
    let now = pos.products("", 0)?;
    let edited = now.iter().find(|p| p.id == tea.id).ok_or("tea")?;
    assert_eq!(edited.price.to_string(), "55.00");
    assert_eq!(edited.name, "ชาไทยเย็น");
    assert_eq!(edited.stock, Some(5), "stock is never touched");
    let events = pos.activity(&pos_core::ActivityQuery::default())?;
    let by = events[0].staff.by.as_ref().ok_or("attribution")?;
    assert_eq!(by.id, pos_core::CLOUD_STAFF);
    // The session is back: the counter itself still needs a sign-in.
    assert!(matches!(
        pos.save_product(ProductInput {
            sku: "X".into(),
            name: "x".into(),
            ..Default::default()
        }),
        Err(pos_core::Error::SignInRequired)
    ));
    // Refusals leave the shop unchanged.
    assert!(matches!(
        pos.apply_cloud_change(
            "product",
            &id,
            &patch(serde_json::json!({"sku": "C1"}))?,
            11
        ),
        Err(pos_core::Error::DuplicateSku)
    ));
    assert!(matches!(
        pos.apply_cloud_change("product", &id, &patch(serde_json::json!({"pin": "1"}))?, 11),
        Err(pos_core::Error::InvalidCloudChange)
    ));
    assert!(matches!(
        pos.apply_cloud_change("staff", &id, &patch(serde_json::json!({"name": "x"}))?, 11),
        Err(pos_core::Error::InvalidCloudChange)
    ));
    // Archive, restore, and a new product with the owner's ID.
    pos.apply_cloud_change(
        "product",
        &id,
        &patch(serde_json::json!({"active": false}))?,
        12,
    )?;
    assert!(pos.products("", 0)?.iter().all(|p| p.id != tea.id));
    pos.apply_cloud_change(
        "product",
        &id,
        &patch(serde_json::json!({"active": true, "price": 6000}))?,
        13,
    )?;
    let back = pos.products("", 0)?;
    assert_eq!(
        back.iter()
            .find(|p| p.id == tea.id)
            .ok_or("restored")?
            .price
            .to_string(),
        "60.00"
    );
    let new = Uuid::new_v4();
    assert!(matches!(
        pos.apply_cloud_change(
            "product",
            &new.to_string(),
            &patch(serde_json::json!({"name": "น้ำ"}))?,
            14
        ),
        Err(pos_core::Error::InvalidCloudChange)
    ));
    pos.apply_cloud_change(
        "product",
        &new.to_string(),
        &patch(serde_json::json!({"name": "น้ำเปล่า", "sku": "W1", "price": 1500, "category": "เครื่องดื่ม"}))?,
        15,
    )?;
    let water = pos.products("", 0)?;
    let water = water.iter().find(|p| p.id == new).ok_or("water")?;
    assert_eq!(
        (water.sku.as_str(), water.category.as_str()),
        ("W1", "เครื่องดื่ม")
    );
    Ok(())
}
