//! Development aid: fills a data folder with sample products, a closed and an
//! open shift, paid/refunded receipts, held bills and an open bill, so native
//! layouts can be captured with `--page`, `--preview` and `--screenshot`.
//! Never run against a real shop folder.
use pos_core::Pos;
use pos_domain::{
    Cart, CartLine, CashKind, CashMovement, LineOption, Money, PaymentMethod, PaymentRequest,
    Product, VoidRequest,
};

/// Required groups take a choice; `extra` also adds the first optional choice.
fn options_for(p: &Product, extra: bool) -> Vec<LineOption> {
    let mut out = Vec::new();
    for g in &p.options {
        let take = if g.required {
            g.choices.get(usize::from(extra))
        } else if extra {
            g.choices.first()
        } else {
            None
        };
        if let Some(c) = take {
            out.push(LineOption {
                group_id: g.id,
                choice_id: c.id,
            });
        }
    }
    out
}
/// Prices a sample bill exactly as checkout will (service, VAT, rounding).
fn total(
    pos: &mut Pos,
    products: &[Product],
    cart: &Cart,
    method: PaymentMethod,
) -> Result<Money, Box<dyn std::error::Error>> {
    let options = pos_domain::PriceOptions::for_bill(
        &pos.pricing_settings()?,
        &pos.tax_settings()?,
        cart.service_charge,
        method == PaymentMethod::Cash,
    );
    let lines = cart
        .lines
        .iter()
        .map(|l| {
            let p = products
                .iter()
                .find(|p| p.id == l.product_id)
                .ok_or(pos_domain::DomainError::MissingProductFields)?;
            Ok((
                p.unit_price(&l.options)?.checked_mul(l.quantity)?,
                l.discount,
                p.service_charge,
            ))
        })
        .collect::<Result<Vec<_>, pos_domain::DomainError>>()?;
    Ok(pos_domain::price_sale(&lines, cart.discount, &options)?.total)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).ok_or("usage: seed_screens DIR")?);
    std::fs::create_dir_all(&dir)?;
    let mut pos = Pos::open(dir.join("pos-heart.sqlite3"))?;
    // Never mix sample data into a database that already holds anything
    // (an empty one, e.g. from launching the app first, is fine).
    let used = !pos.products("", 0)?.is_empty()
        || !pos.history(0)?.is_empty()
        || !pos.held()?.is_empty()
        || pos.active_shift()?.is_some()
        || !pos.shift_history(0)?.is_empty();
    if used {
        return Err(format!(
            "Refusing to seed {}: its database already has data. Use a new folder.",
            dir.display()
        )
        .into());
    }
    // `--staff`: sample owner (PIN 9999), manager (5555) and cashiers
    // นิด (1111) and ต้น (2222); the app then opens on the sign-in screen.
    let staff = std::env::args().any(|a| a == "--staff");
    // `--vat`: a sample VAT-registered company (fictional tax ID with a
    // valid check digit); receipts become abbreviated tax invoices.
    let vat = std::env::args().any(|a| a == "--vat");
    if vat {
        let first12 = "010555512345";
        let sum: u32 = first12
            .chars()
            .filter_map(|c| c.to_digit(10))
            .enumerate()
            .map(|(i, d)| d * (13 - i as u32))
            .sum();
        pos.set_tax_settings(&pos_domain::TaxSettings {
            enabled: true,
            seller_name: "บริษัท ร้านหัวใจ จำกัด (ตัวอย่าง)".into(),
            tax_id: format!("{first12}{}", (11 - sum % 11) % 10),
            branch: "00000".into(),
            address: "99 ถนนตัวอย่าง แขวงตัวอย่าง เขตตัวอย่าง กรุงเทพฯ 10000".into(),
            rate_bp: pos_domain::DEFAULT_VAT_BP,
            pos_id: "E000000000000".into(),
            prices_include_vat: true,
        })?;
    }
    // `--restaurant`: 10% service charge and cash rounded down to 25 satang.
    if std::env::args().any(|a| a == "--restaurant") {
        pos.set_pricing_settings(&pos_domain::PricingSettings {
            service_bp: 1_000,
            rounding: pos_domain::CashRounding {
                step: 25,
                mode: pos_domain::RoundMode::Down,
            },
        })?;
    }
    let mut cashiers = Vec::new();
    let mut manager = None;
    if staff {
        let add = |pos: &mut Pos, name: &str, role: &str, pin: &str| {
            pos.save_staff(
                &pos_domain::StaffInput {
                    id: None,
                    name: name.into(),
                    role: Some(role.into()),
                    pin: Some(pin.into()),
                    active: true,
                },
                0,
            )
        };
        add(&mut pos, "สมชาย", pos_domain::OWNER, "9999")?;
        let m = add(&mut pos, "มานี", pos_domain::MANAGER, "5555")?;
        cashiers.push((add(&mut pos, "นิด", pos_domain::CASHIER, "1111")?.id, "1111"));
        cashiers.push((add(&mut pos, "ต้น", pos_domain::CASHIER, "2222")?.id, "2222"));
        // A custom role: refunds alone and approves them, never moves cash.
        let mut lead = pos_domain::RoleDef::cashier(2_000);
        lead.id = String::new();
        lead.name = "หัวหน้ากะ".into();
        lead.can_approve = true;
        lead.access
            .insert(pos_domain::Permission::Refund, pos_domain::Access::Allow);
        lead.access
            .insert(pos_domain::Permission::Void, pos_domain::Access::Allow);
        lead.access.insert(
            pos_domain::Permission::CashMovement,
            pos_domain::Access::Deny,
        );
        let lead = pos.save_role(&lead)?;
        add(&mut pos, "เปิ้ล", &lead.id, "7777")?;
        pos.sign_in(m.id, "5555")?;
        manager = Some(m.id);
    }
    pos.seed_demo()?;
    let products = pos.products("", 0)?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let start = now - 6 * 3600;
    // Two weeks of earlier trading for period reports, in one long shift.
    let history = uuid::Uuid::new_v4();
    let day = 86_400;
    pos.open_shift(history, "1000".parse()?, now - 14 * day)?;
    let everyday: Vec<_> = products
        .iter()
        .filter(|p| p.on_sale && p.stock.is_none() && p.options.is_empty())
        .cloned()
        .collect();
    let mut last = now - 14 * day;
    for d in (3..14_i64).rev() {
        for k in 0..(2 + d % 4) {
            let p = &everyday[usize::try_from(d + k).unwrap_or(0) % everyday.len()];
            let mut cart = Cart::new();
            cart.lines.push(CartLine {
                product_id: p.id,
                quantity: 1 + u32::try_from(k % 3).unwrap_or(0),
                ..Default::default()
            });
            let due = total(&mut pos, &products, &cart, PaymentMethod::Cash)?;
            last = now - d * day + 3600 * (2 + k);
            pos.checkout_with_payment(
                &cart,
                PaymentRequest {
                    method: PaymentMethod::Cash,
                    amount: due,
                },
                Some(history),
                last,
            )?;
        }
    }
    let h = pos.active_shift()?.ok_or("history shift")?;
    let counted = Money::from_satang(h.expected()?.satang())?;
    pos.close_shift(history, h.revision, counted, last + 60)?;
    // A closed shift from yesterday, then today's open shift.
    let old = uuid::Uuid::new_v4();
    pos.open_shift(old, "1000".parse()?, start - 30 * 3600)?;
    let s = pos.active_shift()?.ok_or("shift")?;
    pos.close_shift(old, s.revision, "990".parse()?, start - 20 * 3600)?;
    let shift = uuid::Uuid::new_v4();
    pos.open_shift(shift, "1000".parse()?, start)?;
    let sellable: Vec<_> = products
        .iter()
        .filter(|p| p.on_sale && p.stock.is_none_or(|n| n > 10))
        .cloned()
        .collect();
    let mut t = start + 600;
    let mut first = None;
    for i in 0..18 {
        if let Some((id, pin)) = cashiers.get(i % 2) {
            pos.sign_in(*id, pin)?;
        }
        let mut cart = Cart::new();
        for j in 0..(1 + i % 3) {
            let p = &sellable[(i * 3 + j) % sellable.len()];
            cart.lines.push(CartLine {
                product_id: p.id,
                quantity: 1 + (i as u32 % 2),
                note: if j == 0 && i % 4 == 0 {
                    "แยกน้ำแข็ง".into()
                } else {
                    String::new()
                },
                options: options_for(p, i % 2 == 0),
                ..Default::default()
            });
        }
        if i == 17 {
            // The latest receipt carries a line and a bill discount.
            cart.lines[0].discount = Some(pos_domain::Discount::Percent(1_000));
            cart.discount = Some(pos_domain::Discount::Amount(Money::from_satang(2_000)?));
            cart.discount_reason = "สมาชิก".into();
        }
        let method = match i % 5 {
            3 => PaymentMethod::Transfer,
            4 => PaymentMethod::Card,
            _ => PaymentMethod::Cash,
        };
        let due = total(&mut pos, &products, &cart, method)?;
        let amount = if method == PaymentMethod::Cash {
            Money::from_satang((due.satang() / 10_000 + 1) * 10_000)?
        } else {
            due
        };
        if i % 6 == 5 {
            cart.label = format!("โต๊ะ {}", i / 6 + 1);
        }
        let sale =
            pos.checkout_with_payment(&cart, PaymentRequest { method, amount }, Some(shift), t)?;
        first.get_or_insert(sale);
        t += 1100 + (i as i64 % 4) * 400;
    }
    if let Some(id) = manager {
        pos.sign_in(id, "5555")?;
    }
    if let Some(sale) = first {
        pos.void_sale_for_shift(
            &VoidRequest {
                sale_id: sale.id,
                refund_method: sale.payment_method,
                reason: "ลูกค้าเปลี่ยนใจ".into(),
                legacy_restock: None,
            },
            Some(shift),
            t,
        )?;
    }
    // A partial refund: one unit of the first line of the last multi-line sale.
    if let Some(record) = pos
        .history(0)?
        .into_iter()
        .find(|r| r.sale.lines.len() > 1 && r.cancellation.is_none())
    {
        pos.refund_items(
            &pos_domain::RefundRequest {
                id: uuid::Uuid::new_v4(),
                sale_id: record.sale.id,
                refund_method: record.sale.payment_method,
                reason: "ลูกค้าคืน 1 ชิ้น".into(),
                lines: vec![pos_domain::RefundLine {
                    line: 0,
                    quantity: 1,
                }],
                legacy_restock: None,
            },
            Some(shift),
            t + 5,
        )?;
    }
    let cash_in = CashMovement {
        id: uuid::Uuid::new_v4(),
        shift_id: shift,
        created_at: t + 10,
        kind: CashKind::CashIn,
        amount: "500".parse()?,
        reason: "แลกเหรียญ".into(),
        payment_method: PaymentMethod::Cash,
        staff: Default::default(),
    };
    pos.move_cash(&cash_in)?;
    pos.move_cash(&CashMovement {
        id: uuid::Uuid::new_v4(),
        kind: CashKind::CashOut,
        amount: "220".parse()?,
        reason: "ซื้อน้ำแข็ง".into(),
        created_at: t + 20,
        ..cash_in
    })?;
    for (label, n) in [("โต๊ะริมหน้าต่าง", 2), ("กลับบ้าน · คุณเอ", 1)]
    {
        let mut cart = Cart::new();
        cart.label = label.into();
        for p in sellable.iter().take(n + 1) {
            cart.lines.push(CartLine {
                product_id: p.id,
                quantity: 1,
                options: options_for(p, false),
                ..Default::default()
            });
        }
        pos.hold(&cart, t + 30)?;
    }
    let mut current = Cart::new();
    current.label = "โต๊ะริมหน้าต่าง 2".into();
    let tea = products.iter().find(|p| p.sku == "D-001").ok_or("tea")?;
    current.lines.push(CartLine {
        product_id: tea.id,
        quantity: 2,
        options: options_for(tea, true),
        ..Default::default()
    });
    for (i, p) in products
        .iter()
        .filter(|p| p.on_sale && p.stock.is_none_or(|n| n > 0) && p.options.is_empty())
        .take(4)
        .enumerate()
    {
        current.lines.push(CartLine {
            product_id: p.id,
            quantity: 1 + (i as u32 % 3),
            note: if i == 1 {
                "แยกน้ำแข็ง".into()
            } else {
                String::new()
            },
            ..Default::default()
        });
    }
    pos.hold(&current, t + 40)?;
    std::fs::write(dir.join("current-bill.txt"), current.id.to_string())?;
    // `--tables`: two zones, open bills on three tables and kitchen tickets.
    if std::env::args().any(|a| a == "--tables") {
        let mut tables = Vec::new();
        for (zone, prefix, n, seats) in [("ในร้าน", "T", 8, 4), ("ระเบียง", "P", 4, 2)]
        {
            for i in 1..=n {
                tables.push(pos.save_table(&pos_domain::DiningTable {
                    id: uuid::Uuid::new_v4(),
                    zone: zone.into(),
                    name: format!("{prefix}{i}"),
                    seats,
                    active: true,
                })?);
            }
        }
        let food: Vec<_> = products
            .iter()
            .filter(|p| p.kitchen && p.on_sale)
            .cloned()
            .collect();
        let drink = products.iter().find(|p| !p.kitchen && p.on_sale).cloned();
        let real_now = time::OffsetDateTime::now_utc().unix_timestamp();
        for (table, guests, minutes, more) in [(1, 4, 24, true), (4, 2, 12, false), (9, 2, 3, true)]
        {
            let mut cart = Cart::new();
            cart.table = Some(tables[table].id);
            cart.label = tables[table].name.clone();
            cart.guests = guests;
            for (k, p) in food.iter().take(3).enumerate() {
                cart.lines.push(CartLine {
                    product_id: p.id,
                    quantity: 1 + (k as u32 % 2),
                    note: if k == 0 {
                        "ไม่เผ็ด".into()
                    } else {
                        String::new()
                    },
                    options: options_for(p, true),
                    ..Default::default()
                });
            }
            if let Some(d) = &drink {
                cart.lines.push(CartLine {
                    product_id: d.id,
                    quantity: guests,
                    ..Default::default()
                });
            }
            let (mut stored, _) = pos.send_to_kitchen(&cart, real_now - minutes * 60)?;
            if more {
                // A second round: one more dish not yet sent.
                if let Some(p) = food.get(3) {
                    stored.lines.push(CartLine {
                        product_id: p.id,
                        quantity: 1,
                        options: options_for(p, false),
                        ..Default::default()
                    });
                }
                pos.hold(&stored, real_now - 60)?;
            }
        }
        // An older ticket the kitchen already finished, for recall.
        let tickets = pos.kitchen_tickets()?;
        if let Some(first) = tickets.first() {
            pos.set_ticket_line(first.id, 0, true)?;
        }
    }
    if vat {
        // A full tax invoice for the second-latest sale.
        if let Some(record) = pos.history(0)?.get(1) {
            let first12 = "010555598765";
            let sum: u32 = first12
                .chars()
                .filter_map(|c| c.to_digit(10))
                .enumerate()
                .map(|(i, d)| d * (13 - i as u32))
                .sum();
            pos.issue_tax_invoice(
                record.sale.id,
                &pos_domain::Buyer {
                    name: "บริษัท ลูกค้าตัวอย่าง จำกัด".into(),
                    tax_id: format!("{first12}{}", (11 - sum % 11) % 10),
                    branch: "00001".into(),
                    address: "1 ถนนตัวอย่าง เชียงใหม่ 50000".into(),
                },
                record.sale.created_at + 60,
            )?;
            // One item returned afterwards: its credit note (ใบลดหนี้).
            if let Some(shift) = pos.active_shift()? {
                pos.refund_items(
                    &pos_domain::RefundRequest {
                        id: uuid::Uuid::new_v4(),
                        sale_id: record.sale.id,
                        refund_method: record.sale.paid_by()[0].method,
                        reason: "สินค้าชำรุด".into(),
                        lines: vec![pos_domain::RefundLine {
                            line: 0,
                            quantity: 1,
                        }],
                        legacy_restock: Some(Vec::new()),
                    },
                    Some(shift.id),
                    record.sale.created_at + 120,
                )?;
            }
        }
    }
    // `--ingredients`: rice, eggs, pork, basil (low) and a recipe for the
    // basil rice; then one plate sold so the history shows a sale.
    if std::env::args().any(|a| a == "--ingredients") {
        use pos_domain::{Ingredient, MILLI, RecipeItem};
        let real_now = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut add = |name: &str,
                       unit: &str,
                       stock: i64,
                       low: i64,
                       cost: &str|
         -> Result<Ingredient, Box<dyn std::error::Error>> {
            Ok(pos.save_ingredient(
                &Ingredient {
                    id: uuid::Uuid::new_v4(),
                    name: name.into(),
                    unit: unit.into(),
                    stock,
                    low,
                    cost: Some(cost.parse()?),
                    active: true,
                },
                real_now - 3600,
            )?)
        };
        let rice = add("ข้าวสวย", "กรัม", 5000 * MILLI, 1000 * MILLI, "0.04")?;
        let egg = add("ไข่ไก่", "ฟอง", 24 * MILLI, 10 * MILLI, "4.50")?;
        let pork = add("หมูสับ", "กรัม", 1800 * MILLI, 500 * MILLI, "0.18")?;
        let basil = add("ใบกะเพรา", "กรัม", 120 * MILLI, 300 * MILLI, "0.10")?;
        let products = pos.products("", 0)?;
        if let Some(p) = products.iter().find(|p| p.name.contains("Basil")) {
            let mut input = pos_domain::ProductInput {
                id: Some(p.id),
                sku: p.sku.clone(),
                name: p.name.clone(),
                category: p.category.clone(),
                price: p.price,
                cost: p.cost,
                stock: p.stock,
                barcode: p.barcode.clone(),
                on_sale: p.on_sale,
                options: p.options.clone(),
                service_charge: p.service_charge,
                kitchen: p.kitchen,
                keep_tracked_stock: true,
                recipe: Vec::new(),
                option_recipe: Vec::new(),
                option_sets: p.option_sets.clone(),
            };
            // An egg option: "+ fried egg" uses one more, "no egg" one less.
            let (extra, none) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
            input.options.push(pos_domain::OptionGroup {
                id: uuid::Uuid::new_v4(),
                name: "ไข่ / Egg".into(),
                required: false,
                multiple: false,
                choices: vec![
                    pos_domain::OptionChoice {
                        id: extra,
                        name: "+ไข่ดาว / Fried egg".into(),
                        price: "10".parse()?,
                    },
                    pos_domain::OptionChoice {
                        id: none,
                        name: "ไม่ใส่ไข่ / No egg".into(),
                        price: pos_domain::Money::ZERO,
                    },
                ],
            });
            input.option_recipe = vec![
                pos_domain::OptionRecipe {
                    choice_id: extra,
                    ingredient_id: egg.id,
                    amount: MILLI,
                },
                pos_domain::OptionRecipe {
                    choice_id: none,
                    ingredient_id: egg.id,
                    amount: -MILLI,
                },
            ];
            input.recipe = vec![
                RecipeItem {
                    ingredient_id: rice.id,
                    amount: 150 * MILLI,
                },
                RecipeItem {
                    ingredient_id: egg.id,
                    amount: MILLI,
                },
                RecipeItem {
                    ingredient_id: pork.id,
                    amount: 100 * MILLI,
                },
                RecipeItem {
                    ingredient_id: basil.id,
                    amount: 10 * MILLI,
                },
            ];
            let saved = pos.save_product(input)?;
            // Two plates now, one with an extra egg, so the report has use.
            if let Some(shift) = pos.active_shift()? {
                let group = saved.options.last().map(|g| g.id).unwrap_or_default();
                let mut cart = pos_domain::Cart::new();
                cart.lines.push(pos_domain::CartLine {
                    product_id: saved.id,
                    quantity: 1,
                    ..Default::default()
                });
                cart.lines.push(pos_domain::CartLine {
                    product_id: saved.id,
                    quantity: 1,
                    options: vec![pos_domain::LineOption {
                        group_id: group,
                        choice_id: extra,
                    }],
                    ..Default::default()
                });
                pos.checkout_for_shift(&cart, "500".parse()?, Some(shift.id), real_now - 1800)?;
            }
        }
    }
    // `--pictures`: generated sample pictures (coloured bands, no photos)
    // for most products, so Sell tiles and the editor show pictures.
    if std::env::args().any(|a| a == "--pictures") {
        let real_now = time::OffsetDateTime::now_utc().unix_timestamp();
        for (n, p) in pos.products("", 0)?.into_iter().enumerate() {
            if n % 4 == 3 {
                continue;
            }
            let hue = (n as u32 * 47) % 255;
            let img = image::RgbaImage::from_fn(256, 192, |x, y| {
                let band = (x / 32 + y / 32) % 2 == 0;
                let base = [hue as u8, (255 - hue) as u8, ((hue * 3) % 255) as u8];
                let k = if band { 1.0 } else { 0.8 };
                image::Rgba([
                    (f32::from(base[0]) * k) as u8,
                    (f32::from(base[1]) * k) as u8,
                    (f32::from(base[2]) * k) as u8,
                    255,
                ])
            });
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png)?;
            pos.set_product_image(p.id, Some(&png.into_inner()), real_now - 100)?;
        }
    }
    // `--sets`: a shared cup-size set used by every drink.
    if std::env::args().any(|a| a == "--sets") {
        let real_now = time::OffsetDateTime::now_utc().unix_timestamp();
        let size = pos_domain::OptionGroup {
            id: uuid::Uuid::new_v4(),
            name: "ขนาดแก้ว / Cup size".into(),
            required: true,
            multiple: false,
            choices: vec![
                pos_domain::OptionChoice {
                    id: uuid::Uuid::new_v4(),
                    name: "ปกติ / Regular".into(),
                    price: pos_domain::Money::ZERO,
                },
                pos_domain::OptionChoice {
                    id: uuid::Uuid::new_v4(),
                    name: "ใหญ่ / Large".into(),
                    price: "10".parse()?,
                },
            ],
        };
        pos.save_option_set(&size, real_now - 900)?;
        let drinks: Vec<uuid::Uuid> = pos
            .products("", 0)?
            .into_iter()
            .filter(|p| p.category.contains("Drinks") && p.stock.is_none())
            .map(|p| p.id)
            .collect();
        pos.link_option_set(size.id, &drinks, true, real_now - 800)?;
    }
    // `--many N`: N generated products in ten categories, for checking a
    // large catalog (Sell loads 500 at a time).
    if let Some(n) = std::env::args()
        .skip_while(|a| a != "--many")
        .nth(1)
        .and_then(|n| n.parse::<u32>().ok())
    {
        for i in 0..n {
            pos.save_product(pos_domain::ProductInput {
                sku: format!("M-{i:05}"),
                name: format!("สินค้าตัวอย่าง {i} / Sample {i}"),
                category: format!("หมวด {} / Group {}", i % 10, i % 10),
                price: pos_domain::Money::from_satang(1000 + i64::from(i % 50) * 100)?,
                ..Default::default()
            })?;
        }
    }
    // `--promos`: a buy-2-get-1 tea, 10% off drinks 14:00–17:00, spend 500
    // save 50, and a WELCOME coupon (10%, 100 bills).
    if std::env::args().any(|a| a == "--promos") {
        use pos_domain::{Discount, PromoRule, PromoTarget, Promotion};
        let real_now = time::OffsetDateTime::now_utc().unix_timestamp();
        let products = pos.products("", 0)?;
        if let Some(tea) = products.iter().find(|p| p.name.contains("Thai iced tea")) {
            pos.save_promotion(
                &Promotion::new(
                    "ชาไทย 2 แถม 1",
                    PromoRule::BuyGet { buy: 2, free: 1 },
                    PromoTarget::Product(tea.id),
                ),
                real_now - 600,
            )?;
            let mut afternoon = Promotion::new(
                "บ่ายนี้เครื่องดื่มลด 10%",
                PromoRule::Off {
                    discount: Discount::Percent(1000),
                },
                PromoTarget::Category(tea.category.clone()),
            );
            afternoon.from_minute = 14 * 60;
            afternoon.to_minute = 17 * 60;
            pos.save_promotion(&afternoon, real_now - 500)?;
        }
        pos.save_promotion(
            &Promotion::new(
                "ครบ 500 ลด 50",
                PromoRule::Bill {
                    min_spend: "500".parse()?,
                    discount: Discount::Amount("50".parse()?),
                },
                PromoTarget::All,
            ),
            real_now - 400,
        )?;
        let mut welcome = Promotion::new(
            "คูปองต้อนรับ ลด 10%",
            PromoRule::Off {
                discount: Discount::Percent(1000),
            },
            PromoTarget::All,
        );
        welcome.coupon = "WELCOME".into();
        welcome.max_uses = Some(100);
        pos.save_promotion(&welcome, real_now - 300)?;
        // One bill over 500 so the report shows the spend-over promotion.
        if let (Some(shift), Some(big)) = (
            pos.active_shift()?,
            pos.products("", 0)?
                .into_iter()
                .find(|p| p.price.satang() >= 50_000 && p.stock.is_none_or(|n| n > 0) && p.on_sale),
        ) {
            let mut cart = pos_domain::Cart::new();
            cart.lines.push(pos_domain::CartLine {
                product_id: big.id,
                quantity: 1,
                ..Default::default()
            });
            pos.checkout_for_shift(&cart, "5000".parse()?, Some(shift.id), real_now - 200)?;
        }
    }
    std::fs::write(
        dir.join("shop.json"),
        r#"{"version":1,"name":"ร้านหัวใจ","kind":"mixed","start_in_sell":false,"setup_complete":true,"promptpay":"0812345678"}"#,
    )?;
    Ok(())
}
