use crate::{
    Error, Pos, Result,
    schema::{held_carts, products},
};
use diesel::{
    prelude::*,
    sql_types::{Integer, Text},
};
use pos_domain::{Money, Product, ProductInput};
use uuid::Uuid;

diesel::define_sql_function! { fn lower(value: Text) -> Text; }
diesel::define_sql_function! { fn instr(value: Text, needle: Text) -> Integer; }

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = products)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
pub(crate) struct ProductRow {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub price_satang: i64,
    pub cost_satang: Option<i64>,
    pub stock: Option<i64>,
    pub active: bool,
    pub barcode: Option<String>,
    pub on_sale: bool,
    pub options_json: String,
    pub service_charge: bool,
    pub kitchen: bool,
    pub recipe_json: String,
    pub option_recipe_json: String,
}
impl TryFrom<ProductRow> for Product {
    type Error = Error;
    fn try_from(row: ProductRow) -> Result<Self> {
        Ok(Self {
            id: Uuid::parse_str(&row.id).map_err(|_| Error::CorruptData)?,
            sku: row.sku,
            name: row.name,
            category: row.category,
            price: Money::from_satang(row.price_satang)?,
            cost: row.cost_satang.map(Money::from_satang).transpose()?,
            stock: row
                .stock
                .map(|n| u32::try_from(n).map_err(|_| Error::CorruptData))
                .transpose()?,
            active: row.active,
            barcode: row.barcode,
            on_sale: row.on_sale,
            options: serde_json::from_str(&row.options_json)?,
            service_charge: row.service_charge,
            kitchen: row.kitchen,
            recipe: serde_json::from_str(&row.recipe_json)?,
            option_recipe: serde_json::from_str(&row.option_recipe_json)?,
        })
    }
}
/// Active products only; checkout separately refuses products that are not for sale.
pub(crate) fn find_product(conn: &mut SqliteConnection, id: Uuid) -> Result<Product> {
    products::table
        .find(id.to_string())
        .filter(products::active.eq(true))
        .select(ProductRow::as_select())
        .first::<ProductRow>(conn)
        .optional()?
        .ok_or(Error::ProductUnavailable)?
        .try_into()
}

/// Saved views shown in the product side navigation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProductView {
    #[default]
    All,
    LowStock,
    OutOfStock,
    NotForSale,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductQuery {
    pub search: String,
    /// Exact category; None for every category.
    pub category: Option<String>,
    pub view: ProductView,
    pub offset: u32,
    pub limit: u32,
}
impl Default for ProductQuery {
    fn default() -> Self {
        Self {
            search: String::new(),
            category: None,
            view: ProductView::All,
            offset: 0,
            limit: 100,
        }
    }
}
/// Counts for the category list and saved views, over active products.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogCounts {
    pub total: u64,
    pub categories: Vec<(String, u64)>,
    pub low_stock: u64,
    pub out_of_stock: u64,
    pub not_for_sale: u64,
}
/// Sample sweetness (required, one) and toppings (optional, many) for drinks.
fn demo_drink_options(coffee: bool) -> Result<Vec<pos_domain::OptionGroup>> {
    use pos_domain::{OptionChoice, OptionGroup};
    let choice = |name: &str, price: &str| -> Result<OptionChoice> {
        Ok(OptionChoice {
            id: Uuid::new_v4(),
            name: name.into(),
            price: price.parse()?,
        })
    };
    let extras = if coffee {
        vec![choice("เพิ่มช็อต / Extra shot", "15")?]
    } else {
        vec![
            choice("ไข่มุก / Pearls", "10")?,
            choice("วิปครีม / Whipped cream", "15")?,
        ]
    };
    Ok(vec![
        OptionGroup {
            id: Uuid::new_v4(),
            name: "ความหวาน / Sweetness".into(),
            required: true,
            multiple: false,
            choices: vec![
                choice("ปกติ / Regular", "0")?,
                choice("หวานน้อย / Less sweet", "0")?,
                choice("ไม่หวาน / No sugar", "0")?,
            ],
        },
        OptionGroup {
            id: Uuid::new_v4(),
            name: "เพิ่ม / Extras".into(),
            required: false,
            multiple: true,
            choices: extras,
        },
    ])
}
fn clean_barcode(barcode: &Option<String>) -> Option<String> {
    barcode
        .as_deref()
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(str::to_owned)
}
impl Pos {
    /// Compatibility search used by tests and simple callers: first 100 by category, name.
    pub fn products(&mut self, search: &str, offset: u32) -> Result<Vec<Product>> {
        self.catalog(&ProductQuery {
            search: search.into(),
            offset,
            ..ProductQuery::default()
        })
    }
    /// Filtered page of active products (search matches name, code, category or barcode).
    pub fn catalog(&mut self, query: &ProductQuery) -> Result<Vec<Product>> {
        let search = query.search.trim();
        let mut q = products::table
            .filter(products::active.eq(true))
            .into_boxed();
        if !search.is_empty() {
            q = q.filter(
                instr(lower(products::name), lower(search))
                    .gt(0)
                    .or(instr(lower(products::sku), lower(search)).gt(0))
                    .or(instr(lower(products::category), lower(search)).gt(0))
                    .or(products::barcode.eq(search)),
            );
        }
        if let Some(category) = &query.category {
            q = q.filter(products::category.eq(category.clone()));
        }
        let low = i64::from(pos_domain::LOW_STOCK);
        q = match query.view {
            ProductView::All => q,
            ProductView::LowStock => q.filter(products::stock.between(1, low)),
            ProductView::OutOfStock => q.filter(products::stock.eq(0)),
            ProductView::NotForSale => q.filter(products::on_sale.eq(false)),
        };
        let rows = q
            .order((products::category, products::name, products::id))
            .limit(i64::from(query.limit.clamp(1, 2000)))
            .offset(i64::from(query.offset))
            .select(ProductRow::as_select())
            .load::<ProductRow>(&mut self.conn)?;
        rows.into_iter().map(Product::try_from).collect()
    }
    pub fn catalog_counts(&mut self) -> Result<CatalogCounts> {
        self.conn.transaction(|conn| {
            let active = || products::table.filter(products::active.eq(true));
            let count = |n: i64| u64::try_from(n).map_err(|_| Error::CorruptData);
            let categories = active()
                .group_by(products::category)
                .select((products::category, diesel::dsl::count_star()))
                .order(products::category)
                .load::<(String, i64)>(conn)?
                .into_iter()
                .map(|(c, n)| Ok((c, count(n)?)))
                .collect::<Result<Vec<_>>>()?;
            let low = i64::from(pos_domain::LOW_STOCK);
            Ok(CatalogCounts {
                total: count(active().count().get_result(conn)?)?,
                categories,
                low_stock: count(
                    active()
                        .filter(products::stock.between(1, low))
                        .count()
                        .get_result(conn)?,
                )?,
                out_of_stock: count(
                    active()
                        .filter(products::stock.eq(0))
                        .count()
                        .get_result(conn)?,
                )?,
                not_for_sale: count(
                    active()
                        .filter(products::on_sale.eq(false))
                        .count()
                        .get_result(conn)?,
                )?,
            })
        })
    }
    /// Exact product code or barcode (scanner input), case-insensitive for codes.
    pub fn find_by_code(&mut self, code: &str) -> Result<Option<Product>> {
        let code = code.trim();
        if code.is_empty() {
            return Ok(None);
        }
        products::table
            .filter(products::active.eq(true))
            .filter(
                lower(products::sku)
                    .eq(lower(code))
                    .or(products::barcode.eq(code)),
            )
            .select(ProductRow::as_select())
            .first::<ProductRow>(&mut self.conn)
            .optional()?
            .map(Product::try_from)
            .transpose()
    }
    pub fn save_product(&mut self, input: ProductInput) -> Result<Product> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.save_product_at(input, now)
    }
    /// Saves a product; starting stock tracking records an `Initial` movement.
    pub fn save_product_at(&mut self, input: ProductInput, now: i64) -> Result<Product> {
        input.validate()?;
        let barcode = clean_barcode(&input.barcode);
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Catalog))?;
            let id = input.id.unwrap_or_else(Uuid::new_v4).to_string();
            let duplicate = diesel::select(diesel::dsl::exists(
                products::table
                    .filter(lower(products::sku).eq(lower(input.sku.trim())))
                    .filter(products::id.ne(&id)),
            ))
            .get_result::<bool>(conn)?;
            if duplicate {
                return Err(Error::DuplicateSku);
            }
            if let Some(code) = &barcode {
                let taken = diesel::select(diesel::dsl::exists(
                    products::table
                        .filter(products::barcode.eq(code))
                        .filter(products::id.ne(&id)),
                ))
                .get_result::<bool>(conn)?;
                if taken {
                    return Err(Error::DuplicateBarcode);
                }
            }
            let previous = match input.id {
                Some(_) => Some(find_product(
                    conn,
                    Uuid::parse_str(&id).map_err(|_| Error::CorruptData)?,
                )?),
                None => None,
            };
            // Option rows are checked like recipe items (sign aside).
            let as_items = |rows: &[pos_domain::OptionRecipe]| -> Vec<pos_domain::RecipeItem> {
                rows.iter()
                    .map(|r| pos_domain::RecipeItem {
                        ingredient_id: r.ingredient_id,
                        amount: r.amount,
                    })
                    .collect()
            };
            let mut before: Vec<pos_domain::RecipeItem> = previous.as_ref().map_or_else(Vec::new, |p| p.recipe.clone());
            before.extend(previous.as_ref().map_or_else(Vec::new, |p| as_items(&p.option_recipe)));
            crate::ingredient::check_recipe(conn, &input.recipe, &before)?;
            crate::ingredient::check_recipe(conn, &as_items(&input.option_recipe), &before)?;
            let was_tracked = previous.as_ref().and_then(|p| p.stock);
            let stock = match (was_tracked, input.stock) {
                (Some(current), Some(_)) if input.keep_tracked_stock => Some(current),
                (_, stock) => stock,
            };
            let product_id = id.clone();
            let row = ProductRow {
                id,
                sku: input.sku.trim().into(),
                name: input.name.trim().into(),
                category: input.category.trim().into(),
                price_satang: input.price.satang(),
                cost_satang: input.cost.map(Money::satang),
                stock: stock.map(i64::from),
                active: true,
                barcode: barcode.clone(),
                on_sale: input.on_sale,
                options_json: serde_json::to_string(&input.options)?,
                service_charge: input.service_charge,
                kitchen: input.kitchen,
                recipe_json: serde_json::to_string(&input.recipe)?,
                option_recipe_json: serde_json::to_string(&input.option_recipe)?,
            };
            diesel::insert_into(products::table)
                .values(&row)
                .on_conflict(products::id)
                .do_update()
                .set(&row)
                .execute(conn)?;
            if let (None, Some(start)) = (was_tracked, stock) {
                crate::stock::insert(
                    conn,
                    &pos_domain::StockMovement {
                        id: Uuid::new_v4(),
                        product_id: Uuid::parse_str(&product_id).map_err(|_| Error::CorruptData)?,
                        created_at: now,
                        kind: pos_domain::StockKind::Initial,
                        before: 0,
                        after: start,
                        reason: String::new(),
                        staff: staff.clone(),
                    },
                )?;
            }
            let saved: Product = row.try_into()?;
            crate::audit::record(conn, crate::ActivityKind::Product, &saved.id.to_string(), now, &staff,
                &serde_json::json!({"action": "save", "name": saved.name, "before": previous, "after": saved}))?;
            Ok(saved)
        })
    }
    pub fn archive_product(&mut self, id: Uuid) -> Result<()> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Catalog))?;
            let previous = find_product(conn, id)?;
            if diesel::update(
                products::table
                    .find(id.to_string())
                    .filter(products::active.eq(true)),
            )
            .set(products::active.eq(false))
            .execute(conn)?
                == 0
            {
                return Err(Error::ProductUnavailable);
            }
            crate::audit::record(conn, crate::ActivityKind::Product, &id.to_string(), crate::audit::now(), &staff,
                &serde_json::json!({"action": "archive", "name": previous.name, "before": previous}))?;
            Ok(())
        })
    }
    /// Bulk "stop selling" / "resume selling". All rows change or none do.
    pub fn set_on_sale(&mut self, ids: &[Uuid], on_sale: bool) -> Result<usize> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Catalog))?;
            let previous = ids.iter().map(|id| find_product(conn, *id)).collect::<Result<Vec<_>>>()?;
            let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
            let changed = diesel::update(
                products::table
                    .filter(products::id.eq_any(&ids))
                    .filter(products::active.eq(true)),
            )
            .set(products::on_sale.eq(on_sale))
            .execute(conn)?;
            if changed != ids.len() {
                return Err(Error::ProductUnavailable);
            }
            for product in previous {
                crate::audit::record(conn, crate::ActivityKind::Product, &product.id.to_string(), crate::audit::now(), &staff,
                    &serde_json::json!({"action": "on_sale", "name": product.name, "before": product.on_sale, "after": on_sale}))?;
            }
            Ok(changed)
        })
    }
    /// Bulk category change. All rows change or none do.
    pub fn set_category(&mut self, ids: &[Uuid], category: &str) -> Result<usize> {
        let category = category.trim();
        if category.chars().count() > 80 {
            return Err(pos_domain::DomainError::TextTooLong.into());
        }
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff = crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Catalog))?;
            let previous = ids.iter().map(|id| find_product(conn, *id)).collect::<Result<Vec<_>>>()?;
            let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
            let changed = diesel::update(
                products::table
                    .filter(products::id.eq_any(&ids))
                    .filter(products::active.eq(true)),
            )
            .set(products::category.eq(category))
            .execute(conn)?;
            if changed != ids.len() {
                return Err(Error::ProductUnavailable);
            }
            for product in previous {
                crate::audit::record(conn, crate::ActivityKind::Product, &product.id.to_string(), crate::audit::now(), &staff,
                    &serde_json::json!({"action": "category", "name": product.name, "before": product.category, "after": category}))?;
            }
            Ok(changed)
        })
    }
    /// Explicit opt-in sample data, never executed on ordinary startup.
    pub fn seed_demo(&mut self) -> Result<()> {
        if products::table.count().get_result::<i64>(&mut self.conn)? > 0 {
            return Ok(());
        }
        let food = "อาหาร / Food";
        let drinks = "เครื่องดื่ม / Drinks";
        let household = "ของใช้ / Household";
        let snacks = "ขนม / Snacks";
        for (sku, name, category, price, stock, barcode, on_sale) in [
            (
                "F-001",
                "ข้าวกะเพราไก่ไข่ดาว / Basil chicken rice + egg",
                food,
                "65",
                None,
                None,
                true,
            ),
            (
                "F-002",
                "ข้าวมันไก่ต้ม / Hainanese chicken rice",
                food,
                "55",
                None,
                None,
                true,
            ),
            (
                "F-003",
                "ชุดข้าวมันไก่ + ซุป + น้ำเก๊กฮวย (พิเศษ) / Chicken rice set + soup + chrysanthemum tea (large)",
                food,
                "89.50",
                None,
                None,
                true,
            ),
            (
                "F-004",
                "ผัดไทยกุ้งสด / Pad thai with prawns",
                food,
                "80",
                None,
                None,
                false,
            ),
            (
                "D-001",
                "ชาไทยเย็น / Thai iced tea",
                drinks,
                "45",
                None,
                None,
                true,
            ),
            (
                "D-002",
                "กาแฟดำเย็น / Iced americano",
                drinks,
                "50",
                None,
                None,
                true,
            ),
            (
                "D-003",
                "น้ำดื่ม 600 มล. / Drinking water 600 ml",
                drinks,
                "10",
                Some(128),
                Some("8851234000017"),
                true,
            ),
            (
                "D-004",
                "โซดา 325 มล. / Soda water 325 ml",
                drinks,
                "15",
                Some(3),
                Some("8851234000024"),
                true,
            ),
            (
                "H-001",
                "ผงซักฟอก 2.7 กก. / Laundry detergent 2.7 kg",
                household,
                "1019",
                Some(6),
                Some("8859876500011"),
                true,
            ),
            (
                "H-002",
                "กระดาษทิชชู่ 6 ม้วน / Toilet tissue, 6 rolls",
                household,
                "119",
                Some(0),
                Some("8859876500028"),
                true,
            ),
            (
                "S-001",
                "ขนมปังสังขยา / Pandan custard toast",
                snacks,
                "35",
                None,
                None,
                true,
            ),
            (
                "S-002",
                "มันฝรั่งทอด 75 ก. / Potato chips 75 g",
                snacks,
                "32",
                Some(24),
                Some("8850000000770"),
                true,
            ),
        ] {
            let options = if sku.starts_with("D-00") && stock.is_none() {
                demo_drink_options(sku == "D-002")?
            } else {
                Vec::new()
            };
            self.save_product(ProductInput {
                id: None,
                sku: sku.into(),
                name: name.into(),
                category: category.into(),
                price: price.parse()?,
                cost: None,
                stock,
                barcode: barcode.map(str::to_owned),
                on_sale,
                options,
                service_charge: !category.starts_with("ของใช้"),
                kitchen: category.starts_with("อาหาร") || category.starts_with("เครื่องดื่ม"),
                recipe: Vec::new(),
                option_recipe: Vec::new(),
                keep_tracked_stock: false,
            })?;
        }
        Ok(())
    }
}
impl Pos {
    pub fn product(&mut self, id: Uuid) -> Result<Product> {
        find_product(&mut self.conn, id)
    }
    pub fn resume(&mut self, id: Uuid) -> Result<(pos_domain::Cart, Vec<Product>)> {
        let json = held_carts::table
            .find(id.to_string())
            .select(held_carts::cart_json)
            .first::<String>(&mut self.conn)
            .optional()?
            .ok_or(Error::CartNotFound)?;
        let cart: pos_domain::Cart = serde_json::from_str(&json)?;
        let products = cart
            .lines
            .iter()
            .map(|line| self.product(line.product_id))
            .collect::<Result<Vec<_>>>()?;
        Ok((cart, products))
    }
}
