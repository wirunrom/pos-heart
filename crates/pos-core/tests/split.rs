use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, DiningTable, Money, PaymentMethod as Method, ProductInput, RefundLine,
    RefundRequest, SplitPayment, Tender, VoidRequest,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn money(s: &str) -> Result<Money> {
    Ok(s.parse()?)
}
fn shop() -> Result<(Pos, Uuid, pos_domain::Product, tempfile::TempDir)> {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, money("100")?, 0)?;
    let p = pos.save_product(ProductInput {
        sku: "R".into(),
        name: "ข้าวผัด".into(),
        price: money("10.25")?,
        stock: Some(20),
        ..Default::default()
    })?;
    Ok((pos, shift, p, dir))
}
fn cart(p: &pos_domain::Product, quantity: u32) -> Cart {
    let mut c = Cart::new();
    c.lines.push(CartLine {
        product_id: p.id,
        quantity,
        ..Default::default()
    });
    c
}
fn split(cash: &str, transfer: &str, given: &str) -> Result<SplitPayment> {
    Ok(SplitPayment {
        tenders: vec![
            Tender {
                method: Method::Cash,
                amount: money(cash)?,
            },
            Tender {
                method: Method::Transfer,
                amount: money(transfer)?,
            },
        ],
        cash_given: money(given)?,
    })
}

#[test]
fn split_payments_reach_each_method_and_only_cash_reaches_the_drawer() -> Result {
    let (mut pos, shift, p, _dir) = shop()?;
    // 4 × 10.25 = 41.00: 11.00 cash (20 handed over) + 30.00 transfer.
    let wrong = split("11", "29", "20")?;
    assert!(matches!(
        pos.checkout_split(&cart(&p, 4), &wrong, Some(shift), 1),
        Err(Error::PaymentTotalChanged)
    ));
    let bill = cart(&p, 4);
    let sale = pos.checkout_split(&bill, &split("11", "30", "20")?, Some(shift), 1)?;
    assert!(sale.is_split());
    assert_eq!(sale.payment_method, Method::Transfer);
    assert_eq!((sale.cash, sale.change), (money("20")?, money("9")?));
    // A retry returns the same receipt without a second ledger entry.
    let again = pos.checkout_split(&bill, &split("11", "30", "20")?, Some(shift), 2)?;
    assert_eq!(again.number, sale.number);
    let s = pos.active_shift()?.ok_or("shift")?;
    assert_eq!(s.sale_count, 1);
    assert_eq!(s.expected()?.to_string(), "111.00");
    assert_eq!(s.transfer_sales, money("30")?);
    let report = pos.daily_summary(time::macros::date!(1970 - 01 - 01))?;
    assert_eq!(report.payments.method(Method::Cash).sales, money("11")?);
    assert_eq!(report.payments.method(Method::Transfer).sales, money("30")?);
    assert_eq!(
        (report.cash_received, report.change_given),
        (money("20")?, money("9")?)
    );
    // One method only is not a split.
    let one = SplitPayment {
        tenders: vec![Tender {
            method: Method::Card,
            amount: money("10.25")?,
        }],
        cash_given: Money::ZERO,
    };
    assert!(
        pos.checkout_split(&cart(&p, 1), &one, Some(shift), 3)
            .is_err()
    );
    Ok(())
}

#[test]
fn split_sales_refund_through_their_own_methods() -> Result {
    let (mut pos, shift, p, _dir) = shop()?;
    let sale = pos.checkout_split(&cart(&p, 4), &split("11", "30", "11")?, Some(shift), 1)?;
    let request = |method, quantity| RefundRequest {
        id: Uuid::new_v4(),
        sale_id: sale.id,
        refund_method: method,
        reason: "คืน".into(),
        lines: vec![RefundLine { line: 0, quantity }],
        legacy_restock: None,
    };
    assert!(matches!(
        pos.refund_items(&request(Method::Card, 1), Some(shift), 2),
        Err(Error::InvalidRefundMethod)
    ));
    // 10.25 back in cash fits the 11.00 cash part; a second plate does not.
    pos.refund_items(&request(Method::Cash, 1), Some(shift), 2)?;
    assert!(matches!(
        pos.refund_items(&request(Method::Cash, 1), Some(shift), 3),
        Err(Error::RefundExceedsMethod)
    ));
    pos.refund_items(&request(Method::Transfer, 2), Some(shift), 3)?;
    let s = pos.active_shift()?.ok_or("shift")?;
    assert_eq!(s.refunds, money("10.25")?);
    assert_eq!(s.transfer_refunds, money("20.50")?);
    let report = pos.daily_summary(time::macros::date!(1970 - 01 - 01))?;
    assert_eq!(
        report.payments.method(Method::Transfer).refunds,
        money("20.50")?
    );

    // A full void returns every part to its own method.
    let other = pos.checkout_split(&cart(&p, 4), &split("11", "30", "11")?, Some(shift), 4)?;
    let void = pos.void_sale_for_shift(
        &VoidRequest {
            sale_id: other.id,
            refund_method: other.payment_method,
            reason: "ยกเลิก".into(),
            legacy_restock: None,
        },
        Some(shift),
        5,
    )?;
    assert_eq!(void.refunded_by(), other.paid_by());
    let s = pos.active_shift()?.ok_or("shift")?;
    assert_eq!(s.refunds, money("21.25")?);
    assert_eq!(s.transfer_refunds, money("50.50")?);
    assert_eq!(s.refund_count, 3);
    let report = pos.daily_summary(time::macros::date!(1970 - 01 - 01))?;
    assert_eq!(
        report.payments.method(Method::Cash).refunds,
        money("21.25")?
    );
    Ok(())
}

#[test]
fn splitting_a_table_bill_keeps_the_kitchen_state_and_the_table() -> Result {
    let (mut pos, shift, p, _dir) = shop()?;
    let table = pos.save_table(&DiningTable {
        id: Uuid::new_v4(),
        zone: String::new(),
        name: "T1".into(),
        seats: 4,
        active: true,
    })?;
    let mut bill = cart(&p, 4);
    bill.table = Some(table.id);
    let (stored, _) = pos.send_to_kitchen(&bill, 1)?;
    // The UI copy may be stale (no kitchen state); the stored one counts.
    let new_id = Uuid::new_v4();
    let (rest, moved) = pos.split_bill(&bill, &[(0, 1)], new_id, 2)?;
    assert_eq!(rest.table, Some(table.id));
    assert_eq!((rest.lines[0].quantity, moved.lines[0].quantity), (3, 1));
    assert_eq!(
        rest.sent[0].quantity + moved.sent[0].quantity,
        stored.sent[0].quantity
    );
    assert!(pos.send_to_kitchen(&rest, 3)?.1.is_none());
    assert!(pos.send_to_kitchen(&moved, 3)?.1.is_none());
    // Retrying the split returns the stored pair instead of splitting again.
    let (again, _) = pos.split_bill(&bill, &[(0, 1)], new_id, 4)?;
    assert_eq!(again.lines[0].quantity, 3);
    // The split-off bill pays on its own; the table keeps its bill.
    pos.checkout_for_shift(&moved, money("20")?, Some(shift), 5)?;
    let held = pos.held()?;
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].table, Some(table.id));
    assert!(matches!(
        pos.split_bill(&rest, &[(0, 3)], Uuid::new_v4(), 6),
        Err(Error::Domain(pos_domain::DomainError::InvalidBillSplit))
    ));
    Ok(())
}
