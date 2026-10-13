use pos_core::{Error, Pos};
use pos_domain::{
    Cart, CartLine, Ingredient, IngredientKind, IngredientRequest, MILLI, Money, ProductInput,
    RecipeItem, RefundLine, RefundRequest, VoidRequest,
};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn ingredient(name: &str, unit: &str, stock: i64) -> Ingredient {
    Ingredient {
        id: Uuid::new_v4(),
        name: name.into(),
        unit: unit.into(),
        stock,
        low: 0,
        cost: None,
        active: true,
    }
}
fn stock(pos: &mut Pos, id: Uuid) -> Result<i64> {
    Ok(pos
        .ingredients()?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or("ingredient")?
        .stock)
}

#[test]
fn sales_use_recipes_voids_give_back_and_refunds_do_not() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let rice = pos.save_ingredient(&ingredient("ข้าวสวย", "กรัม", 1000 * MILLI), 1)?;
    let egg = pos.save_ingredient(&ingredient("ไข่ไก่", "ฟอง", 2 * MILLI), 1)?;
    assert!(matches!(
        pos.save_ingredient(&ingredient("ไข่ไก่", "ฟอง", 0), 1),
        Err(Error::DuplicateIngredient)
    ));
    // Unknown ingredients are refused in a recipe.
    let bad = ProductInput {
        sku: "X".into(),
        name: "x".into(),
        price: "10".parse()?,
        recipe: vec![RecipeItem {
            ingredient_id: Uuid::new_v4(),
            amount: MILLI,
        }],
        ..Default::default()
    };
    assert!(matches!(
        pos.save_product(bad),
        Err(Error::IngredientNotFound)
    ));
    let fried = pos.save_product(ProductInput {
        sku: "F".into(),
        name: "ข้าวผัด".into(),
        price: "60".parse()?,
        recipe: vec![
            RecipeItem {
                ingredient_id: rice.id,
                amount: 150 * MILLI,
            },
            RecipeItem {
                ingredient_id: egg.id,
                amount: MILLI,
            },
        ],
        ..Default::default()
    })?;
    let mut cart = Cart::new();
    cart.lines.push(CartLine {
        product_id: fried.id,
        quantity: 3,
        ..Default::default()
    });
    // Three plates with two eggs left: the sale goes through, eggs go to −1.
    let sale = pos.checkout_for_shift(&cart, "180".parse()?, Some(shift), 10)?;
    assert_eq!(stock(&mut pos, rice.id)?, 550 * MILLI);
    assert_eq!(stock(&mut pos, egg.id)?, -MILLI);
    // A retry deducts nothing more.
    pos.checkout_for_shift(&cart, "180".parse()?, Some(shift), 11)?;
    assert_eq!(stock(&mut pos, rice.id)?, 550 * MILLI);
    let history = pos.ingredient_movements(egg.id, 0)?;
    assert_eq!(history[0].kind, IngredientKind::Sale);
    assert_eq!(history[0].sale_id, Some(sale.id));
    // A partial refund keeps the use (the food was made)…
    let mut other = cart.clone();
    other.id = Uuid::new_v4();
    other.lines[0].quantity = 1;
    let second = pos.checkout_for_shift(&other, "60".parse()?, Some(shift), 12)?;
    pos.refund_items(
        &RefundRequest {
            id: Uuid::new_v4(),
            sale_id: second.id,
            refund_method: second.payment_method,
            reason: "ช้า".into(),
            lines: vec![RefundLine {
                line: 0,
                quantity: 1,
            }],
            legacy_restock: None,
        },
        Some(shift),
        13,
    )?;
    assert_eq!(stock(&mut pos, rice.id)?, 400 * MILLI);
    // …a void gives it back.
    pos.void_sale_for_shift(
        &VoidRequest {
            sale_id: sale.id,
            refund_method: sale.payment_method,
            reason: "คีย์ผิด".into(),
            legacy_restock: None,
        },
        Some(shift),
        14,
    )?;
    assert_eq!(stock(&mut pos, rice.id)?, 850 * MILLI);
    assert_eq!(stock(&mut pos, egg.id)?, MILLI);
    // Manual changes: receive, remove with a reason, count; retries are safe.
    let receive = IngredientRequest {
        id: Uuid::new_v4(),
        ingredient_id: egg.id,
        kind: IngredientKind::Receive,
        quantity: 30 * MILLI,
        reason: String::new(),
    };
    pos.adjust_ingredient(&receive, 15)?;
    pos.adjust_ingredient(&receive, 16)?;
    assert_eq!(stock(&mut pos, egg.id)?, 31 * MILLI);
    pos.adjust_ingredient(
        &IngredientRequest {
            id: Uuid::new_v4(),
            ingredient_id: egg.id,
            kind: IngredientKind::Count,
            quantity: 28 * MILLI,
            reason: String::new(),
        },
        17,
    )?;
    assert_eq!(stock(&mut pos, egg.id)?, 28 * MILLI);
    // Editing never changes the stock.
    let mut renamed = egg.clone();
    renamed.name = "ไข่ไก่เบอร์ 2".into();
    renamed.stock = 0;
    pos.save_ingredient(&renamed, 18)?;
    assert_eq!(stock(&mut pos, egg.id)?, 28 * MILLI);
    Ok(())
}
