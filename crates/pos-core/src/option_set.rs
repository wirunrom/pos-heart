//! Shared option sets: one option group (e.g. sweetness) kept once and used
//! by many products. Loading a product appends its sets' groups to its own
//! `options`, so pricing, selling and option recipes work unchanged; editing
//! a set changes every product that uses it.
use crate::{
    Error, Pos, Result,
    schema::{option_sets, products},
};
use diesel::prelude::*;
use pos_domain::{OptionGroup, Permission, Product};
use uuid::Uuid;

/// All sets in their order.
pub(crate) fn load(conn: &mut SqliteConnection) -> Result<Vec<OptionGroup>> {
    option_sets::table
        .order((option_sets::position, option_sets::id))
        .select(option_sets::set_json)
        .load::<String>(conn)?
        .into_iter()
        .map(|json| Ok(serde_json::from_str(&json)?))
        .collect()
}
/// Appends each product's sets to its options (missing sets are skipped).
pub(crate) fn resolve(
    conn: &mut SqliteConnection,
    mut products: Vec<Product>,
) -> Result<Vec<Product>> {
    if products.iter().all(|p| p.option_sets.is_empty()) {
        return Ok(products);
    }
    let sets = load(conn)?;
    for p in &mut products {
        for id in &p.option_sets {
            if let Some(set) = sets.iter().find(|s| s.id == *id) {
                p.options.push(set.clone());
            }
        }
    }
    Ok(products)
}
pub(crate) fn resolve_one(conn: &mut SqliteConnection, product: Product) -> Result<Product> {
    resolve(conn, vec![product])?
        .pop()
        .ok_or(Error::CorruptData)
}
fn set_links(conn: &mut SqliteConnection, product: &str, sets: &[Uuid]) -> Result<()> {
    diesel::update(products::table.find(product))
        .set(products::option_sets_json.eq(serde_json::to_string(sets)?))
        .execute(conn)?;
    Ok(())
}
/// Option recipe rows of a set's choices go with the set.
fn drop_choice_rows(conn: &mut SqliteConnection, product: &str, set: &OptionGroup) -> Result<()> {
    let json = products::table
        .find(product)
        .select(products::option_recipe_json)
        .first::<String>(conn)?;
    let rows: Vec<pos_domain::OptionRecipe> = serde_json::from_str(&json)?;
    let kept: Vec<_> = rows
        .into_iter()
        .filter(|r| !set.choices.iter().any(|c| c.id == r.choice_id))
        .collect();
    diesel::update(products::table.find(product))
        .set(products::option_recipe_json.eq(serde_json::to_string(&kept)?))
        .execute(conn)?;
    Ok(())
}
fn links(conn: &mut SqliteConnection) -> Result<Vec<(String, Vec<Uuid>)>> {
    products::table
        .filter(products::option_sets_json.ne("[]"))
        .select((products::id, products::option_sets_json))
        .load::<(String, String)>(conn)?
        .into_iter()
        .map(|(id, json)| Ok((id, serde_json::from_str(&json)?)))
        .collect()
}
impl Pos {
    /// Sets in order with how many active products use each.
    pub fn option_sets(&mut self) -> Result<Vec<(OptionGroup, u64)>> {
        self.conn.transaction(|conn| {
            let sets = load(conn)?;
            let used = links(conn)?;
            Ok(sets
                .into_iter()
                .map(|s| {
                    let n = used.iter().filter(|(_, ids)| ids.contains(&s.id)).count() as u64;
                    (s, n)
                })
                .collect())
        })
    }
    /// Adds or edits a set (Catalog). Its choices' IDs must stay stable for
    /// open bills; removed choices make those bills choose again.
    pub fn save_option_set(&mut self, set: &OptionGroup, now: i64) -> Result<()> {
        pos_domain::validate_groups(std::slice::from_ref(set))?;
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let json = serde_json::to_string(set)?;
            let exists = diesel::select(diesel::dsl::exists(
                option_sets::table.find(set.id.to_string()),
            ))
            .get_result::<bool>(conn)?;
            if exists {
                diesel::update(option_sets::table.find(set.id.to_string()))
                    .set((
                        option_sets::set_json.eq(&json),
                        option_sets::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            } else {
                let last = option_sets::table
                    .select(diesel::dsl::max(option_sets::position))
                    .first::<Option<i64>>(conn)?
                    .unwrap_or(0);
                diesel::insert_into(option_sets::table)
                    .values((
                        option_sets::id.eq(set.id.to_string()),
                        option_sets::set_json.eq(&json),
                        option_sets::position.eq(last + 1),
                        option_sets::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            crate::audit::record(
                conn,
                crate::ActivityKind::Product,
                &set.id.to_string(),
                now,
                &staff,
                &serde_json::json!({"action": "save_option_set", "set": set}),
            )?;
            Ok(())
        })
    }
    /// Deletes a set and unlinks it from every product.
    pub fn delete_option_set(&mut self, id: Uuid, now: i64) -> Result<u64> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let set = load(conn)?.into_iter().find(|s| s.id == id);
            let mut unlinked = 0_u64;
            for (product, mut ids) in links(conn)? {
                if ids.contains(&id) {
                    ids.retain(|x| *x != id);
                    if let Some(set) = &set {
                        drop_choice_rows(conn, &product, set)?;
                    }
                    set_links(conn, &product, &ids)?;
                    unlinked += 1;
                }
            }
            if diesel::delete(option_sets::table.find(id.to_string())).execute(conn)? == 0 {
                return Err(Error::OptionSetNotFound);
            }
            crate::audit::record(
                conn,
                crate::ActivityKind::Product,
                &id.to_string(),
                now,
                &staff,
                &serde_json::json!({"action": "delete_option_set", "products": unlinked}),
            )?;
            Ok(unlinked)
        })
    }
    /// Adds (`link`) or removes a set on many products at once.
    pub fn link_option_set(
        &mut self,
        id: Uuid,
        products: &[Uuid],
        link: bool,
        now: i64,
    ) -> Result<u64> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let sets = load(conn)?;
            if !sets.iter().any(|s| s.id == id) {
                return Err(Error::OptionSetNotFound);
            }
            let mut changed = 0_u64;
            for pid in products {
                let p = crate::catalog::find_product(conn, *pid)?;
                let mut ids = p.option_sets.clone();
                let has = ids.contains(&id);
                if link && !has {
                    ids.push(id);
                    // The product with the set must still be valid.
                    let mut groups = p.own_options();
                    groups.extend(ids.iter().filter_map(|i| sets.iter().find(|s| s.id == *i).cloned()));
                    pos_domain::validate_groups(&groups)?;
                } else if !link && has {
                    ids.retain(|x| *x != id);
                    if let Some(set) = sets.iter().find(|s| s.id == id) {
                        drop_choice_rows(conn, &pid.to_string(), set)?;
                    }
                } else {
                    continue;
                }
                set_links(conn, &pid.to_string(), &ids)?;
                changed += 1;
            }
            crate::audit::record(conn, crate::ActivityKind::Product, &id.to_string(), now, &staff,
                &serde_json::json!({"action": if link { "link_option_set" } else { "unlink_option_set" }, "products": changed}))?;
            Ok(changed)
        })
    }
}
