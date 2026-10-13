use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
use pos_core::Pos;
use pos_domain::ProductInput;
use std::error::Error;
type Result = std::result::Result<(), Box<dyn Error>>;

#[test]
fn imports_utf8_quoted_decimals_and_initial_stock_without_updates() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("pos.db");
    let mut pos = Pos::open(&path)?;
    let source = "\u{feff}sku,name,price,stock,barcode,cost,on_sale\r\nA,\"ชา, \"\"ไทย\"\"\nเย็น\",12.75,4,001,3.25,0\r\nB,Water,0.05,,,,1\r\n";
    let preview = pos.preview_product_import(source)?;
    assert!(preview.can_import(), "{:?}", preview.issues);
    assert_eq!(preview.rows[0].product.price.satang(), 1275);
    assert_eq!(pos.import_products(&preview.source)?, 2);
    let a = pos.find_by_code("A")?.ok_or("A missing")?;
    assert_eq!(a.name, "ชา, \"ไทย\"\nเย็น");
    assert_eq!(a.stock, Some(4));
    assert_eq!(a.cost.ok_or("cost missing")?.satang(), 325);
    assert!(!a.on_sale);
    assert_eq!(pos.stock_movements(a.id, 0)?.len(), 1);
    assert!(!pos.preview_product_import(source)?.can_import());
    assert!(pos.import_products(source).is_err());
    drop(pos);
    let mut reopened = Pos::open(&path)?;
    assert_eq!(reopened.catalog_counts()?.total, 2);
    Ok(())
}
#[test]
fn invalid_and_duplicate_rows_refuse_entire_file() -> Result {
    let mut pos = Pos::open(":memory:")?;
    for source in [
        "sku,name,price\nA,Valid,1.15\nB,Invalid,1.151\n",
        "sku,name,price\nA,First,1\na,Second,2\n",
        "sku,name,price,barcode\nA,First,1,0001\nB,Second,2,0001\n",
        "sku,name,price,stock\nA,First,1,-1\n",
        "sku,name,price,on_sale\nA,First,1,yes\n",
        "sku,name,price\nA,\"Unclosed,1\n",
        "sku,name,price\nA,\"Closed\"oops,1\n",
        "sku,name,price,sku\nA,First,1,A\n",
    ] {
        assert!(!pos.preview_product_import(source)?.can_import());
        assert!(pos.import_products(source).is_err());
        assert_eq!(pos.catalog_counts()?.total, 0);
    }
    Ok(())
}
#[test]
fn catalog_change_after_preview_is_revalidated_and_archived_codes_refused() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("pos.db");
    let mut pos = Pos::open(&path)?;
    let mut concurrent = Pos::open(&path)?;
    let source = "sku,name,price,barcode\nA,First,1.15,01\nB,Second,2.25,02\n";
    let preview = pos.preview_product_import(source)?;
    assert!(preview.can_import());
    let b = concurrent.save_product(ProductInput {
        sku: "B".into(),
        name: "External".into(),
        price: "5".parse()?,
        ..Default::default()
    })?;
    assert!(pos.import_products(&preview.source).is_err());
    assert!(pos.find_by_code("A")?.is_none());
    concurrent.archive_product(b.id)?;
    assert!(!pos.preview_product_import(source)?.can_import());
    Ok(())
}
#[test]
fn stock_audit_failure_rolls_back_every_imported_product() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("pos.db");
    let mut pos = Pos::open(&path)?;
    let mut db = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    db.batch_execute("CREATE TRIGGER fail_stock BEFORE INSERT ON stock_movements BEGIN SELECT RAISE(ABORT, 'forced audit failure'); END;")?;
    assert!(
        pos.import_products("sku,name,price,stock\nA,Untracked,1,\nB,Tracked,2,3\n")
            .is_err()
    );
    assert_eq!(pos.catalog_counts()?.total, 0);
    Ok(())
}

#[test]
fn import_requires_catalog_authorization_inside_write() -> Result {
    use pos_domain::{Permission, StaffInput};
    let mut pos = Pos::open(":memory:")?;
    let owner = pos.save_staff(
        &StaffInput {
            id: None,
            name: "Owner".into(),
            role: Some(pos_domain::OWNER.into()),
            pin: Some("9999".into()),
            active: true,
        },
        1,
    )?;
    let cashier = pos.save_staff(
        &StaffInput {
            id: None,
            name: "Cashier".into(),
            role: Some(pos_domain::CASHIER.into()),
            pin: Some("1234".into()),
            active: true,
        },
        1,
    )?;
    let source = "sku,name,price\nA,First,1.15\nB,Second,2.25\n";
    pos.sign_in(cashier.id, "1234")?;
    assert!(pos.preview_product_import(source)?.can_import());
    assert!(matches!(
        pos.import_products(source),
        Err(pos_core::Error::ApprovalRequired(Permission::Catalog))
    ));
    assert_eq!(pos.catalog_counts()?.total, 0);
    pos.approve(owner.id, "9999", Permission::Catalog)?;
    assert_eq!(pos.import_products(source)?, 2);
    pos.clear_approval();
    let events = pos.activity(&pos_core::ActivityQuery {
        kind: Some(pos_core::ActivityKind::Product),
        ..Default::default()
    })?;
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| {
        e.staff.by.as_ref().is_some_and(|s| s.id == cashier.id)
            && e.staff
                .approved_by
                .as_ref()
                .is_some_and(|s| s.id == owner.id)
    }));
    Ok(())
}
#[test]
fn bounded_import_refuses_oversize_and_excess_records() -> Result {
    let mut pos = Pos::open(":memory:")?;
    let oversized = "a".repeat(pos_core::MAX_IMPORT_BYTES + 1);
    assert!(!pos.preview_product_import(&oversized)?.can_import());
    let mut source = String::from("sku,name,price\n");
    for i in 0..=pos_core::MAX_IMPORT_ROWS {
        source.push_str(&format!("A{i},Item,1.15\n"));
    }
    assert!(!pos.preview_product_import(&source)?.can_import());
    assert!(pos.import_products(&source).is_err());
    assert_eq!(pos.catalog_counts()?.total, 0);
    Ok(())
}
