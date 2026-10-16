mod support;
use pos_core::Pos;
use pos_domain::{
    Cart, CartLine, Discount, Money, PaymentMethod, ProductInput, RefundLine, RefundRequest,
    TaxSettings, VoidRequest,
};
use time::macros::{date, offset};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn at() -> i64 {
    date!(2026 - 10 - 13)
        .midnight()
        .assume_offset(offset!(+7))
        .unix_timestamp()
        + 3600
}
fn cart(id: Uuid, quantity: u32) -> Cart {
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: id,
        quantity,
        ..Default::default()
    });
    cart
}
#[test]
fn standard_cost_is_catalog_snapshot_and_unknown_is_not_zero() -> Result {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let known = pos.save_product(ProductInput {
        sku: "K".into(),
        name: "Known".into(),
        price: "10.25".parse()?,
        cost: Some("4.15".parse()?),
        stock: Some(5),
        ..Default::default()
    })?;
    let unknown = pos.save_product(ProductInput {
        sku: "U".into(),
        name: "Unknown".into(),
        price: "20".parse()?,
        ..Default::default()
    })?;
    let free = pos.save_product(ProductInput {
        sku: "Z".into(),
        name: "Zero cost".into(),
        price: "1".parse()?,
        cost: Some(Money::ZERO),
        ..Default::default()
    })?;
    let mut c = cart(known.id, 2);
    c.lines.push(CartLine {
        product_id: unknown.id,
        quantity: 1,
        ..Default::default()
    });
    c.lines.push(CartLine {
        product_id: free.id,
        quantity: 1,
        ..Default::default()
    });
    let sale = pos.checkout(&c, "100".parse()?, at())?;
    assert_eq!(sale.lines[0].unit_cost, Some("4.15".parse()?));
    pos.save_product(ProductInput {
        id: Some(known.id),
        sku: known.sku,
        name: known.name,
        price: "11".parse()?,
        cost: Some("9".parse()?),
        stock: Some(3),
        keep_tracked_stock: true,
        ..Default::default()
    })?;
    drop(pos);
    let mut pos = Pos::open(&path)?;
    let retry = pos.checkout(&c, "100".parse()?, at())?;
    assert_eq!(retry.lines[0].unit_cost, Some("4.15".parse()?));
    let s = pos.daily_summary(date!(2026 - 10 - 13))?.profit;
    assert_eq!(s.covered_revenue.satang(), 2150);
    assert_eq!(s.cost_of_goods.satang(), 830);
    assert_eq!(s.gross_profit.satang(), 1320);
    assert_eq!(s.unknown_revenue.satang(), 2000);
    assert_eq!((s.known_sold_units, s.unknown_sold_units), (3, 1));
    Ok(())
}
#[test]
fn vat_discounts_and_multiple_refunds_reverse_exact_original_profit_on_event_day() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    pos.set_tax_settings(&TaxSettings {
        enabled: true,
        seller_name: "Shop".into(),
        tax_id: "0105555123450".into(),
        address: "Bangkok".into(),
        ..Default::default()
    })?;
    let p = pos.save_product(ProductInput {
        sku: "P".into(),
        name: "P".into(),
        price: "1.07".parse()?,
        cost: Some("0.41".parse()?),
        stock: Some(9),
        ..Default::default()
    })?;
    let mut c = cart(p.id, 3);
    c.discount = Some(Discount::Amount("0.01".parse()?));
    let sale = pos.checkout(&c, "10".parse()?, at())?;
    let vat = sale.tax.as_ref().ok_or("tax")?.vat;
    assert_eq!(
        sale.lines.iter().map(|l| l.vat_share.satang()).sum::<i64>(),
        vat.satang()
    );
    let revenue = sale.total.satang() - vat.satang();
    for n in 0..3 {
        let request = RefundRequest {
            id: Uuid::new_v4(),
            sale_id: sale.id,
            reason: "return".into(),
            refund_method: PaymentMethod::Cash,
            legacy_restock: None,
            lines: vec![RefundLine {
                line: 0,
                quantity: 1,
            }],
        };
        let r = pos.refund_items(&request, Some(shift), at() + 86400 + n)?;
        assert!(r.lines[0].revenue_ex_vat.is_some());
        assert_eq!(
            pos.refund_items(&request, Some(shift), at() + 86400 + n)?
                .id,
            r.id
        );
    }
    let sale_day = pos.daily_summary(date!(2026 - 10 - 13))?.profit;
    assert_eq!(sale_day.covered_revenue.satang(), revenue);
    assert_eq!(sale_day.cost_of_goods.satang(), 123);
    let refund_day = pos.daily_summary(date!(2026 - 10 - 14))?.profit;
    assert_eq!(refund_day.covered_revenue.satang(), -revenue);
    assert_eq!(refund_day.cost_of_goods.satang(), -123);
    assert_eq!(
        refund_day.gross_profit.satang(),
        -sale_day.gross_profit.satang()
    );
    let all = pos
        .summary(date!(2026 - 10 - 13), date!(2026 - 10 - 14))?
        .profit;
    assert_eq!(all.gross_profit.satang(), 0);
    assert_eq!(pos.product(p.id)?.stock, Some(9));
    Ok(())
}
#[test]
fn cost_overflow_rolls_back_checkout_and_full_void_reverses_cost_once() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let input = ProductInput {
        sku: "P".into(),
        name: "P".into(),
        price: "1".parse()?,
        cost: Some(Money::from_satang(i64::MAX)?),
        stock: Some(5),
        ..Default::default()
    };
    let p = pos.save_product(input.clone())?;
    let c = cart(p.id, 2);
    pos.hold(&c, at())?;
    assert!(pos.checkout(&c, "10".parse()?, at()).is_err());
    assert_eq!(pos.product(p.id)?.stock, Some(5));
    assert_eq!(pos.held()?.len(), 1);
    assert!(pos.history(0)?.is_empty());
    pos.save_product(ProductInput {
        id: Some(p.id),
        cost: Some("2.50".parse()?),
        ..input
    })?;
    let sale = pos.checkout(&c, "10".parse()?, at())?;
    assert_eq!(
        pos.daily_summary(date!(2026 - 10 - 13))?
            .profit
            .gross_profit
            .satang(),
        -300
    );
    let request = VoidRequest {
        sale_id: sale.id,
        reason: "void".into(),
        refund_method: PaymentMethod::Cash,
        legacy_restock: None,
    };
    pos.void_sale_for_shift(&request, Some(shift), at() + 86400)?;
    pos.void_sale_for_shift(&request, Some(shift), at() + 86400)?;
    let day = pos.daily_summary(date!(2026 - 10 - 14))?.profit;
    assert_eq!(day.cost_of_goods.satang(), -500);
    assert_eq!(day.gross_profit.satang(), 300);
    Ok(())
}

#[test]
fn schema_twelve_migration_keeps_legacy_receipts_unknown_and_byte_identical() -> Result {
    use diesel::{
        Connection, SqliteConnection, connection::SimpleConnection, prelude::*, sql_types::Text,
    };
    #[derive(QueryableByName)]
    struct Json {
        #[diesel(sql_type = Text)]
        receipt_json: String,
    }
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("db");
    let mut pos = Pos::open(&path)?;
    pos.open_shift(Uuid::new_v4(), Money::ZERO, 0)?;
    let p = pos.save_product(ProductInput {
        sku: "OLD".into(),
        name: "Old".into(),
        price: "10.25".parse()?,
        ..Default::default()
    })?;
    let sale = pos.checkout(&cart(p.id, 1), "20".parse()?, at())?;
    drop(pos);
    let mut db = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    let mut old = serde_json::to_value(&sale)?;
    for line in old["lines"].as_array_mut().ok_or("lines")? {
        line.as_object_mut().ok_or("line")?.remove("unit_cost");
        line.as_object_mut().ok_or("line")?.remove("vat_share");
    }
    let old = serde_json::to_string(&old)?;
    diesel::sql_query("UPDATE sales SET receipt_json=?")
        .bind::<Text, _>(&old)
        .execute(&mut db)?;
    db.batch_execute(support::DROP_OUTBOX)?;
    db.batch_execute(
        "DROP TABLE devices; DROP INDEX one_open_shift_per_register; ALTER TABLE shifts DROP COLUMN register_id; DROP TABLE registers; CREATE UNIQUE INDEX one_open_shift ON shifts ((1)) WHERE closed_at IS NULL; DROP TABLE purchase_cancellations; DROP TABLE purchase_receipts; DROP TABLE purchase_orders; DROP TABLE suppliers; DROP TABLE option_sets; ALTER TABLE products DROP COLUMN option_sets_json; DROP TABLE categories; DROP TABLE product_images; ALTER TABLE products DROP COLUMN image_rev; DROP TABLE ingredient_movements; DROP TABLE ingredients; ALTER TABLE products DROP COLUMN recipe_json; ALTER TABLE products DROP COLUMN option_recipe_json; DROP TABLE promotions; DROP TABLE credit_notes; DROP TABLE kitchen_tickets; DROP TABLE dining_tables; ALTER TABLE products DROP COLUMN kitchen;
         DROP TABLE activity_events; ALTER TABLE products DROP COLUMN cost_satang; PRAGMA user_version=12;",
    )?;
    drop(db);
    let mut pos = Pos::open(&path)?;
    assert_eq!(pos.product(p.id)?.cost, None);
    pos.save_product(ProductInput {
        id: Some(p.id),
        sku: p.sku,
        name: p.name,
        price: p.price,
        cost: Some("3".parse()?),
        ..Default::default()
    })?;
    let report = pos.daily_summary(date!(2026 - 10 - 13))?.profit;
    assert_eq!(report.known_sold_units, 0);
    assert_eq!(report.unknown_sold_units, 1);
    assert_eq!(report.unknown_revenue.satang(), 1025);
    assert_eq!(report.gross_profit.satang(), 0);
    let mut db = SqliteConnection::establish(path.to_str().ok_or("path")?)?;
    let stored = diesel::sql_query("SELECT receipt_json FROM sales").get_result::<Json>(&mut db)?;
    assert_eq!(stored.receipt_json, old);
    Ok(())
}

#[test]
fn option_revenue_service_vat_exclusive_and_cash_rounding_use_paid_snapshots() -> Result {
    use pos_domain::{
        CashRounding, LineOption, OptionChoice, OptionGroup, PricingSettings, RoundMode,
    };
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    pos.open_shift(Uuid::new_v4(), Money::ZERO, 0)?;
    pos.set_tax_settings(&TaxSettings {
        enabled: true,
        seller_name: "Shop".into(),
        tax_id: "0105555123450".into(),
        address: "Bangkok".into(),
        prices_include_vat: false,
        ..Default::default()
    })?;
    pos.set_pricing_settings(&PricingSettings {
        service_bp: 1000,
        rounding: CashRounding {
            step: 25,
            mode: RoundMode::Down,
        },
    })?;
    let group = Uuid::new_v4();
    let choice = Uuid::new_v4();
    let p = pos.save_product(ProductInput {
        sku: "OPT".into(),
        name: "Option product".into(),
        price: "10.05".parse()?,
        cost: Some("4.11".parse()?),
        options: vec![OptionGroup {
            id: group,
            name: "Extras".into(),
            required: false,
            multiple: true,
            choices: vec![OptionChoice {
                id: choice,
                name: "Extra".into(),
                price: "0.01".parse()?,
            }],
        }],
        ..Default::default()
    })?;
    let mut c = cart(p.id, 3);
    c.lines[0].options = vec![LineOption {
        group_id: group,
        choice_id: choice,
    }];
    c.discount = Some(Discount::Amount("0.01".parse()?));
    let sale = pos.checkout(&c, "100".parse()?, at())?;
    assert_eq!(sale.total, "35.50".parse()?);
    assert_eq!(sale.lines[0].unit_price, "10.06".parse()?);
    assert_eq!(sale.service_charge, "3.02".parse()?);
    assert_eq!(sale.rounding, -1);
    let profit = pos.daily_summary(date!(2026 - 10 - 13))?.profit;
    assert_eq!(profit.covered_revenue.satang(), 3318);
    assert_eq!(profit.cost_of_goods.satang(), 1233);
    assert_eq!(profit.gross_profit.satang(), 2085);
    Ok(())
}
