//! Bounded UTF-8 CSV import. Adds new products only; confirmation parses and checks again.
use crate::{Error, Pos, Result, catalog::ProductRow, schema::products};
use diesel::prelude::*;
use pos_domain::{Money, ProductInput};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_IMPORT_ROWS: usize = 5000;
#[derive(Clone, Debug)]
pub struct ImportIssue {
    /// Logical CSV record, including the header as row 1.
    pub row: usize,
    pub message: String,
}
#[derive(Clone, Debug)]
pub struct ImportRow {
    pub row: usize,
    pub product: ProductInput,
}
#[derive(Clone, Debug, Default)]
pub struct ImportPreview {
    /// Exact reviewed bytes; confirmation never reads a subsequently changed file.
    pub source: String,
    pub rows: Vec<ImportRow>,
    pub issues: Vec<ImportIssue>,
}
impl ImportPreview {
    pub fn can_import(&self) -> bool {
        !self.rows.is_empty() && self.issues.is_empty()
    }
}
fn invalid(message: &str) -> Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into()
}
/// RFC-style quoting, escaped quotes, CRLF and embedded newlines. Reject malformed quoting.
fn records(source: &str) -> std::result::Result<Vec<Vec<String>>, &'static str> {
    if source.len() > MAX_IMPORT_BYTES {
        return Err("ไฟล์ใหญ่เกิน 2 MiB / File exceeds 2 MiB");
    }
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = source.trim_start_matches('\u{feff}').chars().peekable();
    let mut quoted = false;
    let mut closed = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() && !closed => quoted = true,
            ',' => {
                row.push(std::mem::take(&mut cell));
                closed = false;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                closed = false;
                if rows.len() > MAX_IMPORT_ROWS + 1 {
                    return Err("เกิน 5,000 รายการ / More than 5,000 records");
                }
            }
            '"' => return Err("เครื่องหมายคำพูด CSV ไม่ถูกต้อง / Malformed CSV quote"),
            _ if closed => return Err("มีข้อความหลังคำพูดปิด / Text after closing quote"),
            _ => cell.push(c),
        }
    }
    if quoted {
        return Err("คำพูด CSV ปิดไม่ครบ / Unclosed CSV quote");
    }
    if !row.is_empty() || !cell.is_empty() || closed {
        row.push(cell);
        rows.push(row);
    }
    if rows.len() > MAX_IMPORT_ROWS + 1 {
        return Err("เกิน 5,000 รายการ / More than 5,000 records");
    }
    Ok(rows)
}
fn parse(source: &str) -> ImportPreview {
    let mut out = ImportPreview {
        source: source.into(),
        ..Default::default()
    };
    let records = match records(source) {
        Ok(r) => r,
        Err(message) => {
            out.issues.push(ImportIssue {
                row: 0,
                message: message.into(),
            });
            return out;
        }
    };
    let Some(header) = records.first() else {
        out.issues.push(ImportIssue {
            row: 1,
            message: "ไฟล์ว่าง / Empty file".into(),
        });
        return out;
    };
    let columns: HashMap<_, _> = header
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_lowercase(), i))
        .collect();
    let allowed = [
        "sku",
        "name",
        "category",
        "price",
        "stock",
        "barcode",
        "on_sale",
        "service_charge",
        "cost",
    ];
    if columns.len() != header.len()
        || !["sku", "name", "price"]
            .iter()
            .all(|h| columns.contains_key(*h))
        || columns.keys().any(|h| !allowed.contains(&h.as_str()))
    {
        out.issues.push(ImportIssue { row: 1, message: "หัวตารางต้องมี sku,name,price; รองรับ category,stock,barcode,on_sale,service_charge,cost เท่านั้นและห้ามซ้ำ / Invalid or duplicate headers".into() });
        return out;
    }
    let mut skus = HashSet::new();
    let mut barcodes = HashSet::new();
    for (i, fields) in records.iter().enumerate().skip(1) {
        if fields.iter().all(|s| s.trim().is_empty()) {
            continue;
        }
        let row = i + 1;
        let parsed = (|| -> std::result::Result<ProductInput, String> {
            if fields.len() != header.len() {
                return Err("จำนวนช่องไม่ตรงหัวตาราง / Wrong field count".into());
            }
            let get = |key: &str| columns.get(key).map_or("", |i| fields[*i].trim());
            let boolean = |key: &str| match get(key) {
                "" | "1" | "true" => Ok(true),
                "0" | "false" => Ok(false),
                _ => Err(format!("{key}: ใช้ 1/0 หรือ true/false / Invalid boolean")),
            };
            let product =
                ProductInput {
                    sku: get("sku").into(),
                    name: get("name").into(),
                    category: get("category").into(),
                    price: get("price").parse::<Money>().map_err(|_| {
                        "price: ราคาไม่ถูกต้อง (ทศนิยมไม่เกิน 2 ตำแหน่ง) / Invalid price".to_owned()
                    })?,
                    cost: if get("cost").is_empty() {
                        None
                    } else {
                        Some(
                            get("cost")
                                .parse::<Money>()
                                .map_err(|_| "cost: ต้นทุนไม่ถูกต้อง / Invalid cost".to_owned())?,
                        )
                    },
                    stock: if get("stock").is_empty() {
                        None
                    } else {
                        Some(get("stock").parse::<u32>().map_err(|_| {
                            "stock: ต้องเป็นจำนวนเต็มไม่ติดลบ / Invalid stock".to_owned()
                        })?)
                    },
                    barcode: (!get("barcode").is_empty()).then(|| get("barcode").into()),
                    on_sale: boolean("on_sale")?,
                    service_charge: boolean("service_charge")?,
                    ..Default::default()
                };
            product
                .validate()
                .map_err(|e| format!("ตรวจชื่อ/รหัส/ความยาว / Check product fields: {e}"))?;
            if !skus.insert(product.sku.to_ascii_lowercase()) {
                return Err("รหัสสินค้าซ้ำในไฟล์ / Duplicate SKU in file".into());
            }
            if let Some(code) = &product.barcode
                && !barcodes.insert(code.clone())
            {
                return Err("บาร์โค้ดซ้ำในไฟล์ / Duplicate barcode in file".into());
            }
            Ok(product)
        })();
        match parsed {
            Ok(product) => out.rows.push(ImportRow { row, product }),
            Err(message) => out.issues.push(ImportIssue { row, message }),
        }
    }
    if out.rows.is_empty() && out.issues.is_empty() {
        out.issues.push(ImportIssue {
            row: 2,
            message: "ไม่มีสินค้า / No products".into(),
        });
    }
    out
}
fn check_catalog(conn: &mut SqliteConnection, preview: &mut ImportPreview) -> Result<()> {
    let existing = products::table
        .select((products::sku, products::barcode))
        .load::<(String, Option<String>)>(conn)?;
    let skus: HashSet<_> = existing
        .iter()
        .map(|(s, _)| s.to_ascii_lowercase())
        .collect();
    let barcodes: HashSet<_> = existing.into_iter().filter_map(|(_, b)| b).collect();
    for row in &preview.rows {
        if skus.contains(&row.product.sku.to_ascii_lowercase()) {
            preview.issues.push(ImportIssue {
                row: row.row,
                message:
                    "รหัสมีอยู่แล้ว (รวมสินค้าที่ลบจากรายการ) / SKU already exists (including archived)"
                        .into(),
            });
        }
        if row
            .product
            .barcode
            .as_ref()
            .is_some_and(|b| barcodes.contains(b))
        {
            preview.issues.push(ImportIssue {
                row: row.row,
                message: "บาร์โค้ดมีอยู่แล้ว / Barcode already exists".into(),
            });
        }
    }
    Ok(())
}
impl Pos {
    pub fn preview_product_import(&mut self, source: &str) -> Result<ImportPreview> {
        let mut preview = parse(source);
        check_catalog(&mut self.conn, &mut preview)?;
        Ok(preview)
    }
    /// All additions and initial stock records commit together. Never updates existing stock.
    pub fn import_products(&mut self, source: &str) -> Result<usize> {
        let mut preview = parse(source);
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Catalog))?;
            check_catalog(conn, &mut preview)?;
            if !preview.can_import() {
                return Err(invalid(
                    "นำเข้าไม่สำเร็จ กรุณาตรวจไฟล์ใหม่ / Import refused; preview the file again",
                ));
            }
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            for row in &preview.rows {
                let p = &row.product;
                let id = Uuid::new_v4();
                let row = ProductRow {
                    id: id.to_string(),
                    sku: p.sku.clone(),
                    name: p.name.clone(),
                    category: p.category.clone(),
                    price_satang: p.price.satang(),
                    cost_satang: p.cost.map(|c| c.satang()),
                    stock: p.stock.map(i64::from),
                    active: true,
                    barcode: p.barcode.clone(),
                    on_sale: p.on_sale,
                    options_json: "[]".into(),
                    service_charge: p.service_charge,
                    kitchen: p.kitchen,
                    recipe_json: "[]".into(),
                    option_recipe_json: "[]".into(),
                    image_rev: None,
                };
                crate::category::ensure(conn, &row.category)?;
                diesel::insert_into(products::table)
                    .values(&row)
                    .execute(conn)?;
                crate::audit::record(
                    conn,
                    crate::ActivityKind::Product,
                    &id.to_string(),
                    now,
                    &staff,
                    &serde_json::json!({"action":"csv_import", "name":p.name, "before":null,
                        "after":{"id":id,"sku":p.sku,"name":p.name,"category":p.category,
                            "price":p.price,"cost":p.cost,"stock":p.stock,"barcode":p.barcode,
                            "on_sale":p.on_sale,"service_charge":p.service_charge}}),
                )?;
                if let Some(after) = p.stock {
                    crate::stock::insert(
                        conn,
                        &pos_domain::StockMovement {
                            id: Uuid::new_v4(),
                            product_id: id,
                            created_at: now,
                            kind: pos_domain::StockKind::Initial,
                            before: 0,
                            after,
                            reason: "CSV import".into(),
                            staff: staff.clone(),
                        },
                    )?;
                }
            }
            Ok(preview.rows.len())
        })
    }
}
