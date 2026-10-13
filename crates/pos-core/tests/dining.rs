use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, DiningTable, Money, PaymentMethod, PaymentRequest, Product, ProductInput,
};
use std::error::Error as StdError;
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
struct Shop {
    pos: Pos,
    shift: Uuid,
    rice: Product,
    water: Product,
    t1: DiningTable,
    t2: DiningTable,
    _dir: tempfile::TempDir,
}
fn shop() -> Result<Shop> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let rice = pos.save_product(ProductInput {
        sku: "R".into(),
        name: "ข้าวผัด".into(),
        price: "60".parse()?,
        ..Default::default()
    })?;
    let water = pos.save_product(ProductInput {
        sku: "W".into(),
        name: "น้ำเปล่า".into(),
        price: "10".parse()?,
        kitchen: false,
        ..Default::default()
    })?;
    let table = |name: &str| DiningTable {
        id: Uuid::new_v4(),
        zone: "ในร้าน".into(),
        name: name.into(),
        seats: 4,
        active: true,
    };
    let t1 = pos.save_table(&table("T1"))?;
    let t2 = pos.save_table(&table("T2"))?;
    Ok(Shop {
        pos,
        shift,
        rice,
        water,
        t1,
        t2,
        _dir: dir,
    })
}
fn line(p: &Product, q: u32, note: &str) -> CartLine {
    CartLine {
        product_id: p.id,
        quantity: q,
        note: note.into(),
        ..Default::default()
    }
}

#[test]
fn tables_hold_one_open_bill_each() -> Result {
    let mut s = shop()?;
    assert_eq!(s.pos.dining_tables()?.len(), 2);
    let mut a = Cart::new();
    a.table = Some(s.t1.id);
    a.lines.push(line(&s.rice, 1, ""));
    s.pos.hold(&a, 1)?;
    let mut b = Cart::new();
    b.table = Some(s.t1.id);
    b.lines.push(line(&s.rice, 1, ""));
    assert!(matches!(s.pos.hold(&b, 2), Err(Error::TableBusy)));
    // Moving a bill to a free table is just holding it there.
    a.table = Some(s.t2.id);
    s.pos.hold(&a, 3)?;
    s.pos.hold(&b, 4)?;
    // A table with an open bill cannot be switched off; duplicates are refused.
    let off = DiningTable {
        active: false,
        ..s.t2.clone()
    };
    assert!(matches!(s.pos.save_table(&off), Err(Error::TableBusy)));
    let dup = DiningTable {
        id: Uuid::new_v4(),
        ..s.t1.clone()
    };
    assert!(matches!(s.pos.save_table(&dup), Err(Error::DuplicateTable)));
    Ok(())
}

#[test]
fn kitchen_gets_only_changes_and_cancellations() -> Result {
    let mut s = shop()?;
    let mut cart = Cart::new();
    cart.table = Some(s.t1.id);
    cart.guests = 3;
    cart.lines.push(line(&s.rice, 2, "ไม่เผ็ด"));
    cart.lines.push(line(&s.water, 3, ""));
    let (stored, ticket) = s.pos.send_to_kitchen(&cart, 10)?;
    let ticket = ticket.ok_or("ticket")?;
    assert_eq!((ticket.table.as_str(), ticket.guests), ("T1", 3));
    // Water is not prepared by the kitchen.
    assert_eq!(ticket.lines.len(), 1);
    assert_eq!(
        (ticket.lines[0].name.as_str(), ticket.lines[0].quantity),
        ("ข้าวผัด", 2)
    );
    assert_eq!(ticket.lines[0].note, "ไม่เผ็ด");
    // Sending again, even from a stale copy, fires nothing.
    assert!(s.pos.send_to_kitchen(&cart, 11)?.1.is_none());
    // One more plate, then one cancelled.
    let mut more = stored.clone();
    more.lines[0].quantity = 3;
    let t2 = s.pos.send_to_kitchen(&more, 12)?.1.ok_or("t2")?;
    assert_eq!(t2.lines[0].quantity, 1);
    let mut less = more.clone();
    less.lines[0].quantity = 1;
    let t3 = s.pos.send_to_kitchen(&less, 13)?.1.ok_or("t3")?;
    assert_eq!(t3.lines[0].quantity, -2);
    assert!(t3.is_cancellation());
    // Cancellations are in the activity log.
    let log = s.pos.activity(&pos_core::ActivityQuery::default())?;
    assert!(
        log.iter()
            .any(|e| e.kind == pos_core::ActivityKind::Kitchen)
    );
    // The kitchen screen: three open tickets, oldest first; bump and recall.
    let open = s.pos.kitchen_tickets()?;
    assert_eq!(open.len(), 3);
    assert!(open[0].number < open[1].number);
    s.pos.set_ticket_line(open[0].id, 0, true)?;
    s.pos.bump_ticket(open[0].id, Some(20))?;
    let after = s.pos.kitchen_tickets()?;
    assert_eq!(after.iter().filter(|t| t.bumped_at.is_none()).count(), 2);
    let bumped = after.iter().find(|t| t.id == open[0].id).ok_or("recall")?;
    assert!(bumped.lines[0].done);
    s.pos.bump_ticket(open[0].id, None)?;
    assert_eq!(
        s.pos
            .kitchen_tickets()?
            .iter()
            .filter(|t| t.bumped_at.is_none())
            .count(),
        3
    );
    Ok(())
}

#[test]
fn autosave_never_rolls_back_the_kitchen_state_and_paying_frees_the_table() -> Result {
    let mut s = shop()?;
    let mut cart = Cart::new();
    cart.table = Some(s.t1.id);
    cart.lines.push(line(&s.rice, 1, ""));
    s.pos.hold(&cart, 1)?;
    s.pos.send_to_kitchen(&cart, 2)?;
    // An autosave queued before the send still carries no kitchen state.
    s.pos.hold(&cart, 3)?;
    assert!(s.pos.send_to_kitchen(&cart, 4)?.1.is_none());
    let held = s.pos.held()?;
    assert_eq!(held[0].sent.len(), 1);
    s.pos.checkout_with_payment(
        &held[0],
        PaymentRequest {
            method: PaymentMethod::Cash,
            amount: "100".parse()?,
        },
        Some(s.shift),
        5,
    )?;
    assert!(s.pos.held()?.is_empty());
    let mut next = Cart::new();
    next.table = Some(s.t1.id);
    next.lines.push(line(&s.rice, 1, ""));
    s.pos.hold(&next, 6)?;
    // Emptying a sent bill cancels it in the kitchen and discards the bill.
    let mut empty = next.clone();
    s.pos.send_to_kitchen(&next, 7)?;
    empty.lines.clear();
    let (_, ticket) = s.pos.send_to_kitchen(&empty, 8)?;
    assert!(ticket.ok_or("cancel")?.is_cancellation());
    assert!(s.pos.held()?.is_empty());
    Ok(())
}
