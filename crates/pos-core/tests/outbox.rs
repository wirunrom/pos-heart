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
