use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_types::{BigInt, Text},
};
use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, DomainError, Money, Product, ProductInput, Sale, SignedMoney, VoidRequest,
};
use std::{
    error::Error as StdError,
    path::Path,
    sync::{Arc, Barrier},
};
use time::{
    Date,
    macros::{date, offset},
};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
fn connection(path: &Path) -> Result<SqliteConnection> {
    Ok(SqliteConnection::establish(
        path.to_str().ok_or("Invalid test path")?,
    )?)
}
fn item(pos: &mut Pos, sku: &str, price: &str, stock: Option<u32>) -> Result<Product> {
    Ok(pos.save_product(ProductInput {
        id: None,
        sku: sku.into(),
        name: format!("สินค้า {sku}"),
        category: "test".into(),
        price: price.parse()?,
        stock,
        ..Default::default()
    })?)
}
fn cart(product: &Product, quantity: u32) -> Cart {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: product.id,
        quantity,
        note: "ไม่หวาน".into(),
        ..Default::default()
    });
    cart
}
fn request(sale: &Sale) -> VoidRequest {
    VoidRequest {
        sale_id: sale.id,
        refund_method: pos_domain::PaymentMethod::Cash,
        reason: "คิดเงินผิด / wrong order".into(),
        legacy_restock: None,
    }
}
fn midnight(date: Date) -> i64 {
    date.midnight().assume_offset(offset!(+7)).unix_timestamp()
}
#[derive(QueryableByName)]
struct ReceiptJson {
    #[diesel(sql_type = Text)]
    receipt_json: String,
}
fn receipt_json(path: &Path, id: Uuid) -> Result<String> {
    Ok(
        diesel::sql_query("SELECT receipt_json FROM sales WHERE cart_id=?")
            .bind::<Text, _>(id.to_string())
            .get_result::<ReceiptJson>(&mut connection(path)?)?
            .receipt_json,
    )
}
#[derive(QueryableByName)]
struct Stock {
    #[diesel(sql_type = BigInt)]
    stock: i64,
}
fn stock(path: &Path, id: Uuid) -> Result<i64> {
    Ok(diesel::sql_query("SELECT stock FROM products WHERE id=?")
        .bind::<Text, _>(id.to_string())
        .get_result::<Stock>(&mut connection(path)?)?
        .stock)
}

#[test]
fn full_void_preserves_receipt_returns_snapshot_stock_once_and_survives_backup() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shop.sqlite3");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, "retail", "10.25", Some(10))?;
    let food = item(&mut pos, "food", "69", None)?;
    let mut c = cart(&p, 2);
    c.lines.push(CartLine {
        product_id: p.id,
        quantity: 1,
        note: "other note".into(),
        ..Default::default()
    });
    c.lines.push(CartLine {
        product_id: food.id,
        quantity: 1,
        note: "".into(),
        ..Default::default()
    });
    let start = midnight(date!(2026 - 10 - 12));
    let sale = pos.checkout(&c, "200".parse()?, start)?;
    assert_eq!(sale.total.to_string(), "99.75");
    assert_eq!(sale.lines[0].stock_tracked, Some(true));
    assert_eq!(sale.lines[2].stock_tracked, Some(false));
    let immutable = receipt_json(&path, sale.id)?;
    pos.save_product(ProductInput {
        id: Some(food.id),
        sku: food.sku,
        name: "now tracked".into(),
        category: food.category,
        price: "999".parse()?,
        stock: Some(7),
        ..Default::default()
    })?;
    pos.archive_product(p.id)?;
    let cancelled = pos.void_sale(&request(&sale), start + 1)?;
    assert_eq!(cancelled.refund, sale.total); // Refund amount paid, never the tendered 200 baht.
    assert_eq!(cancelled.restocked.len(), 1);
    assert_eq!(cancelled.restocked[0].quantity, 3);
    assert_eq!(stock(&path, p.id)?, 10);
    assert_eq!(stock(&path, food.id)?, 7); // Untracked at sale time, even if tracked now.
    assert_eq!(receipt_json(&path, sale.id)?, immutable);
    let repeated = pos.void_sale(&request(&sale), start + 2)?;
    assert_eq!(repeated.id, cancelled.id);
    assert_eq!(stock(&path, p.id)?, 10);
    assert_eq!(pos.checkout(&c, "200".parse()?, start + 3)?.id, sale.id);
    assert_eq!(stock(&path, p.id)?, 10);
    let summary = pos.daily_summary(date!(2026 - 10 - 12))?;
    assert_eq!((summary.sale_count, summary.void_count), (1, 1));
    assert_eq!(summary.net.to_string(), "0.00");
    assert!(summary.top_products.iter().all(|p| p.net_quantity == 0));
    let backup = dir.path().join("backup.sqlite3");
    pos.backup(&backup)?;
    drop(pos);
    let mut restored = Pos::open(backup)?;
    assert_eq!(
        restored.history(0)?[0].cancellation.as_ref().map(|v| v.id),
        Some(cancelled.id)
    );
    assert_eq!(
        restored.daily_summary(date!(2026 - 10 - 12))?.refunds,
        sale.total
    );
    assert_eq!(
        Pos::open(path)?.void_sale(&request(&sale), start + 4)?.id,
        cancelled.id
    );
    Ok(())
}

#[test]
fn failed_audit_write_rolls_back_stock_and_invalid_requests_do_nothing() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shop");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, "p", "10.25", Some(5))?;
    let sale = pos.checkout(&cart(&p, 2), "30".parse()?, 100)?;
    let mut invalid = request(&sale);
    invalid.reason = "   ".into();
    assert!(matches!(
        pos.void_sale(&invalid, 101),
        Err(Error::Domain(DomainError::MissingVoidReason))
    ));
    assert!(matches!(
        pos.void_sale(&request(&sale), 99),
        Err(Error::InvalidVoidTime)
    ));
    let mut missing = request(&sale);
    missing.sale_id = Uuid::new_v4();
    assert!(matches!(
        pos.void_sale(&missing, 101),
        Err(Error::SaleNotFound)
    ));
    let mut conn = connection(&path)?;
    conn.batch_execute("CREATE TRIGGER reject_void BEFORE INSERT ON sale_voids BEGIN SELECT RAISE(ABORT, 'test write failure'); END;")?;
    assert!(pos.void_sale(&request(&sale), 101).is_err());
    assert_eq!(stock(&path, p.id)?, 3);
    assert!(pos.history(0)?[0].cancellation.is_none());
    conn.batch_execute("DROP TRIGGER reject_void;")?;
    pos.void_sale(&request(&sale), 102)?;
    assert_eq!(stock(&path, p.id)?, 5);
    Ok(())
}

#[test]
fn stock_tracking_changes_and_overflow_refuse_partial_refunds() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shop");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, "p", "1", Some(5))?;
    let sale = pos.checkout(&cart(&p, 2), "2".parse()?, 100)?;
    let change = |stock| ProductInput {
        id: Some(p.id),
        sku: p.sku.clone(),
        name: p.name.clone(),
        category: p.category.clone(),
        price: p.price,
        stock,
        ..Default::default()
    };
    pos.save_product(change(None))?;
    assert!(matches!(
        pos.void_sale(&request(&sale), 101),
        Err(Error::StockTrackingChanged(_))
    ));
    assert!(pos.product(p.id)?.stock.is_none());
    pos.save_product(change(Some(u32::MAX)))?;
    assert!(matches!(
        pos.void_sale(&request(&sale), 101),
        Err(Error::Domain(DomainError::Overflow))
    ));
    assert_eq!(pos.product(p.id)?.stock, Some(u32::MAX));
    assert!(pos.history(0)?[0].cancellation.is_none());
    pos.save_product(change(Some(3)))?;
    pos.void_sale(&request(&sale), 102)?;
    assert_eq!(pos.product(p.id)?.stock, Some(5));
    Ok(())
}

#[test]
fn simultaneous_voids_return_one_audit_and_restore_stock_once() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shop");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, "p", "1", Some(5))?;
    let sale = pos.checkout(&cart(&p, 2), "2".parse()?, 100)?;
    drop(pos);
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let path = path.clone();
        let barrier = barrier.clone();
        let request = request(&sale);
        workers.push(std::thread::spawn(move || -> pos_core::Result<_> {
            let mut pos = Pos::open(path)?;
            barrier.wait();
            pos.void_sale(&request, 101)
        }));
    }
    let mut ids = Vec::new();
    for worker in workers {
        ids.push(worker.join().map_err(|_| "Test worker panicked")??.id);
    }
    assert_eq!(ids[0], ids[1]);
    assert_eq!(stock(&path, p.id)?, 5);
    assert_eq!(Pos::open(path)?.history(0)?.len(), 1);
    Ok(())
}

#[test]
fn bangkok_boundaries_refund_day_and_negative_net_are_exact() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop"))?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, "p", "10.25", None)?;
    let start = midnight(date!(2026 - 10 - 12));
    let end = start + 86_400;
    // UTC boundary is 17:00 on the preceding calendar day, not UTC midnight.
    assert_eq!(time::OffsetDateTime::from_unix_timestamp(start)?.hour(), 17);
    let previous = pos.checkout(&cart(&p, 3), "40".parse()?, start - 1)?;
    let current = pos.checkout(&cart(&p, 2), "30".parse()?, start)?;
    pos.checkout(&cart(&p, 1), "20".parse()?, end - 1)?;
    let following = pos.checkout(&cart(&p, 4), "50".parse()?, end)?;
    pos.void_sale(&request(&previous), start + 1)?;
    let summary = pos.daily_summary(date!(2026 - 10 - 12))?;
    assert_eq!((summary.sale_count, summary.void_count), (2, 1));
    assert_eq!(summary.gross_sales.to_string(), "30.75");
    assert_eq!(summary.refunds.to_string(), "30.75");
    assert_eq!(summary.cash_received.to_string(), "50.00");
    assert_eq!(summary.change_given.to_string(), "19.25");
    assert_eq!(summary.net.to_string(), "0.00");
    assert_eq!(summary.top_products[0].net_quantity, 0);
    assert_eq!(summary.top_products[0].net_sales.to_string(), "0.00");
    pos.void_sale(&request(&current), end)?;
    let next = pos.daily_summary(date!(2026 - 10 - 13))?;
    assert_eq!(next.gross_sales, following.total);
    assert_eq!(next.refunds, current.total);
    assert_eq!(next.net.to_string(), "20.50");
    assert_eq!(next.top_products[0].net_quantity, 2);
    pos.void_sale(&request(&following), end + 86_400)?;
    let negative = pos.daily_summary(date!(2026 - 10 - 14))?;
    assert_eq!(negative.sale_count, 0);
    assert_eq!(negative.void_count, 1);
    assert_eq!(negative.net.to_string(), "-41.00");
    assert_eq!(negative.top_products[0].net_quantity, -4);
    assert_eq!(negative.top_products[0].net_sales.to_string(), "-41.00");
    let empty = pos.daily_summary(date!(2026 - 10 - 15))?;
    assert_eq!((empty.sale_count, empty.void_count), (0, 0));
    assert!(empty.top_products.is_empty());
    assert_eq!(empty.net.to_string(), "0.00");
    Ok(())
}

#[test]
fn version_one_receipts_migrate_unchanged_and_require_explicit_legacy_stock() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("legacy");
    // Build a real v1 schema. Its JSON predates the stock_tracked field.
    let mut conn = connection(&path)?;
    conn.batch_execute(include_str!("../migrations/001_initial.sql"))?;
    let id = Uuid::new_v4();
    let cart_id = Uuid::new_v4();
    diesel::sql_query("INSERT INTO products(id,sku,name,category,price_satang,stock,active) VALUES(?, 'OLD', 'Old item', '',1025,8,1)")
        .bind::<Text,_>(id.to_string()).execute(&mut conn)?;
    let json = serde_json::json!({ "id": cart_id, "number": 1, "created_at": 100,
        "label": "old", "lines": [{ "product_id": id, "sku": "OLD", "name": "Old item",
        "unit_price": 1025, "quantity": 2, "note": "เก่า", "total": 2050 }], "total":2050, "cash":3000, "change":950 }).to_string();
    diesel::sql_query("INSERT INTO sales(cart_id,created_at,label,total_satang,cash_satang,change_satang,receipt_json) VALUES(?,100,'old',2050,3000,950,?)")
        .bind::<Text,_>(cart_id.to_string()).bind::<Text,_>(&json).execute(&mut conn)?;
    drop(conn);
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let record = pos.history(0)?.remove(0);
    assert_eq!(record.sale.lines[0].stock_tracked, None);
    assert_eq!(receipt_json(&path, cart_id)?, json);
    let mut request = request(&record.sale);
    assert!(matches!(
        pos.void_sale(&request, 101),
        Err(Error::LegacyStockReviewRequired)
    ));
    assert_eq!(stock(&path, id)?, 8);
    request.legacy_restock = Some(vec![Uuid::new_v4()]);
    assert!(matches!(
        pos.void_sale(&request, 101),
        Err(Error::InvalidStockSelection)
    ));
    request.legacy_restock = Some(vec![id]);
    let cancellation = pos.void_sale(&request, 102)?;
    assert!(cancellation.legacy_stock_reviewed);
    assert_eq!(stock(&path, id)?, 10);
    assert_eq!(receipt_json(&path, cart_id)?, json);
    drop(pos);
    assert!(Pos::open(path)?.history(0)?[0].cancellation.is_some());
    Ok(())
}

#[test]
fn ranking_is_limited_and_report_overflow_is_recoverable() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("ranking"))?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let start = midnight(date!(2026 - 10 - 12));
    for quantity in 1..=12 {
        let p = item(&mut pos, &format!("P{quantity}"), "0.01", None)?;
        pos.checkout(&cart(&p, quantity), "1".parse()?, start)?;
    }
    let report = pos.daily_summary(date!(2026 - 10 - 12))?;
    assert_eq!(report.top_products.len(), 10);
    assert_eq!(report.top_products[0].net_quantity, 12);
    assert_eq!(report.top_products[9].net_quantity, 3);
    let mut large = Pos::open(dir.path().join("large"))?;
    large.open_shift(Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(
        &mut large,
        "MAX",
        &Money::from_satang(i64::MAX)?.to_string(),
        None,
    )?;
    large.checkout(&cart(&p, 1), p.price, start)?;
    let current = large.active_shift()?.ok_or("missing shift")?;
    large.close_shift(current.id, current.revision, p.price, start)?;
    large.open_shift(Uuid::new_v4(), Money::ZERO, start)?;
    let cent = item(&mut large, "CENT", "0.01", None)?;
    large.checkout(&cart(&cent, 1), cent.price, start)?;
    assert!(matches!(
        large.daily_summary(date!(2026 - 10 - 12)),
        Err(Error::Domain(DomainError::Overflow))
    ));
    assert_eq!(large.history(0)?.len(), 2);
    assert_eq!(
        SignedMoney::from_satang(i64::MIN).to_string(),
        "-92233720368547758.08"
    );
    assert!(matches!(
        large.daily_summary(Date::MAX),
        Err(Error::InvalidReportDate)
    ));
    Ok(())
}
