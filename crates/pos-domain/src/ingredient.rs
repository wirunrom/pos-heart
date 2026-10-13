//! Ingredients and recipes: a dish's recipe lists the ingredients one unit
//! uses, and each sale deducts them inside the checkout transaction.
//! Quantities are integer thousandths of the ingredient's unit (1 kg = 1000,
//! 0.25 kg = 250), so grams, millilitres and half eggs all stay exact.
//! Ingredient stock may go below zero: counts are approximate in a kitchen
//! and a sale is never refused because of an ingredient.
use crate::{DomainError, Money};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Thousandths per unit.
pub const MILLI: i64 = 1000;
/// Most ingredients in one recipe.
pub const MAX_RECIPE: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ingredient {
    pub id: Uuid,
    pub name: String,
    /// "กรัม", "ml", "ฟอง"… shown after every quantity.
    pub unit: String,
    /// Stock in thousandths of `unit`; may be negative.
    pub stock: i64,
    /// Low-stock warning at or below this (thousandths); 0 = no warning.
    #[serde(default)]
    pub low: i64,
    /// Cost of one whole unit, for recipe costs. None is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Money>,
    pub active: bool,
}
impl Ingredient {
    pub fn validate(&self) -> Result<(), DomainError> {
        let name = self.name.trim().chars().count();
        let unit = self.unit.trim().chars().count();
        if name == 0 || name > 80 || unit == 0 || unit > 20 || self.low < 0 {
            return Err(DomainError::InvalidIngredient);
        }
        Ok(())
    }
    pub fn is_low(&self) -> bool {
        self.active && self.low > 0 && self.stock <= self.low
    }
    /// Cost of `amount` thousandths, rounded half up to the satang.
    pub fn cost_of(&self, amount: i64) -> Option<Money> {
        let unit = i128::from(self.cost?.satang());
        let satang = (unit * i128::from(amount) + i128::from(MILLI / 2)) / i128::from(MILLI);
        Money::from_satang(i64::try_from(satang).ok()?).ok()
    }
}
/// One line of a recipe: what one sold unit of the product uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeItem {
    pub ingredient_id: Uuid,
    /// Thousandths of the ingredient's unit, more than zero.
    pub amount: i64,
}
pub fn validate_recipe(recipe: &[RecipeItem]) -> Result<(), DomainError> {
    let mut seen = std::collections::HashSet::new();
    if recipe.len() > MAX_RECIPE
        || recipe
            .iter()
            .any(|r| r.amount <= 0 || r.amount > 1_000_000 * MILLI || !seen.insert(r.ingredient_id))
    {
        return Err(DomainError::InvalidIngredient);
    }
    Ok(())
}
/// An option choice's change to the recipe, per unit sold: "+ fried egg"
/// adds 1 egg (1000), "no egg" removes it (−1000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionRecipe {
    pub choice_id: Uuid,
    pub ingredient_id: Uuid,
    /// Thousandths; negative reduces the base recipe. Never zero.
    pub amount: i64,
}
/// Choices must belong to the product's options; one row per (choice,
/// ingredient); amounts non-zero.
pub fn validate_option_recipe(
    rows: &[OptionRecipe],
    groups: &[crate::OptionGroup],
) -> Result<(), DomainError> {
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let known = groups
            .iter()
            .any(|g| g.choices.iter().any(|c| c.id == r.choice_id));
        if !known
            || r.amount == 0
            || r.amount.abs() > 1_000_000 * MILLI
            || !seen.insert((r.choice_id, r.ingredient_id))
        {
            return Err(DomainError::InvalidIngredient);
        }
    }
    if rows.len() > MAX_RECIPE * 4 {
        return Err(DomainError::InvalidIngredient);
    }
    Ok(())
}
/// One unit's recipe with the chosen options applied. An ingredient never
/// goes below zero; ingredients that end at zero are left out.
pub fn line_recipe(
    base: &[RecipeItem],
    option_recipe: &[OptionRecipe],
    chosen: &[crate::LineOption],
) -> Vec<RecipeItem> {
    let mut out: Vec<RecipeItem> = base.to_vec();
    for row in option_recipe
        .iter()
        .filter(|r| chosen.iter().any(|c| c.choice_id == r.choice_id))
    {
        match out
            .iter_mut()
            .find(|i| i.ingredient_id == row.ingredient_id)
        {
            Some(item) => item.amount = item.amount.saturating_add(row.amount),
            None => out.push(RecipeItem {
                ingredient_id: row.ingredient_id,
                amount: row.amount,
            }),
        }
    }
    out.retain(|i| i.amount > 0);
    out
}
/// Total use per ingredient for (recipe, units sold) pairs, in first-seen order.
pub fn ingredient_use<'a>(
    lines: impl IntoIterator<Item = (&'a [RecipeItem], u32)>,
) -> Result<Vec<(Uuid, i64)>, DomainError> {
    let mut out: Vec<(Uuid, i64)> = Vec::new();
    for (recipe, units) in lines {
        for item in recipe {
            let used = item
                .amount
                .checked_mul(i64::from(units))
                .ok_or(DomainError::Overflow)?;
            match out.iter_mut().find(|(id, _)| *id == item.ingredient_id) {
                Some((_, total)) => {
                    *total = total.checked_add(used).ok_or(DomainError::Overflow)?
                }
                None => out.push((item.ingredient_id, used)),
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngredientKind {
    /// Added with a starting stock.
    Initial,
    /// Bought or delivered: adds.
    Receive,
    /// Waste, spoiled or staff meals: removes; needs a reason.
    Remove,
    /// Physical count: sets the stock.
    Count,
    /// Used by a sale (recipe × units), recorded with the receipt.
    Sale,
    /// Given back when the sale was voided.
    Void,
}
impl IngredientKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Receive => "receive",
            Self::Remove => "remove",
            Self::Count => "count",
            Self::Sale => "sale",
            Self::Void => "void",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "initial" => Self::Initial,
            "receive" => Self::Receive,
            "remove" => Self::Remove,
            "count" => Self::Count,
            "sale" => Self::Sale,
            "void" => Self::Void,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientMovement {
    pub id: Uuid,
    pub ingredient_id: Uuid,
    pub created_at: i64,
    pub kind: IngredientKind,
    pub before: i64,
    pub after: i64,
    pub reason: String,
    /// The sale for sale and void movements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sale_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "crate::Attribution::is_empty")]
    pub staff: crate::Attribution,
}
/// An operator's ingredient change. `id` makes a lost response safe to retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngredientRequest {
    pub id: Uuid,
    pub ingredient_id: Uuid,
    pub kind: IngredientKind,
    /// Thousandths received or removed, or the counted total.
    pub quantity: i64,
    pub reason: String,
}
impl IngredientRequest {
    pub fn validate(&self) -> Result<(), DomainError> {
        let ok = match self.kind {
            IngredientKind::Receive => self.quantity > 0,
            IngredientKind::Remove => self.quantity > 0 && !self.reason.trim().is_empty(),
            IngredientKind::Count => self.quantity >= 0,
            _ => false,
        };
        if !ok || self.reason.chars().count() > 240 {
            return Err(DomainError::InvalidStockChange);
        }
        Ok(())
    }
    pub fn after(&self, before: i64) -> Result<i64, DomainError> {
        match self.kind {
            IngredientKind::Receive => before.checked_add(self.quantity),
            IngredientKind::Remove => before.checked_sub(self.quantity),
            IngredientKind::Count => Some(self.quantity),
            _ => None,
        }
        .ok_or(DomainError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u128, amount: i64) -> RecipeItem {
        RecipeItem {
            ingredient_id: Uuid::from_u128(id),
            amount,
        }
    }
    #[test]
    fn recipes_add_up_per_ingredient_and_cost_exactly() -> Result<(), DomainError> {
        // Fried rice: 150 g rice, 1 egg; omelette: 2 eggs.
        let fried = [item(1, 150 * MILLI), item(2, MILLI)];
        let omelette = [item(2, 2 * MILLI)];
        let used = ingredient_use([(&fried[..], 3), (&omelette[..], 2)])?;
        assert_eq!(
            used,
            vec![
                (Uuid::from_u128(1), 450 * MILLI),
                (Uuid::from_u128(2), 7 * MILLI)
            ]
        );
        let egg = Ingredient {
            id: Uuid::from_u128(2),
            name: "ไข่ไก่".into(),
            unit: "ฟอง".into(),
            stock: 3 * MILLI,
            low: 5 * MILLI,
            cost: Some(Money::from_satang(450)?),
            active: true,
        };
        assert!(egg.is_low());
        // Half an egg at 4.50 = 2.25.
        assert_eq!(egg.cost_of(500), Some(Money::from_satang(225)?));
        assert_eq!(
            validate_recipe(&[item(1, 0)]),
            Err(DomainError::InvalidIngredient)
        );
        assert_eq!(
            validate_recipe(&[item(1, 5), item(1, 6)]),
            Err(DomainError::InvalidIngredient)
        );
        Ok(())
    }
    #[test]
    fn option_recipes_add_and_remove_without_going_below_zero() {
        let (extra, none) = (Uuid::from_u128(10), Uuid::from_u128(11));
        let chosen = |id| {
            [crate::LineOption {
                group_id: Uuid::nil(),
                choice_id: id,
            }]
        };
        let base = [item(1, 150 * MILLI), item(2, MILLI)];
        let rows = [
            OptionRecipe {
                choice_id: extra,
                ingredient_id: Uuid::from_u128(2),
                amount: MILLI,
            },
            OptionRecipe {
                choice_id: extra,
                ingredient_id: Uuid::from_u128(3),
                amount: 5 * MILLI,
            },
            OptionRecipe {
                choice_id: none,
                ingredient_id: Uuid::from_u128(2),
                amount: -2 * MILLI,
            },
        ];
        // "+ egg": two eggs and the extra sauce.
        let more = line_recipe(&base, &rows, &chosen(extra));
        assert_eq!(
            more,
            vec![item(1, 150 * MILLI), item(2, 2 * MILLI), item(3, 5 * MILLI)]
        );
        // "No egg": the egg is gone, never negative.
        assert_eq!(
            line_recipe(&base, &rows, &chosen(none)),
            vec![item(1, 150 * MILLI)]
        );
        assert_eq!(line_recipe(&base, &rows, &[]), base.to_vec());
        let group = crate::OptionGroup {
            id: Uuid::from_u128(20),
            name: "ไข่".into(),
            required: false,
            multiple: false,
            choices: vec![crate::OptionChoice {
                id: extra,
                name: "+ไข่".into(),
                price: Money::ZERO,
            }],
        };
        assert_eq!(
            validate_option_recipe(&rows[..2], std::slice::from_ref(&group)),
            Ok(())
        );
        // "none" is not a choice of this product.
        assert!(validate_option_recipe(&rows, &[group]).is_err());
    }
    #[test]
    fn requests_may_take_stock_below_zero_only_by_removal() {
        let req = |kind, quantity, reason: &str| IngredientRequest {
            id: Uuid::nil(),
            ingredient_id: Uuid::nil(),
            kind,
            quantity,
            reason: reason.into(),
        };
        assert_eq!(req(IngredientKind::Remove, 500, "เสีย").after(200), Ok(-300));
        assert_eq!(req(IngredientKind::Count, 1200, "").after(-300), Ok(1200));
        assert!(req(IngredientKind::Remove, 500, " ").validate().is_err());
        assert!(req(IngredientKind::Sale, 1, "").validate().is_err());
    }
}
