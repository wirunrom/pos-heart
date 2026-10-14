use super::*;
use pos_domain::{
    Discount, Ingredient, IngredientKind, IngredientRequest, LineOption, OptionChoice, OptionGroup,
    OptionRecipe, PromoRule, PromoTarget, Promotion, RecipeItem, RefundLine, RefundRequest,
    StaffInput,
};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn report_csv_uses_receipts_and_movements_across_bangkok_midnight() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let at = time::macros::datetime!(2025-10-13 16:59 UTC).unix_timestamp();
    let next = at + 120; // 00:01 Bangkok on the following day.
    let owner = pos.save_staff(
        &StaffInput {
            id: None,
            name: "=เจ้าของ".into(),
            role: Some("owner".into()),
            pin: Some("9999".into()),
            active: true,
        },
        at - 10,
    )?;
    pos.sign_in(owner.id, "9999")?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, "100".parse()?, at - 9)?;
    let mut ingredients = Vec::new();
    for (name, cost) in [
        ("แป้ง", Some("4".parse()?)),
        ("ไม่ทราบต้นทุน", None),
        ("ต้นทุนศูนย์", Some(Money::ZERO)),
    ] {
        ingredients.push(pos.save_ingredient(
            &Ingredient {
                id: Uuid::new_v4(),
                name: name.into(),
                unit: "kg".into(),
                stock: 100_000,
                low: 0,
                cost,
                active: true,
            },
            at - 8,
        )?);
    }
    let group = Uuid::new_v4();
    let choice = Uuid::new_v4();
    let product = pos.save_product(ProductInput {
        sku: "P".into(),
        name: "ขนม".into(),
        price: "10".parse()?,
        cost: Some("2".parse()?),
        stock: Some(10),
        options: vec![OptionGroup {
            id: group,
            name: "ขนาด".into(),
            required: false,
            multiple: false,
            choices: vec![OptionChoice {
                id: choice,
                name: "เพิ่ม".into(),
                price: "2".parse()?,
            }],
        }],
        recipe: vec![
            RecipeItem {
                ingredient_id: ingredients[0].id,
                amount: 1250,
            },
            RecipeItem {
                ingredient_id: ingredients[1].id,
                amount: 125,
            },
            RecipeItem {
                ingredient_id: ingredients[2].id,
                amount: 500,
            },
        ],
        option_recipe: vec![OptionRecipe {
            choice_id: choice,
            ingredient_id: ingredients[0].id,
            amount: 250,
        }],
        ..Default::default()
    })?;
    let mut promo = Promotion::new(
        "@ลดสิบเปอร์เซ็นต์",
        PromoRule::Off {
            discount: Discount::Percent(1000),
        },
        PromoTarget::All,
    );
    pos.save_promotion(&promo, at - 7)?;
    let bill = |quantity| {
        let mut cart = Cart::new();
        cart.lines.push(pos_domain::CartLine {
            product_id: product.id,
            quantity,
            options: vec![LineOption {
                group_id: group,
                choice_id: choice,
            }],
            ..Default::default()
        });
        cart
    };
    let first = pos.checkout_for_shift(&bill(2), "30".parse()?, Some(shift), at)?;
    let second = pos.checkout_with_payment(
        &bill(1),
        PaymentRequest {
            method: pos_domain::PaymentMethod::Transfer,
            amount: "10.80".parse()?,
        },
        Some(shift),
        at + 1,
    )?;
    assert_eq!(first.total.to_string(), "21.60");
    // Changing/deleting the promotion and changing today's ingredient cost must
    // never cause the report to reconstruct discounts or recipe quantities.
    promo.name = "ชื่อใหม่".into();
    pos.save_promotion(&promo, at + 2)?;
    pos.delete_promotion(promo.id)?;
    ingredients[0].cost = Some("8".parse()?);
    pos.save_ingredient(&ingredients[0], at + 3)?;
    pos.void_sale_for_shift(
        &VoidRequest {
            sale_id: first.id,
            refund_method: first.payment_method,
            reason: "ผิด".into(),
            legacy_restock: None,
        },
        Some(shift),
        next,
    )?;
    let refund = RefundRequest {
        id: Uuid::new_v4(),
        sale_id: second.id,
        refund_method: second.payment_method,
        reason: "คืน".into(),
        lines: vec![RefundLine {
            line: 0,
            quantity: 1,
        }],
        legacy_restock: None,
    };
    pos.refund_items(&refund, Some(shift), next + 1)?;
    pos.refund_items(&refund, Some(shift), next + 2)?;
    pos.adjust_ingredient(
        &IngredientRequest {
            id: Uuid::new_v4(),
            ingredient_id: ingredients[0].id,
            kind: IngredientKind::Remove,
            quantity: 333,
            reason: "เสีย".into(),
        },
        next + 3,
    )?;
    let from = time::macros::date!(2025 - 10 - 13);
    let to = time::macros::date!(2025 - 10 - 14);
    let sale_path = dir.path().join("sales.csv");
    export_report(&mut pos, &sale_path, from, from)?;
    let sales = std::fs::read_to_string(&sale_path)?;
    assert!(sales.starts_with('\u{feff}'));
    assert!(sales.contains("cash_received,30.00\r\nchange_given,8.40\r\n"));
    assert!(sales.contains("cash_sales,21.60\r\ncash_refunds,0.00"));
    assert!(sales.contains("transfer_sales,10.80"));
    assert!(sales.contains(&format!("{},'@ลดสิบเปอร์เซ็นต์,2,3.60", promo.id)));
    assert!(!sales.contains("ชื่อใหม่"));
    assert!(sales.contains(&format!("{},แป้ง,kg,4.500,0.000,36.00", ingredients[0].id)));
    assert!(sales.contains(&format!(
        "{},ไม่ทราบต้นทุน,kg,0.375,0.000,\r\n",
        ingredients[1].id
    )));
    assert!(sales.contains(&format!(
        "{},ต้นทุนศูนย์,kg,1.500,0.000,0.00",
        ingredients[2].id
    )));
    assert!(sales.contains(&format!("{},'=เจ้าของ,2,32.40,0.00", owner.id)));
    assert!(sales.contains("Current unit cost estimate; not historical inventory valuation."));
    let refund_path = dir.path().join("refunds.csv");
    export_report(&mut pos, &refund_path, to, to)?;
    let refunds = std::fs::read_to_string(refund_path)?;
    assert!(refunds.contains("partial_refund_count,1"));
    assert!(refunds.contains("net_sales,-32.40"));
    assert!(refunds.contains(&format!("{},'@ลดสิบเปอร์เซ็นต์,-1,-2.40", promo.id)));
    assert!(refunds.contains(&format!("{},แป้ง,kg,-3.000,0.333,\r\n", ingredients[0].id)));
    // Partial refunds return no ingredients; the complete period retains the
    // second bill's use even though all money was refunded.
    let period_path = dir.path().join("period.csv");
    export_report(&mut pos, &period_path, from, to)?;
    let period = std::fs::read_to_string(&period_path)?;
    assert!(period.contains(&format!("{},แป้ง,kg,1.500,0.333,12.00", ingredients[0].id)));
    assert!(period.contains(&format!("{},'@ลดสิบเปอร์เซ็นต์,1,1.20", promo.id)));
    assert!(export_report(&mut pos, &sale_path, to, to).is_err());
    assert_eq!(std::fs::read_to_string(sale_path)?, sales);
    Ok(())
}

#[test]
fn csv_quantities_keep_exact_thousandths_including_signed_limits() {
    assert_eq!(csv_quantity(1), "0.001");
    assert_eq!(csv_quantity(-333), "-0.333");
    assert_eq!(csv_quantity(0), "0.000");
    assert_eq!(csv_quantity(i64::MIN), "-9223372036854775.808");
    assert_eq!(csv_quantity(i64::MAX), "9223372036854775.807");
}
