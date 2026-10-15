//! The protocol carries a whole counter flow as JSON, as a paired device will.
use pos_core::{Pos, ProductQuery};
use pos_domain::{Cart, CartLine, Money, PaymentMethod, PaymentRequest, ProductInput, ShiftCount};
use pos_service::{Period, Query, Reply, Request, bangkok_date, handle};
use std::error::Error;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Sends `request` through JSON both ways, like the wire, and checks the
/// reply survives a round trip unchanged.
fn call(pos: &mut Pos, request: Request, now: i64) -> Result<Reply> {
    let wire = serde_json::to_string(&request)?;
    let request: Request = serde_json::from_str(&wire)?;
    let reply = handle(pos, request, now)?;
    let back: Reply = serde_json::from_str(&serde_json::to_string(&reply)?)?;
    assert_eq!(serde_json::to_value(&back)?, serde_json::to_value(&reply)?);
    Ok(back)
}
fn query(now: i64) -> Query {
    Query {
        products: ProductQuery::default(),
        sale_offset: 0,
        report: Period::day(bangkok_date(now)),
        shift_offset: 0,
        movement_shift: None,
        movement_offset: 0,
        stock_product: None,
    }
}

#[test]
fn a_counter_day_runs_through_the_protocol() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    let now = 1_760_000_000;
    let Reply::ProductSaved(product) = call(
        &mut pos,
        Request::SaveProduct(Box::new(ProductInput {
            sku: "D-001".into(),
            name: "ชาไทยเย็น / Thai iced tea".into(),
            category: "เครื่องดื่ม".into(),
            price: "45".parse()?,
            stock: Some(10),
            ..Default::default()
        })),
        now,
    )?
    else {
        return Err("expected ProductSaved".into());
    };
    let shift = uuid::Uuid::new_v4();
    call(&mut pos, Request::OpenShift(shift, "500".parse()?), now + 1)?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: product.id,
        quantity: 2,
        ..Default::default()
    });
    call(&mut pos, Request::Hold(Box::new(cart.clone())), now + 2)?;
    let pay = Request::Pay(
        Box::new(cart.clone()),
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: "100".parse()?,
        },
        shift,
    );
    let Reply::Paid(sale) = call(&mut pos, pay.clone(), now + 3)? else {
        return Err("expected Paid".into());
    };
    assert_eq!(sale.total.to_string(), "90.00");
    assert_eq!(sale.change.to_string(), "10.00");
    // A resent payment (dropped connection) returns the same receipt.
    let Reply::Paid(again) = call(&mut pos, pay, now + 9)? else {
        return Err("expected Paid".into());
    };
    assert_eq!(again.number, sale.number);
    let Reply::Data(data) = call(&mut pos, Request::Refresh(Box::new(query(now))), now + 10)?
    else {
        return Err("expected Data".into());
    };
    assert_eq!(data.products[0].stock, Some(8));
    assert_eq!(data.sales.len(), 1);
    assert!(data.held.is_empty());
    assert_eq!(data.today_summary.sale_count, 1);
    let open = data.shift.ok_or("shift should be open")?;
    let expected = open.expected()?;
    assert_eq!(expected.to_string(), "590.00");
    let Reply::ShiftClosed(closed) = call(
        &mut pos,
        Request::CloseShift(
            shift,
            open.revision,
            ShiftCount {
                counted: Money::from_satang(expected.satang())?,
                reason: String::new(),
                denominations: Default::default(),
            },
        ),
        now + 20,
    )?
    else {
        return Err("expected ShiftClosed".into());
    };
    assert_eq!(closed.id, shift);
    Ok(())
}

#[test]
fn approval_covers_only_the_wrapped_request() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("shop.sqlite3"))?;
    // Without staff an approval wrapper for an unknown approver is refused
    // and leaves no approval behind.
    let wrapped = Request::Approved {
        approver: uuid::Uuid::new_v4(),
        pin: "0000".into(),
        permission: pos_domain::Permission::Void,
        inner: Box::new(Request::Lookup("X".into())),
    };
    assert!(call(&mut pos, wrapped, 1).is_err());
    assert!(matches!(
        call(&mut pos, Request::Lookup("X".into()), 2)?,
        Reply::Found(code, None) if code == "X"
    ));
    Ok(())
}

const _: () = assert!(pos_service::PROTOCOL_VERSION >= 1);
