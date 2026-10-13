use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
use pos_core::Pos;
use pos_domain::{Cart, CartLine, Money, Product, ProductInput};
use std::error::Error;
type Result = std::result::Result<(), Box<dyn Error>>;
fn item(pos: &mut Pos, stock: Option<u32>) -> std::result::Result<Product, Box<dyn Error>> {
    Ok(pos.save_product(ProductInput {
        id: None,
        sku: "P001".into(),
        name: "สินค้า / Item".into(),
        category: "General".into(),
        price: "10.25".parse()?,
        stock,
        ..Default::default()
    })?)
}
fn cart(p: &Product, quantity: u32) -> Cart {
    let mut c = Cart::new();
    c.lines.push(CartLine {
        product_id: p.id,
        quantity,
        note: "no ice".into(),
        ..Default::default()
    });
    c
}
#[test]
fn checkout_is_atomic_idempotent_and_survives_restart() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("shop.sqlite3");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, Some(5))?;
    let c = cart(&p, 2);
    pos.hold(&c, 100)?;
    let sale = pos.checkout(&c, "30".parse()?, 101)?;
    assert_eq!(sale.total.to_string(), "20.50");
    assert_eq!(sale.change.to_string(), "9.50");
    assert!(pos.held()?.is_empty());
    assert_eq!(pos.products("", 0)?[0].stock, Some(3));
    let repeated = pos.checkout(&c, "100".parse()?, 200)?;
    assert_eq!(repeated.number, sale.number);
    assert_eq!(repeated.cash, sale.cash);
    assert_eq!(pos.products("", 0)?[0].stock, Some(3));
    assert!(pos.hold(&c, 201).is_err());
    drop(pos);
    let mut reopened = Pos::open(path)?;
    assert_eq!(reopened.history(0)?.len(), 1);
    assert_eq!(reopened.products("", 0)?[0].stock, Some(3));
    Ok(())
}
#[test]
fn insufficient_cash_rolls_back_stock_and_preserves_held_cart() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, Some(5))?;
    let c = cart(&p, 2);
    pos.hold(&c, 1)?;
    assert!(pos.checkout(&c, "1".parse()?, 2).is_err());
    assert_eq!(pos.products("", 0)?[0].stock, Some(5));
    assert!(pos.history(0)?.is_empty());
    assert_eq!(pos.held()?.len(), 1);
    Ok(())
}
#[test]
fn repeated_product_lines_share_stock_validation() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, Some(3))?;
    let mut c = cart(&p, 2);
    c.lines.push(CartLine {
        product_id: p.id,
        quantity: 2,
        note: "different note".into(),
        ..Default::default()
    });
    assert!(pos.checkout(&c, "100".parse()?, 1).is_err());
    assert_eq!(pos.products("", 0)?[0].stock, Some(3));
    assert!(pos.history(0)?.is_empty());
    Ok(())
}
#[test]
fn receipt_keeps_sale_time_price_and_untracked_food_works() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, None)?;
    let c = cart(&p, 2);
    pos.save_product(ProductInput {
        id: Some(p.id),
        sku: p.sku,
        name: "New name".into(),
        category: p.category,
        price: "11.50".parse()?,
        stock: None,
        ..Default::default()
    })?;
    let sale = pos.checkout(&c, "30".parse()?, 1)?;
    assert_eq!(sale.total.to_string(), "23.00");
    pos.archive_product(p.id)?;
    assert!(pos.products("", 0)?.is_empty());
    assert_eq!(pos.history(0)?[0].sale.lines[0].name, "New name");
    assert_eq!(pos.history(0)?[0].sale.total, sale.total);
    Ok(())
}
#[test]
fn duplicate_sku_future_schema_and_backup() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    item(&mut pos, Some(1))?;
    assert!(item(&mut pos, None).is_err());
    let backup = dir.path().join("backup");
    pos.backup(&backup)?;
    assert!(pos.backup(&backup).is_err());
    assert_eq!(Pos::open(backup)?.products("", 0)?.len(), 1);
    drop(pos);
    let mut conn = SqliteConnection::establish(path.to_str().ok_or("Invalid test path")?)?;
    conn.batch_execute("PRAGMA user_version=999;")?;
    drop(conn);
    assert!(Pos::open(path).is_err());
    Ok(())
}
#[test]
fn archived_item_cannot_be_paid_and_invalid_money_cannot_deserialize() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    pos.open_shift(uuid::Uuid::new_v4(), Money::ZERO, 0)?;
    let p = item(&mut pos, Some(2))?;
    let c = cart(&p, 1);
    pos.archive_product(p.id)?;
    assert!(pos.checkout(&c, "50".parse()?, 1).is_err());
    assert!(pos.history(0)?.is_empty());
    assert!(serde_json::from_str::<Money>("-1").is_err());
    Ok(())
}
