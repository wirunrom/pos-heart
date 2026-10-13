//! Ingredients: stock in thousandths of a unit (column authoritative, the
//! rest as JSON), a movement per change, and recipe use at checkout. Sales
//! deduct inside the checkout transaction; a void gives the same amounts
//! back; partial refunds do not (the food was made).
use crate::{
    Error, Pos, Result,
    schema::{ingredient_movements, ingredients},
};
use diesel::prelude::*;
use pos_domain::{
    Attribution, Ingredient, IngredientKind, IngredientMovement, IngredientRequest, Permission,
    RecipeItem,
};
use uuid::Uuid;

fn load_one(conn: &mut SqliteConnection, id: Uuid) -> Result<Option<Ingredient>> {
    ingredients::table
        .find(id.to_string())
        .select((ingredients::ingredient_json, ingredients::stock_milli))
        .first::<(String, i64)>(conn)
        .optional()?
        .map(|(json, stock)| {
            let mut i: Ingredient = serde_json::from_str(&json)?;
            i.stock = stock;
            Ok(i)
        })
        .transpose()
}
fn insert(conn: &mut SqliteConnection, m: &IngredientMovement) -> Result<()> {
    diesel::insert_into(ingredient_movements::table)
        .values((
            ingredient_movements::id.eq(m.id.to_string()),
            ingredient_movements::ingredient_id.eq(m.ingredient_id.to_string()),
            ingredient_movements::created_at.eq(m.created_at),
            ingredient_movements::kind.eq(m.kind.key()),
            ingredient_movements::sale_id.eq(m.sale_id.map(|id| id.to_string())),
            ingredient_movements::movement_json.eq(serde_json::to_string(m)?),
        ))
        .execute(conn)?;
    Ok(())
}
fn set_stock(conn: &mut SqliteConnection, id: Uuid, stock: i64, now: i64) -> Result<()> {
    diesel::update(ingredients::table.find(id.to_string()))
        .set((
            ingredients::stock_milli.eq(stock),
            ingredients::updated_at.eq(now),
        ))
        .execute(conn)?;
    Ok(())
}
/// Every recipe ingredient must exist; new recipes may only use active ones.
pub(crate) fn check_recipe(
    conn: &mut SqliteConnection,
    recipe: &[RecipeItem],
    before: &[RecipeItem],
) -> Result<()> {
    for item in recipe {
        let ingredient = load_one(conn, item.ingredient_id)?.ok_or(Error::IngredientNotFound)?;
        let kept = before.iter().any(|b| b.ingredient_id == item.ingredient_id);
        if !ingredient.active && !kept {
            return Err(Error::IngredientNotFound);
        }
    }
    Ok(())
}
/// Deducts `used` (ingredient, thousandths) for a sale. Missing ingredients
/// (removed after the recipe was saved) are skipped rather than refusing a
/// sale. The caller owns the IMMEDIATE transaction.
pub(crate) fn use_for_sale(
    conn: &mut SqliteConnection,
    sale_id: Uuid,
    receipt: u64,
    used: &[(Uuid, i64)],
    staff: &Attribution,
    now: i64,
) -> Result<()> {
    for (id, amount) in used {
        let Some(ingredient) = load_one(conn, *id)? else {
            continue;
        };
        let after = ingredient
            .stock
            .checked_sub(*amount)
            .ok_or(pos_domain::DomainError::Overflow)?;
        set_stock(conn, *id, after, now)?;
        insert(
            conn,
            &IngredientMovement {
                id: Uuid::new_v4(),
                ingredient_id: *id,
                created_at: now,
                kind: IngredientKind::Sale,
                before: ingredient.stock,
                after,
                reason: format!("#{receipt:06}"),
                sale_id: Some(sale_id),
                staff: staff.clone(),
            },
        )?;
    }
    Ok(())
}
/// Gives back what a voided sale used. The caller owns the transaction.
pub(crate) fn return_for_void(
    conn: &mut SqliteConnection,
    sale_id: Uuid,
    receipt: u64,
    staff: &Attribution,
    now: i64,
) -> Result<()> {
    let used = ingredient_movements::table
        .filter(ingredient_movements::sale_id.eq(sale_id.to_string()))
        .filter(ingredient_movements::kind.eq(IngredientKind::Sale.key()))
        .select(ingredient_movements::movement_json)
        .load::<String>(conn)?;
    for json in used {
        let sold: IngredientMovement = serde_json::from_str(&json)?;
        let Some(ingredient) = load_one(conn, sold.ingredient_id)? else {
            continue;
        };
        let back = sold
            .before
            .checked_sub(sold.after)
            .ok_or(pos_domain::DomainError::Overflow)?;
        let after = ingredient
            .stock
            .checked_add(back)
            .ok_or(pos_domain::DomainError::Overflow)?;
        set_stock(conn, sold.ingredient_id, after, now)?;
        insert(
            conn,
            &IngredientMovement {
                id: Uuid::new_v4(),
                ingredient_id: sold.ingredient_id,
                created_at: now,
                kind: IngredientKind::Void,
                before: ingredient.stock,
                after,
                reason: format!("#{receipt:06}"),
                sale_id: Some(sale_id),
                staff: staff.clone(),
            },
        )?;
    }
    Ok(())
}
impl Pos {
    /// Active ingredients first, by name.
    pub fn ingredients(&mut self) -> Result<Vec<Ingredient>> {
        let mut all: Vec<Ingredient> = ingredients::table
            .select((ingredients::ingredient_json, ingredients::stock_milli))
            .load::<(String, i64)>(&mut self.conn)?
            .into_iter()
            .map(|(json, stock)| {
                let mut i: Ingredient = serde_json::from_str(&json)?;
                i.stock = stock;
                Ok(i)
            })
            .collect::<Result<_>>()?;
        all.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
        Ok(all)
    }
    /// Adds an ingredient (its stock starts as given, recorded as Initial) or
    /// edits name, unit, warning level, cost and active; an edit never
    /// changes the stock, which moves only through movements and sales.
    pub fn save_ingredient(&mut self, input: &Ingredient, now: i64) -> Result<Ingredient> {
        input.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Stock))?;
            let mut saved = input.clone();
            saved.name = saved.name.trim().to_owned();
            saved.unit = saved.unit.trim().to_owned();
            let taken = ingredients::table
                .select(ingredients::ingredient_json)
                .load::<String>(conn)?
                .into_iter()
                .filter_map(|json| serde_json::from_str::<Ingredient>(&json).ok())
                .any(|i| i.id != saved.id && i.name.eq_ignore_ascii_case(&saved.name));
            if taken {
                return Err(Error::DuplicateIngredient);
            }
            match load_one(conn, saved.id)? {
                Some(existing) => {
                    saved.stock = existing.stock;
                    diesel::update(ingredients::table.find(saved.id.to_string()))
                        .set((
                            ingredients::ingredient_json.eq(serde_json::to_string(&saved)?),
                            ingredients::updated_at.eq(now),
                        ))
                        .execute(conn)?;
                }
                None => {
                    diesel::insert_into(ingredients::table)
                        .values((
                            ingredients::id.eq(saved.id.to_string()),
                            ingredients::ingredient_json.eq(serde_json::to_string(&saved)?),
                            ingredients::stock_milli.eq(saved.stock),
                            ingredients::updated_at.eq(now),
                        ))
                        .execute(conn)?;
                    insert(
                        conn,
                        &IngredientMovement {
                            id: Uuid::new_v4(),
                            ingredient_id: saved.id,
                            created_at: now,
                            kind: IngredientKind::Initial,
                            before: 0,
                            after: saved.stock,
                            reason: String::new(),
                            sale_id: None,
                            staff,
                        },
                    )?;
                }
            }
            Ok(saved)
        })
    }
    /// Receive, remove or count an ingredient. The same request ID returns
    /// the original movement.
    pub fn adjust_ingredient(
        &mut self,
        request: &IngredientRequest,
        now: i64,
    ) -> Result<IngredientMovement> {
        request.validate()?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            if let Some(json) = ingredient_movements::table
                .find(request.id.to_string())
                .select(ingredient_movements::movement_json)
                .first::<String>(conn)
                .optional()?
            {
                let previous: IngredientMovement = serde_json::from_str(&json)?;
                return if previous.ingredient_id == request.ingredient_id
                    && previous.kind == request.kind
                {
                    Ok(previous)
                } else {
                    Err(Error::ConflictingRequest)
                };
            }
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Stock))?;
            let ingredient =
                load_one(conn, request.ingredient_id)?.ok_or(Error::IngredientNotFound)?;
            let after = request.after(ingredient.stock)?;
            set_stock(conn, ingredient.id, after, now)?;
            let movement = IngredientMovement {
                id: request.id,
                ingredient_id: ingredient.id,
                created_at: now,
                kind: request.kind,
                before: ingredient.stock,
                after,
                reason: request.reason.trim().to_owned(),
                sale_id: None,
                staff,
            };
            insert(conn, &movement)?;
            Ok(movement)
        })
    }
    /// Newest first, 50 per page.
    pub fn ingredient_movements(
        &mut self,
        id: Uuid,
        offset: u32,
    ) -> Result<Vec<IngredientMovement>> {
        ingredient_movements::table
            .filter(ingredient_movements::ingredient_id.eq(id.to_string()))
            .order((
                ingredient_movements::created_at.desc(),
                ingredient_movements::id,
            ))
            .limit(50)
            .offset(i64::from(offset))
            .select(ingredient_movements::movement_json)
            .load::<String>(&mut self.conn)?
            .into_iter()
            .map(|json| Ok(serde_json::from_str(&json)?))
            .collect()
    }
}
