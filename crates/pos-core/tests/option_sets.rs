use pos_core::{Error, Pos};
use pos_domain::{Cart, CartLine, LineOption, Money, OptionChoice, OptionGroup, ProductInput};
use uuid::Uuid;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn sweetness() -> Result<OptionGroup> {
    Ok(OptionGroup {
        id: Uuid::new_v4(),
        name: "ความหวาน".into(),
        required: true,
        multiple: false,
        choices: vec![
            OptionChoice {
                id: Uuid::new_v4(),
                name: "หวานปกติ".into(),
                price: Money::ZERO,
            },
            OptionChoice {
                id: Uuid::new_v4(),
                name: "หวานน้อย".into(),
                price: Money::ZERO,
            },
        ],
    })
}

#[test]
fn shared_sets_apply_to_every_linked_product_and_unlink_cleanly() -> Result {
    let dir = tempfile::tempdir()?;
    let mut pos = Pos::open(dir.path().join("db"))?;
    let shift = Uuid::new_v4();
    pos.open_shift(shift, Money::ZERO, 0)?;
    let mut set = sweetness()?;
    pos.save_option_set(&set, 1)?;
    let drink = |sku: &str, sets: Vec<Uuid>| ProductInput {
        sku: sku.into(),
        name: sku.into(),
        price: "40".parse().unwrap_or_default(),
        option_sets: sets,
        ..Default::default()
    };
    let tea = pos.save_product(drink("TEA", vec![set.id]))?;
    let coffee = pos.save_product(drink("COF", Vec::new()))?;
    assert_eq!(tea.options.len(), 1);
    assert_eq!(tea.own_options().len(), 0);
    // Bulk link: coffee uses the set too.
    assert_eq!(
        pos.link_option_set(set.id, &[tea.id, coffee.id], true, 2)?,
        1
    );
    assert_eq!(pos.product(coffee.id)?.options[0].name, "ความหวาน");
    assert_eq!(pos.option_sets()?[0].1, 2);
    // Editing the set changes both: a priced choice.
    set.choices.push(OptionChoice {
        id: Uuid::new_v4(),
        name: "หวานมาก +5".into(),
        price: "5".parse()?,
    });
    pos.save_option_set(&set, 3)?;
    let extra = set.choices[2].id;
    let mut cart = Cart::new();
    for p in [tea.id, coffee.id] {
        cart.lines.push(CartLine {
            product_id: p,
            quantity: 1,
            options: vec![LineOption {
                group_id: set.id,
                choice_id: extra,
            }],
            ..Default::default()
        });
    }
    let sale = pos.checkout_for_shift(&cart, "100".parse()?, Some(shift), 4)?;
    assert_eq!(sale.total, "90".parse()?);
    assert_eq!(sale.lines[0].options[0].name, "หวานมาก +5");
    // Saving a product keeps its sets apart from its own groups.
    let mut input = drink("TEA", vec![set.id]);
    input.id = Some(tea.id);
    input.options = pos.product(tea.id)?.options;
    let saved = pos.save_product(input)?;
    assert_eq!((saved.options.len(), saved.own_options().len()), (1, 0));
    // Unknown sets are refused; deleting a set unlinks it everywhere.
    assert!(matches!(
        pos.save_product(drink("X", vec![Uuid::new_v4()])),
        Err(Error::OptionSetNotFound)
    ));
    assert_eq!(pos.delete_option_set(set.id, 5)?, 2);
    assert!(pos.product(tea.id)?.options.is_empty());
    assert!(pos.option_sets()?.is_empty());
    Ok(())
}
