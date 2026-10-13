//! Categories: ordered rows beside the products' category names. Saving or
//! importing a product with a new name adds it at the end; renaming updates
//! every product in the same transaction (into an existing name = merge).
use crate::{
    Error, Pos, Result,
    schema::{categories, products},
};
use diesel::prelude::*;
use pos_domain::Permission;

fn valid(name: &str) -> bool {
    (1..=80).contains(&name.trim().chars().count())
}
/// Adds `name` at the end unless it exists (empty names are "uncategorised").
pub(crate) fn ensure(conn: &mut SqliteConnection, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(());
    }
    let exists = diesel::select(diesel::dsl::exists(categories::table.find(name)))
        .get_result::<bool>(conn)?;
    if !exists {
        let last = categories::table
            .select(diesel::dsl::max(categories::position))
            .first::<Option<i64>>(conn)?
            .unwrap_or(0);
        diesel::insert_into(categories::table)
            .values((categories::name.eq(name), categories::position.eq(last + 1)))
            .execute(conn)?;
    }
    Ok(())
}
/// Category names in display order.
pub(crate) fn ordered(conn: &mut SqliteConnection) -> Result<Vec<String>> {
    Ok(categories::table
        .order((categories::position, categories::name))
        .select(categories::name)
        .load::<String>(conn)?)
}
impl Pos {
    pub fn add_category(&mut self, name: &str, now: i64) -> Result<()> {
        if !valid(name) {
            return Err(Error::InvalidCategory);
        }
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let exists = diesel::select(diesel::dsl::exists(categories::table.find(name.trim())))
                .get_result::<bool>(conn)?;
            if exists {
                return Err(Error::DuplicateCategory);
            }
            ensure(conn, name)?;
            crate::audit::record(
                conn,
                crate::ActivityKind::Product,
                name.trim(),
                now,
                &staff,
                &serde_json::json!({"action": "add_category", "name": name.trim()}),
            )?;
            Ok(())
        })
    }
    /// Renames a category and all its products; into an existing name, the
    /// two merge (the old position is dropped).
    pub fn rename_category(&mut self, old: &str, new: &str, now: i64) -> Result<u64> {
        if !valid(new) {
            return Err(Error::InvalidCategory);
        }
        let (old, new) = (old.trim().to_owned(), new.trim().to_owned());
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let known = diesel::select(diesel::dsl::exists(categories::table.find(&old)))
                .get_result::<bool>(conn)?;
            if !known {
                return Err(Error::CategoryNotFound);
            }
            if old == new {
                return Ok(0);
            }
            let moved = diesel::update(products::table.filter(products::category.eq(&old)))
                .set(products::category.eq(&new))
                .execute(conn)?;
            let target = diesel::select(diesel::dsl::exists(categories::table.find(&new)))
                .get_result::<bool>(conn)?;
            if target {
                diesel::delete(categories::table.find(&old)).execute(conn)?;
            } else {
                diesel::update(categories::table.find(&old))
                    .set(categories::name.eq(&new))
                    .execute(conn)?;
            }
            crate::audit::record(conn, crate::ActivityKind::Product, &new, now, &staff,
                &serde_json::json!({"action": if target { "merge_category" } else { "rename_category" }, "from": old, "to": new, "products": moved}))?;
            Ok(u64::try_from(moved).unwrap_or(0))
        })
    }
    /// Sets the display order: `names` must be exactly the current categories.
    pub fn reorder_categories(&mut self, names: &[String]) -> Result<()> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let mut current = ordered(conn)?;
            let mut wanted = names.to_vec();
            current.sort();
            wanted.sort();
            if current != wanted {
                return Err(Error::CategoryNotFound);
            }
            for (i, name) in names.iter().enumerate() {
                diesel::update(categories::table.find(name))
                    .set(
                        categories::position
                            .eq(i64::try_from(i).map_err(|_| Error::CorruptData)? + 1),
                    )
                    .execute(conn)?;
            }
            Ok(())
        })
    }
    /// Deletes a category; its products move to `into` (another category)
    /// or become uncategorised.
    pub fn delete_category(&mut self, name: &str, into: Option<&str>, now: i64) -> Result<u64> {
        let name = name.trim().to_owned();
        let into = into.map(str::trim).unwrap_or("").to_owned();
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(Permission::Catalog))?;
            let known = diesel::select(diesel::dsl::exists(categories::table.find(&name)))
                .get_result::<bool>(conn)?;
            let target_ok = into.is_empty()
                || (into != name
                    && diesel::select(diesel::dsl::exists(categories::table.find(&into)))
                        .get_result::<bool>(conn)?);
            if !known || !target_ok {
                return Err(Error::CategoryNotFound);
            }
            let moved = diesel::update(products::table.filter(products::category.eq(&name)))
                .set(products::category.eq(&into))
                .execute(conn)?;
            diesel::delete(categories::table.find(&name)).execute(conn)?;
            crate::audit::record(conn, crate::ActivityKind::Product, &name, now, &staff,
                &serde_json::json!({"action": "delete_category", "name": name, "into": into, "products": moved}))?;
            Ok(u64::try_from(moved).unwrap_or(0))
        })
    }
}
