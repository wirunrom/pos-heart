use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, DomainError, Money, PaymentMethod, PaymentRequest, Product, ProductInput,
    StockKind, StockRequest,
};
use std::error::Error as StdError;
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
fn tracked(pos: &mut Pos, stock: Option<u32>) -> Result<Product> {
    Ok(pos.save_product_at(
        ProductInput {
            sku: "W".into(),
            name: "น้ำดื่ม".into(),
            price: "10".parse()?,
            stock,
            ..Default::default()
        },
        5,
    )?)
}
fn request(p: &Product, kind: StockKind, quantity: u32, reason: &str) -> StockRequest {
    StockRequest {
        id: Uuid::new_v4(),
        product_id: p.id,
        kind,
        quantity,
        expected: None,
        reason: reason.into(),
    }
}

#[test]
fn receive_remove_and_count_keep_a_history() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let p = tracked(&mut pos, Some(10))?;
    let start = pos.stock_movements(p.id, 0)?;
    assert_eq!(start.len(), 1);
    assert_eq!((start[0].kind, start[0].after), (StockKind::Initial, 10));

    let received = pos.adjust_stock(&request(&p, StockKind::Receive, 24, "ล็อตใหม่"), 10)?;
    assert_eq!(
        (received.before, received.after, received.delta()),
        (10, 34, 24)
    );
    pos.adjust_stock(&request(&p, StockKind::Remove, 4, "ขวดแตก"), 11)?;
    let mut count = request(&p, StockKind::Count, 28, "นับสิ้นเดือน");
    count.expected = Some(30);
    let counted = pos.adjust_stock(&count, 12)?;
    assert_eq!(
        (counted.before, counted.after, counted.delta()),
        (30, 28, -2)
    );
    assert_eq!(pos.product(p.id)?.stock, Some(28));
    let history = pos.stock_movements(p.id, 0)?;
    let kinds: Vec<_> = history.iter().map(|m| m.kind).collect();
    assert_eq!(
        kinds,
        [
            StockKind::Count,
            StockKind::Remove,
            StockKind::Receive,
            StockKind::Initial
        ]
    );
    assert_eq!(history[1].reason, "ขวดแตก");
    Ok(())
}

#[test]
fn retries_are_idempotent_and_conflicts_refused() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let p = tracked(&mut pos, Some(5))?;
    let receive = request(&p, StockKind::Receive, 3, "");
    let first = pos.adjust_stock(&receive, 10)?;
    let again = pos.adjust_stock(&receive, 11)?;
    assert_eq!(first, again);
    assert_eq!(pos.product(p.id)?.stock, Some(8));
    let changed = StockRequest {
        quantity: 4,
        ..receive
    };
    assert!(matches!(
        pos.adjust_stock(&changed, 12),
        Err(Error::ConflictingRequest)
    ));
    assert_eq!(pos.product(p.id)?.stock, Some(8));
    Ok(())
}

#[test]
fn counts_refuse_when_a_sale_happened_meanwhile() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let p = tracked(&mut pos, Some(10))?;
    let mut count = request(&p, StockKind::Count, 9, "");
    count.expected = Some(10);
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: p.id,
        quantity: 2,
        ..Default::default()
    });
    pos.checkout_with_payment(
        &cart,
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: "20".parse()?,
        },
        Some(shift),
        11,
    )?;
    assert!(matches!(
        pos.adjust_stock(&count, 12),
        Err(Error::StockChanged)
    ));
    assert_eq!(pos.product(p.id)?.stock, Some(8));
    assert_eq!(pos.stock_movements(p.id, 0)?.len(), 1);
    Ok(())
}

#[test]
fn invalid_changes_leave_stock_unchanged() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let p = tracked(&mut pos, Some(3))?;
    assert!(matches!(
        pos.adjust_stock(&request(&p, StockKind::Remove, 4, "หาย"), 10),
        Err(Error::Domain(DomainError::InvalidStockChange))
    ));
    assert!(matches!(
        pos.adjust_stock(&request(&p, StockKind::Remove, 1, ""), 10),
        Err(Error::Domain(DomainError::InvalidStockChange))
    ));
    pos.adjust_stock(&request(&p, StockKind::Count, u32::MAX, ""), 10)?;
    assert!(matches!(
        pos.adjust_stock(&request(&p, StockKind::Receive, 1, ""), 11),
        Err(Error::Domain(DomainError::Overflow))
    ));
    let food = pos.save_product(ProductInput {
        sku: "F".into(),
        name: "ข้าว".into(),
        price: "50".parse()?,
        ..Default::default()
    })?;
    assert!(matches!(
        pos.adjust_stock(&request(&food, StockKind::Receive, 1, ""), 12),
        Err(Error::StockNotTracked)
    ));
    assert!(pos.stock_movements(food.id, 0)?.is_empty());
    Ok(())
}

#[test]
fn editing_a_product_keeps_stock_sold_meanwhile() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let p = tracked(&mut pos, Some(10))?;
    // The editor opened at 10; a sale then takes 3.
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: p.id,
        quantity: 3,
        ..Default::default()
    });
    pos.checkout(&cart, "30".parse()?, 6)?;
    let edit = ProductInput {
        id: Some(p.id),
        sku: "W".into(),
        name: "น้ำดื่ม 600 มล.".into(),
        price: "12".parse()?,
        stock: Some(10),
        keep_tracked_stock: true,
        ..Default::default()
    };
    let saved = pos.save_product_at(edit.clone(), 7)?;
    assert_eq!(saved.stock, Some(7));
    // Switching tracking off and on again starts a new Initial movement.
    pos.save_product_at(
        ProductInput {
            stock: None,
            ..edit.clone()
        },
        8,
    )?;
    let restarted = pos.save_product_at(
        ProductInput {
            stock: Some(20),
            ..edit
        },
        9,
    )?;
    assert_eq!(restarted.stock, Some(20));
    let initial = pos
        .stock_movements(p.id, 0)?
        .into_iter()
        .filter(|m| m.kind == StockKind::Initial)
        .count();
    assert_eq!(initial, 2);
    Ok(())
}
