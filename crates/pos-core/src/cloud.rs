//! Owner edits from the cloud (multi-device phase 2), applied on the main
//! register. They run as the fixed "Cloud" member: allowed without a
//! signed-in staff member and attributed to Cloud in Activity and stock
//! history. Only the desktop worker calls this (never a protocol request),
//! so a paired device cannot use the Cloud identity.
use crate::{Error, Pos, Result, catalog::find_any_product, schema::products};
use diesel::prelude::*;
use pos_domain::{Money, ProductInput, StaffRef};
use serde_json::{Map, Value};
use uuid::Uuid;

/// The Cloud member's ID on Activity and stock records.
pub const CLOUD_STAFF: Uuid = Uuid::from_u128(0xC10D);

pub(crate) fn cloud_ref() -> StaffRef {
    StaffRef {
        id: CLOUD_STAFF,
        name: "Cloud".into(),
        role: "cloud".into(),
        role_name: "คลาวด์ (เจ้าของ)".into(),
    }
}

/// The patch fields as typed values; anything else is refused.
#[derive(Default)]
struct Patch {
    name: Option<String>,
    sku: Option<String>,
    category: Option<String>,
    price: Option<Money>,
    on_sale: Option<bool>,
    active: Option<bool>,
}
fn parse(patch: &Map<String, Value>) -> Result<Patch> {
    let bad = || Error::InvalidCloudChange;
    let text = |v: &Value| v.as_str().map(str::to_owned).ok_or_else(bad);
    let mut p = Patch::default();
    for (key, value) in patch {
        match key.as_str() {
            "name" => p.name = Some(text(value)?),
            "sku" => p.sku = Some(text(value)?),
            "category" => p.category = Some(text(value)?),
            "price" => {
                p.price = Some(Money::from_satang(value.as_i64().ok_or_else(bad)?)?);
            }
            "on_sale" => p.on_sale = Some(value.as_bool().ok_or_else(bad)?),
            "active" => p.active = Some(value.as_bool().ok_or_else(bad)?),
            _ => return Err(bad()),
        }
    }
    Ok(p)
}
impl Patch {
    fn edits(&self) -> bool {
        self.name.is_some()
            || self.sku.is_some()
            || self.category.is_some()
            || self.price.is_some()
            || self.on_sale.is_some()
    }
    fn apply(self, input: &mut ProductInput) {
        if let Some(v) = self.name {
            input.name = v;
        }
        if let Some(v) = self.sku {
            input.sku = v;
        }
        if let Some(v) = self.category {
            input.category = v;
        }
        if let Some(v) = self.price {
            input.price = v;
        }
        if let Some(v) = self.on_sale {
            input.on_sale = v;
        }
    }
}

impl Pos {
    /// Applies one owner edit: `kind` "product", `id` the product (an unknown
    /// ID adds it, which needs sku, name and price), `patch` the fields.
    /// Refusals (duplicate SKU, invalid values) leave the shop unchanged and
    /// are reported back to the owner.
    pub fn apply_cloud_change(
        &mut self,
        kind: &str,
        id: &str,
        patch: &Map<String, Value>,
        now: i64,
    ) -> Result<()> {
        if kind != "product" {
            return Err(Error::InvalidCloudChange);
        }
        let id = Uuid::parse_str(id).map_err(|_| Error::InvalidCloudChange)?;
        let patch = parse(patch)?;
        let cloud = crate::staff::Session::cloud();
        let saved = std::mem::replace(&mut self.session, cloud);
        let result = self.apply_product_patch(id, patch, now);
        self.session = saved;
        result
    }
    fn apply_product_patch(&mut self, id: Uuid, patch: Patch, now: i64) -> Result<()> {
        let active = patch.active;
        let Some(p) = find_any_product(&mut self.conn, id)? else {
            if active == Some(false) {
                return Ok(());
            }
            if patch.sku.is_none() || patch.name.is_none() || patch.price.is_none() {
                return Err(Error::InvalidCloudChange);
            }
            let mut input = ProductInput {
                id: Some(id),
                ..ProductInput::default()
            };
            patch.apply(&mut input);
            self.save_product_at(input, now)?;
            return Ok(());
        };
        if !p.active {
            if active != Some(true) {
                return Err(Error::ProductUnavailable);
            }
            self.conn.immediate_transaction(|conn| {
                diesel::update(products::table.find(id.to_string()))
                    .set(products::active.eq(true))
                    .execute(conn)?;
                crate::audit::record(
                    conn,
                    crate::ActivityKind::Product,
                    &id.to_string(),
                    now,
                    &pos_domain::Attribution {
                        by: Some(cloud_ref()),
                        approved_by: None,
                    },
                    &serde_json::json!({"action": "restore", "name": p.name}),
                )
            })?;
        }
        if patch.edits() {
            let mut input = ProductInput {
                id: Some(p.id),
                sku: p.sku.clone(),
                name: p.name.clone(),
                category: p.category.clone(),
                price: p.price,
                cost: p.cost,
                stock: p.stock,
                barcode: p.barcode.clone(),
                on_sale: p.on_sale,
                options: p.own_options(),
                service_charge: p.service_charge,
                kitchen: p.kitchen,
                recipe: p.recipe.clone(),
                option_recipe: p.option_recipe.clone(),
                option_sets: p.option_sets.clone(),
                keep_tracked_stock: true,
            };
            patch.apply(&mut input);
            self.save_product_at(input, now)?;
        }
        if active == Some(false) && p.active {
            self.archive_product(id)?;
        }
        Ok(())
    }
}
