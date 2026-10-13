use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
use pos_core::{Error, Pos, ProductQuery, ProductView};
use pos_domain::{
    Cart, CartLine, CashCount, DomainError, Money, PaymentMethod, PaymentRequest, Product,
    ProductInput, ShiftCount, VoidRequest,
};
use std::error::Error as StdError;
use time::macros::{date, offset};
use uuid::Uuid;

type Result<T = ()> = std::result::Result<T, Box<dyn StdError>>;
fn product(
    pos: &mut Pos,
    sku: &str,
    category: &str,
    stock: Option<u32>,
    barcode: Option<&str>,
) -> Result<Product> {
    Ok(pos.save_product(ProductInput {
        sku: sku.into(),
        name: format!("สินค้า {sku}"),
        category: category.into(),
        price: "12.50".parse()?,
        stock,
        barcode: barcode.map(str::to_owned),
        ..Default::default()
    })?)
}
fn cart(p: &Product, quantity: u32) -> Cart {
    let mut c = Cart::new();
    c.lines.push(CartLine {
        product_id: p.id,
        quantity,
        note: String::new(),
        ..Default::default()
    });
    c
}
fn cash(amount: &str) -> Result<PaymentRequest> {
    Ok(PaymentRequest {
        method: PaymentMethod::Cash,
        amount: amount.parse()?,
    })
}

#[test]
fn version_four_products_migrate_on_sale_without_barcodes() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("legacy.sqlite3");
    {
        let mut db = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
        db.batch_execute(include_str!("../migrations/001_initial.sql"))?;
        db.batch_execute(include_str!("../migrations/002_sale_voids.sql"))?;
        db.batch_execute(include_str!("../migrations/003_shifts.sql"))?;
        db.batch_execute(include_str!("../migrations/004_payment_methods.sql"))?;
        db.batch_execute(
            "INSERT INTO products(id,sku,name,category,price_satang,stock,active)
             VALUES('6f1c1a8e-62b4-4d55-9a49-1b2f7d3b9a10','OLD','Old item','Retail',1025,8,1)",
        )?;
    }
    let mut pos = Pos::open(&path)?;
    let p = &pos.products("", 0)?[0];
    assert!(p.on_sale);
    assert_eq!(p.barcode, None);
    assert_eq!(p.stock, Some(8));
    drop(pos);
    // Reopening a current database is a no-op; a future version is refused.
    let mut db = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    db.batch_execute("PRAGMA user_version=22")?;
    assert!(matches!(
        Pos::open(&path),
        Err(Error::UnsupportedSchema(22))
    ));
    Ok(())
}

#[test]
fn queries_filter_by_category_view_and_barcode_with_counts() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let water = product(&mut pos, "W1", "Drinks", Some(40), Some("8851234000017"))?;
    product(&mut pos, "S1", "Drinks", Some(3), None)?;
    product(&mut pos, "T1", "Household", Some(0), None)?;
    let rice = product(&mut pos, "F1", "Food", None, None)?;
    pos.set_on_sale(&[rice.id], false)?;

    let drinks = pos.catalog(&ProductQuery {
        category: Some("Drinks".into()),
        ..Default::default()
    })?;
    assert_eq!(drinks.len(), 2);
    let low = pos.catalog(&ProductQuery {
        view: ProductView::LowStock,
        ..Default::default()
    })?;
    assert_eq!(
        low.iter().map(|p| p.sku.as_str()).collect::<Vec<_>>(),
        ["S1"]
    );
    assert!(low[0].is_low_stock());
    let out = pos.catalog(&ProductQuery {
        view: ProductView::OutOfStock,
        ..Default::default()
    })?;
    assert_eq!(out[0].sku, "T1");
    let off = pos.catalog(&ProductQuery {
        view: ProductView::NotForSale,
        ..Default::default()
    })?;
    assert_eq!(off[0].id, rice.id);
    let scanned = pos.catalog(&ProductQuery {
        search: "8851234000017".into(),
        ..Default::default()
    })?;
    assert_eq!(scanned[0].id, water.id);
    assert_eq!(
        pos.find_by_code("8851234000017")?.map(|p| p.id),
        Some(water.id)
    );
    assert_eq!(pos.find_by_code("w1")?.map(|p| p.id), Some(water.id));
    assert_eq!(pos.find_by_code("nothing")?.map(|p| p.id), None);

    let counts = pos.catalog_counts()?;
    assert_eq!(counts.total, 4);
    // In the order categories were first used (shops can reorder them).
    assert_eq!(
        counts.categories,
        vec![
            ("Drinks".to_owned(), 2),
            ("Household".to_owned(), 1),
            ("Food".to_owned(), 1)
        ]
    );
    assert_eq!(
        (counts.low_stock, counts.out_of_stock, counts.not_for_sale),
        (1, 1, 1)
    );
    // Archived products leave every list and count.
    pos.archive_product(water.id)?;
    assert_eq!(pos.catalog_counts()?.total, 3);
    assert_eq!(pos.find_by_code("8851234000017")?.map(|p| p.id), None);
    Ok(())
}

#[test]
fn barcodes_are_unique_and_blank_means_none() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    product(&mut pos, "A", "", None, Some("123"))?;
    assert!(matches!(
        product(&mut pos, "B", "", None, Some(" 123 ")),
        Err(e) if matches!(e.downcast_ref::<Error>(), Some(Error::DuplicateBarcode))
    ));
    let blank = product(&mut pos, "C", "", None, Some("   "))?;
    assert_eq!(blank.barcode, None);
    let other_blank = product(&mut pos, "D", "", None, None)?;
    assert_eq!(other_blank.barcode, None);
    Ok(())
}

#[test]
fn not_for_sale_refuses_checkout_and_rolls_back_everything() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let tracked = product(&mut pos, "T", "", Some(5), None)?;
    let paused = product(&mut pos, "P", "", None, None)?;
    let mut bill = cart(&tracked, 2);
    bill.lines.push(CartLine {
        product_id: paused.id,
        quantity: 1,
        note: String::new(),
        ..Default::default()
    });
    pos.hold(&bill, 1)?;
    pos.set_on_sale(&[paused.id], false)?;
    let refused = pos.checkout_with_payment(&bill, cash("100")?, Some(shift), 2);
    assert!(matches!(refused, Err(Error::NotForSale(_))));
    assert_eq!(pos.product(tracked.id)?.stock, Some(5));
    assert_eq!(pos.held()?.len(), 1);
    assert_eq!(pos.active_shift()?.ok_or("shift")?.sale_count, 0);
    // Resuming still works so the operator can remove the line.
    let (resumed, _) = pos.resume(bill.id)?;
    assert_eq!(resumed.lines.len(), 2);
    pos.set_on_sale(&[paused.id], true)?;
    let sale = pos.checkout_with_payment(&bill, cash("100")?, Some(shift), 3)?;
    assert_eq!(sale.total, "37.50".parse()?);
    assert_eq!(pos.product(tracked.id)?.stock, Some(3));
    Ok(())
}

#[test]
fn bulk_changes_apply_to_all_or_none() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let a = product(&mut pos, "A", "Old", None, None)?;
    let b = product(&mut pos, "B", "Old", None, None)?;
    assert!(matches!(
        pos.set_category(&[a.id, Uuid::new_v4()], "New"),
        Err(Error::ProductUnavailable)
    ));
    assert!(pos.products("", 0)?.iter().all(|p| p.category == "Old"));
    assert_eq!(pos.set_category(&[a.id, b.id], "  New  ")?, 2);
    assert!(pos.products("", 0)?.iter().all(|p| p.category == "New"));
    assert!(matches!(
        pos.set_on_sale(&[a.id, Uuid::new_v4()], false),
        Err(Error::ProductUnavailable)
    ));
    assert!(pos.products("", 0)?.iter().all(|p| p.on_sale));
    assert_eq!(pos.set_on_sale(&[a.id, b.id], false)?, 2);
    assert!(pos.products("", 0)?.iter().all(|p| !p.on_sale));
    Ok(())
}

#[test]
fn closing_requires_reason_when_unbalanced_and_keeps_denominations() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let id = Uuid::new_v4();
    pos.open_shift(id, "100".parse()?, 10)?;
    let shift = pos.active_shift()?.ok_or("shift")?;
    let notes = vec![
        CashCount {
            value: "50".parse()?,
            count: 1,
        },
        CashCount {
            value: "20".parse()?,
            count: 2,
        },
    ];
    let short = ShiftCount {
        counted: "90".parse()?,
        reason: "  ".into(),
        denominations: notes.clone(),
    };
    assert!(matches!(
        pos.close_shift_with(id, shift.revision, &short, 11),
        Err(Error::Domain(DomainError::MissingDifferenceReason))
    ));
    let mismatch = ShiftCount {
        counted: "95".parse()?,
        reason: "ทอนเกิน".into(),
        denominations: notes.clone(),
    };
    assert!(matches!(
        pos.close_shift_with(id, shift.revision, &mismatch, 11),
        Err(Error::Domain(DomainError::CountMismatch))
    ));
    assert!(pos.active_shift()?.is_some());
    let explained = ShiftCount {
        reason: " ทอนเกินบิล A-0177 ".into(),
        ..short
    };
    let closed = pos.close_shift_with(id, shift.revision, &explained, 11)?;
    let closing = closed.closing.ok_or("closing")?;
    assert_eq!(closing.difference.satang(), -1_000);
    assert_eq!(closing.reason, "ทอนเกินบิล A-0177");
    assert_eq!(closing.denominations, notes);
    // Balanced counts need no reason.
    let next = Uuid::new_v4();
    pos.open_shift(next, Money::ZERO, 12)?;
    let balanced = ShiftCount {
        counted: Money::ZERO,
        reason: String::new(),
        denominations: Vec::new(),
    };
    pos.close_shift_with(next, 0, &balanced, 13)?;
    assert_eq!(pos.shift_number(id)?, 1);
    assert_eq!(pos.shift_number(next)?, 2);
    let numbers = pos.shift_numbers()?;
    assert_eq!((numbers.get(&id), numbers.get(&next)), (Some(&1), Some(&2)));
    Ok(())
}

#[test]
fn daily_summary_has_hourly_sales_and_restocked_units() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let day = date!(2026 - 10 - 13);
    let midnight = day.midnight().assume_offset(offset!(+7)).unix_timestamp();
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, midnight)?;
    let p = product(&mut pos, "T", "", Some(10), None)?;
    let noon = pos.checkout_with_payment(
        &cart(&p, 2),
        cash("25")?,
        Some(shift),
        midnight + 12 * 3600 + 5,
    )?;
    pos.checkout_with_payment(
        &cart(&p, 1),
        cash("20")?,
        Some(shift),
        midnight + 12 * 3600 + 3599,
    )?;
    pos.checkout_with_payment(
        &cart(&p, 1),
        cash("20")?,
        Some(shift),
        midnight + 23 * 3600 + 3599,
    )?;
    pos.void_sale_for_shift(
        &VoidRequest {
            sale_id: noon.id,
            refund_method: PaymentMethod::Cash,
            reason: "ลูกค้าเปลี่ยนใจ".into(),
            legacy_restock: None,
        },
        Some(shift),
        midnight + 13 * 3600,
    )?;
    let summary = pos.daily_summary(day)?;
    assert_eq!(summary.hourly[12], "37.50".parse()?);
    assert_eq!(summary.hourly[23], "12.50".parse()?);
    assert_eq!(summary.hourly.iter().filter(|m| m.satang() > 0).count(), 2);
    assert_eq!(summary.restocked_units, 2);
    assert_eq!(summary.gross_sales, "50".parse()?);
    Ok(())
}

fn tea(pos: &mut Pos, stock: Option<u32>) -> Result<Product> {
    use pos_domain::{OptionChoice, OptionGroup};
    let choice = |n: u128, name: &str, price: &str| -> Result<OptionChoice> {
        Ok(OptionChoice {
            id: Uuid::from_u128(n),
            name: name.into(),
            price: price.parse()?,
        })
    };
    Ok(pos.save_product(ProductInput {
        sku: "TEA".into(),
        name: "ชาไทยเย็น".into(),
        price: "45".parse()?,
        stock,
        options: vec![
            OptionGroup {
                id: Uuid::from_u128(1),
                name: "ความหวาน".into(),
                required: true,
                multiple: false,
                choices: vec![choice(11, "ปกติ", "0")?, choice(12, "หวานน้อย", "0")?],
            },
            OptionGroup {
                id: Uuid::from_u128(2),
                name: "เพิ่ม".into(),
                required: false,
                multiple: true,
                choices: vec![choice(21, "ไข่มุก", "10")?, choice(22, "วิปครีม", "15.25")?],
            },
        ],
        ..Default::default()
    })?)
}
fn opt(g: u128, c: u128) -> pos_domain::LineOption {
    pos_domain::LineOption {
        group_id: Uuid::from_u128(g),
        choice_id: Uuid::from_u128(c),
    }
}

#[test]
fn options_are_priced_from_the_catalog_and_snapshotted() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let p = tea(&mut pos, Some(5))?;
    assert_eq!(pos.product(p.id)?.options.len(), 2);
    let mut bill = Cart::new();
    bill.lines.push(CartLine {
        product_id: p.id,
        quantity: 2,
        options: vec![opt(1, 12), opt(2, 21), opt(2, 22)],
        ..Default::default()
    });
    bill.lines.push(CartLine {
        product_id: p.id,
        quantity: 1,
        options: vec![opt(1, 11)],
        ..Default::default()
    });
    // A non-cash review must pin the option-inclusive total.
    let wrong = PaymentRequest {
        method: PaymentMethod::Transfer,
        amount: "90".parse()?,
    };
    assert!(matches!(
        pos.checkout_with_payment(&bill, wrong, Some(shift), 1),
        Err(Error::PaymentTotalChanged)
    ));
    let exact = PaymentRequest {
        method: PaymentMethod::Transfer,
        amount: "185.50".parse()?,
    };
    let sale = pos.checkout_with_payment(&bill, exact, Some(shift), 2)?;
    assert_eq!(sale.lines[0].unit_price, "70.25".parse()?);
    assert_eq!(sale.lines[0].total, "140.50".parse()?);
    let names: Vec<_> = sale.lines[0]
        .options
        .iter()
        .map(|o| o.name.as_str())
        .collect();
    assert_eq!(names, ["หวานน้อย", "ไข่มุก", "วิปครีม"]);
    assert_eq!(sale.lines[1].unit_price, "45".parse()?);
    // Separate option lines still share one stock check.
    assert_eq!(pos.product(p.id)?.stock, Some(2));
    // The stored receipt keeps the snapshot even if options change later.
    pos.save_product(ProductInput {
        id: Some(p.id),
        sku: "TEA".into(),
        name: "ชาไทยเย็น".into(),
        price: "45".parse()?,
        stock: Some(2),
        ..Default::default()
    })?;
    let stored = &pos.history(0)?[0].sale;
    assert_eq!(stored.lines[0].options.len(), 3);
    assert_eq!(stored.total, "185.50".parse()?);
    Ok(())
}

#[test]
fn invalid_or_stale_options_roll_back_checkout() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let p = tea(&mut pos, Some(5))?;
    let mut missing = Cart::new();
    missing.lines.push(CartLine {
        product_id: p.id,
        quantity: 1,
        options: vec![opt(2, 21)],
        ..Default::default()
    });
    assert!(matches!(
        pos.checkout_with_payment(&missing, cash("100")?, Some(shift), 1),
        Err(Error::Domain(DomainError::InvalidOptions))
    ));
    let mut held = Cart::new();
    held.lines.push(CartLine {
        product_id: p.id,
        quantity: 1,
        options: vec![opt(1, 12), opt(2, 22)],
        ..Default::default()
    });
    pos.hold(&held, 2)?;
    // Whipped cream is removed from the menu while the bill is held.
    let mut edited = pos.product(p.id)?;
    edited.options[1].choices.truncate(1);
    pos.save_product(ProductInput {
        id: Some(p.id),
        sku: edited.sku,
        name: edited.name,
        price: edited.price,
        stock: edited.stock,
        options: edited.options,
        ..Default::default()
    })?;
    assert!(matches!(
        pos.checkout_with_payment(&held, cash("100")?, Some(shift), 3),
        Err(Error::Domain(DomainError::InvalidOptions))
    ));
    assert_eq!(pos.product(p.id)?.stock, Some(5));
    assert_eq!(pos.held()?.len(), 1);
    assert_eq!(pos.active_shift()?.ok_or("shift")?.sale_count, 0);
    // Invalid option definitions are refused when saving.
    let mut bad = pos.product(p.id)?;
    bad.options[0].choices.clear();
    assert!(matches!(
        pos.save_product(ProductInput {
            id: Some(p.id),
            sku: bad.sku,
            name: bad.name,
            price: bad.price,
            options: bad.options,
            ..Default::default()
        }),
        Err(Error::Domain(DomainError::InvalidOptions))
    ));
    Ok(())
}

#[test]
fn product_pictures_are_stored_apart_and_survive_edits() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let p = product(&mut pos, "P1", "Food", None, None)?;
    assert_eq!(p.image, None);
    assert!(matches!(
        pos.set_product_image(p.id, Some(b"not a png"), 10),
        Err(Error::InvalidImage)
    ));
    let png = b"\x89PNG\r\n\x1a\nfake-but-signed".to_vec();
    let rev = pos.set_product_image(p.id, Some(&png), 10)?;
    assert_eq!(rev, Some(10));
    // A second change in the same second still gets a newer revision.
    assert_eq!(pos.set_product_image(p.id, Some(&png), 10)?, Some(11));
    assert_eq!(pos.product(p.id)?.image, Some(11));
    assert_eq!(pos.product_image(p.id)?, Some((11, png.clone())));
    // Saving the product keeps its picture.
    pos.save_product(ProductInput {
        id: Some(p.id),
        sku: p.sku.clone(),
        name: "renamed".into(),
        price: p.price,
        ..Default::default()
    })?;
    assert_eq!(pos.product(p.id)?.image, Some(11));
    pos.set_product_image(p.id, None, 12)?;
    assert_eq!(pos.product(p.id)?.image, None);
    assert_eq!(pos.product_image(p.id)?, None);
    Ok(())
}

#[test]
fn categories_keep_an_order_rename_merge_and_delete_with_their_products() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    product(&mut pos, "A1", "Drinks", None, None)?;
    product(&mut pos, "A2", "Drinks", None, None)?;
    product(&mut pos, "B1", "Food", None, None)?;
    product(&mut pos, "C1", "", None, None)?;
    pos.add_category("Desserts", 1)?;
    assert!(matches!(
        pos.add_category("Food", 1),
        Err(Error::DuplicateCategory)
    ));
    let names =
        |pos: &mut Pos| -> Result<Vec<(String, u64)>> { Ok(pos.catalog_counts()?.categories) };
    // New categories go last; empty ones are listed; uncategorised at the end.
    assert_eq!(
        names(&mut pos)?,
        vec![
            ("Drinks".into(), 2),
            ("Food".into(), 1),
            ("Desserts".into(), 0),
            (String::new(), 1)
        ]
    );
    pos.reorder_categories(&["Food".into(), "Desserts".into(), "Drinks".into()])?;
    assert_eq!(names(&mut pos)?[0].0, "Food");
    assert!(pos.reorder_categories(&["Food".into()]).is_err());
    // Rename moves the products; renaming into an existing name merges.
    assert_eq!(pos.rename_category("Drinks", "Beverages", 2)?, 2);
    assert_eq!(pos.rename_category("Beverages", "Food", 3)?, 2);
    assert_eq!(names(&mut pos)?[0], ("Food".into(), 3));
    // Deleting moves products into another category or uncategorised.
    assert_eq!(pos.delete_category("Food", Some("Desserts"), 4)?, 3);
    assert_eq!(
        names(&mut pos)?,
        vec![("Desserts".into(), 3), (String::new(), 1)]
    );
    assert!(matches!(
        pos.delete_category("Nope", None, 5),
        Err(Error::CategoryNotFound)
    ));
    Ok(())
}
