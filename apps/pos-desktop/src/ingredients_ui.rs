//! Ingredients (Products → Ingredients): the list with stock, warnings and
//! cost, the add/edit dialog, receive / remove / count, and each one's
//! history. Recipes are edited on the product (catalog_ui).
use crate::{
    app::App,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align};
use pos_domain::{Ingredient, IngredientKind, IngredientMovement, IngredientRequest, Permission};
use uuid::Uuid;

#[derive(Default)]
pub struct IngredientsState {
    /// The Products workspace shows ingredients instead of products.
    pub open: bool,
    pub editor: Option<Editor>,
    pub adjust: Option<Adjust>,
    pub history: Option<(Uuid, Vec<IngredientMovement>)>,
}
pub struct Editor {
    ingredient: Ingredient,
    exists: bool,
    stock: String,
    low: String,
    cost: String,
}
pub struct Adjust {
    id: Uuid,
    ingredient: Ingredient,
    kind: IngredientKind,
    quantity: String,
    reason: String,
}
fn kind_label(lang: crate::i18n::Language, kind: IngredientKind) -> &'static str {
    match kind {
        IngredientKind::Initial => lang.text("เริ่มต้น", "Start"),
        IngredientKind::Receive => lang.text("รับเข้า", "Received"),
        IngredientKind::Remove => lang.text("นำออก", "Removed"),
        IngredientKind::Count => lang.text("นับสต็อก", "Counted"),
        IngredientKind::Sale => lang.text("ขาย", "Sold"),
        IngredientKind::Void => lang.text("ยกเลิกบิล", "Voided"),
    }
}
/// "1,250.5 กรัม"
pub fn amount(i: &Ingredient, milli: i64) -> String {
    format!("{} {}", format::qty(milli), i.unit)
}
pub fn low_count(app: &App) -> usize {
    app.data
        .ingredients
        .iter()
        .filter(|i| i.is_low() || (i.active && i.stock < 0))
        .count()
}

pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let may = crate::staff_ui::may(app, Permission::Stock);
    ui.spacing_mut().item_spacing.y = space::S2;
    ui.horizontal(|ui| {
        ui.label(t.text(lang.text("วัตถุดิบ", "Ingredients"), type_scale::TITLE_LG));
        ui.label(t.muted(
            format::count(app.data.ingredients.len() as u64),
            type_scale::BODY,
        ));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if may
                && w::button(lang.text("เพิ่มวัตถุดิบ", "Add ingredient"))
                    .icon(Icon::Plus)
                    .primary()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
            {
                app.error = None;
                app.ingredients.editor = Some(Editor {
                    ingredient: Ingredient {
                        id: Uuid::new_v4(),
                        name: String::new(),
                        unit: lang.text("กรัม", "g").into(),
                        stock: 0,
                        low: 0,
                        cost: None,
                        active: true,
                    },
                    exists: false,
                    stock: String::new(),
                    low: String::new(),
                    cost: String::new(),
                });
            }
        });
    });
    w::help_text(
        ui,
        lang.text(
            "ใส่สูตรในหน้าแก้ไขสินค้า ขายแล้วระบบตัดวัตถุดิบตามสูตรให้อัตโนมัติ สต็อกติดลบได้ (ไม่ห้ามขาย) ยกเลิกทั้งบิลคืนวัตถุดิบให้ คืนบางรายการไม่คืน เพราะอาหารทำไปแล้ว",
            "Set recipes on each product; sales deduct their ingredients automatically. Stock may go below zero (sales are never refused). Voiding a whole bill gives ingredients back; partial refunds do not, as the food was made.",
        ),
    );
    if app.data.ingredients.is_empty() {
        w::state_view(
            ui,
            Icon::Products,
            lang.text("ยังไม่มีวัตถุดิบ", "No ingredients yet"),
            lang.text(
                "เพิ่มวัตถุดิบ เช่น ข้าวสาร (กรัม) ไข่ไก่ (ฟอง) แล้วใส่สูตรให้สินค้า",
                "Add ingredients such as rice (g) or eggs (pcs), then give products a recipe.",
            ),
        );
        return;
    }
    let list = app.data.ingredients.clone();
    let out = egui::ScrollArea::vertical()
        .id_salt("ingredients")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for i in &list {
                ui.horizontal(|ui| {
                    let name_w = (ui.available_width() * 0.4).max(140.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(name_w, 0.0),
                        egui::Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(name_w);
                            ui.horizontal(|ui| {
                                w::name_label(
                                    ui,
                                    &i.name,
                                    type_scale::BODY_STRONG,
                                    if i.active {
                                        t.p.fg_default
                                    } else {
                                        t.p.fg_subtle
                                    },
                                );
                                if !i.active {
                                    w::badge(ui, lang.text("ปิดใช้", "Off"), w::Tone::Neutral);
                                } else if i.stock < 0 {
                                    w::badge(ui, lang.text("ติดลบ", "Below zero"), w::Tone::Danger);
                                } else if i.is_low() {
                                    w::badge(ui, lang.text("ใกล้หมด", "Low"), w::Tone::Warning);
                                }
                            });
                            let mut sub = Vec::new();
                            if let Some(c) = i.cost {
                                sub.push(format!("{} / {}", format::money(c), i.unit));
                            }
                            if i.low > 0 {
                                sub.push(format!(
                                    "{} {}",
                                    lang.text("เตือนที่", "warn at"),
                                    amount(i, i.low)
                                ));
                            }
                            if !sub.is_empty() {
                                ui.label(t.muted(sub.join(" · "), type_scale::CAPTION));
                            }
                        },
                    );
                    ui.label(
                        egui::RichText::new(amount(i, i.stock))
                            .font(t.strong_font(type_scale::MONEY))
                            .color(if i.stock < 0 {
                                t.p.danger_fg
                            } else {
                                t.p.fg_default
                            }),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = space::S1;
                        if may
                            && w::button(lang.text("แก้ไข", "Edit"))
                                .ghost()
                                .size(w::Size::Sm)
                                .show(ui)
                                .clicked()
                        {
                            app.error = None;
                            app.ingredients.editor = Some(Editor {
                                ingredient: i.clone(),
                                exists: true,
                                stock: String::new(),
                                low: if i.low > 0 {
                                    format::qty(i.low)
                                } else {
                                    String::new()
                                },
                                cost: i.cost.map(format::money).unwrap_or_default(),
                            });
                        }
                        let showing = app
                            .ingredients
                            .history
                            .as_ref()
                            .is_some_and(|(id, _)| *id == i.id);
                        if w::button(if showing {
                            lang.text("ซ่อนประวัติ", "Hide history")
                        } else {
                            lang.text("ประวัติ", "History")
                        })
                        .ghost()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                        {
                            if showing {
                                app.ingredients.history = None;
                            } else {
                                app.ingredients.history = Some((i.id, Vec::new()));
                                app.send(Command::IngredientHistory(i.id));
                            }
                        }
                        if may && i.active {
                            for kind in [
                                IngredientKind::Count,
                                IngredientKind::Remove,
                                IngredientKind::Receive,
                            ] {
                                let label = match kind {
                                    IngredientKind::Receive => lang.text("รับเข้า", "Receive"),
                                    IngredientKind::Remove => lang.text("นำออก", "Remove"),
                                    _ => lang.text("นับ", "Count"),
                                };
                                if w::button(label)
                                    .size(w::Size::Sm)
                                    .enabled(!app.busy)
                                    .show(ui)
                                    .clicked()
                                {
                                    app.error = None;
                                    app.ingredients.adjust = Some(Adjust {
                                        id: Uuid::new_v4(),
                                        ingredient: i.clone(),
                                        kind,
                                        quantity: String::new(),
                                        reason: String::new(),
                                    });
                                }
                            }
                        }
                    });
                });
                if let Some((_, moves)) = app
                    .ingredients
                    .history
                    .as_ref()
                    .filter(|(id, _)| *id == i.id)
                {
                    history(ui, &t, lang, i, moves);
                }
                w::divider(ui);
            }
        });
    w::scroll_fade(ui, &out, t.p.bg_surface);
}
fn history(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: crate::i18n::Language,
    i: &Ingredient,
    moves: &[IngredientMovement],
) {
    egui::Frame::new()
        .fill(t.p.bg_sunken)
        .corner_radius(t.control_radius())
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if moves.is_empty() {
                ui.label(t.muted(
                    lang.text("ยังไม่มีประวัติ", "No history yet"),
                    type_scale::CAPTION,
                ));
            }
            for m in moves.iter().take(20) {
                ui.horizontal(|ui| {
                    ui.label(t.muted(format::date_time(lang, m.created_at), type_scale::CAPTION));
                    ui.label(t.text(kind_label(lang, m.kind), type_scale::LABEL));
                    let delta = m.after - m.before;
                    let sign = if delta > 0 { "+" } else { "" };
                    ui.label(t.text(
                        format!("{sign}{}", format::qty(delta)),
                        type_scale::MONEY_SM,
                    ));
                    ui.label(t.muted(format!("→ {}", amount(i, m.after)), type_scale::CAPTION));
                    if !m.reason.is_empty() {
                        ui.label(t.muted(&m.reason, type_scale::CAPTION));
                    }
                    if let Some(by) = &m.staff.by {
                        ui.label(t.muted(&by.name, type_scale::CAPTION));
                    }
                });
            }
        });
}

pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    editor_dialog(app, ctx);
    adjust_dialog(app, ctx);
}
fn editor_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut e) = app.ingredients.editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let stock = if e.exists || e.stock.trim().is_empty() {
        Some(0)
    } else {
        format::parse_qty(&e.stock)
    };
    let low = if e.low.trim().is_empty() {
        Some(0)
    } else {
        format::parse_qty(&e.low)
    };
    let cost = if e.cost.trim().is_empty() {
        Some(None)
    } else {
        format::parse_money(&e.cost).ok().map(Some)
    };
    let response = w::dialog(ctx, "ingredient-editor", 460.0, |ui| {
        w::dialog_title(
            ui,
            if e.exists {
                lang.text("แก้ไขวัตถุดิบ", "Edit ingredient")
            } else {
                lang.text("เพิ่มวัตถุดิบ", "Add ingredient")
            },
            false,
        );
        w::field_label(ui, lang.text("ชื่อ", "Name"), None);
        w::text_input(
            ui,
            &mut e.ingredient.name,
            w::FieldOpts {
                hint: lang.text("เช่น ไข่ไก่", "e.g. Eggs"),
                limit: 80,
                ..Default::default()
            },
        );
        w::field_label(ui, lang.text("หน่วย", "Unit"), None);
        ui.horizontal_wrapped(|ui| {
            for unit in [
                lang.text("กรัม", "g"),
                lang.text("กิโลกรัม", "kg"),
                "ml",
                lang.text("ลิตร", "L"),
                lang.text("ฟอง", "egg"),
                lang.text("ชิ้น", "pcs"),
            ] {
                if w::chip(ui, unit, e.ingredient.unit == unit, false).clicked() {
                    e.ingredient.unit = unit.into();
                }
            }
        });
        w::text_input(
            ui,
            &mut e.ingredient.unit,
            w::FieldOpts {
                hint: lang.text("หรือพิมพ์หน่วยเอง", "or type a unit"),
                limit: 20,
                ..Default::default()
            },
        );
        if !e.exists {
            ui.horizontal(|ui| {
                ui.label(t.text(lang.text("สต็อกตอนนี้", "Stock now"), type_scale::BODY));
                w::text_input(
                    ui,
                    &mut e.stock,
                    w::FieldOpts {
                        hint: "0",
                        limit: 12,
                        width: Some(120.0),
                        invalid: stock.is_none(),
                        ..Default::default()
                    },
                );
                ui.label(t.muted(&e.ingredient.unit, type_scale::BODY));
            });
        }
        ui.horizontal(|ui| {
            ui.label(t.text(lang.text("เตือนเมื่อเหลือ", "Warn at"), type_scale::BODY));
            w::text_input(
                ui,
                &mut e.low,
                w::FieldOpts {
                    hint: lang.text("ไม่เตือน", "off"),
                    limit: 12,
                    width: Some(120.0),
                    invalid: low.is_none(),
                    ..Default::default()
                },
            );
            ui.label(t.muted(&e.ingredient.unit, type_scale::BODY));
        });
        ui.horizontal(|ui| {
            ui.label(t.text(
                lang.text("ต้นทุนต่อหน่วย (บาท)", "Cost per unit (฿)"),
                type_scale::BODY,
            ));
            w::text_input(
                ui,
                &mut e.cost,
                w::FieldOpts {
                    hint: lang.text("ไม่บังคับ", "optional"),
                    limit: 12,
                    width: Some(120.0),
                    invalid: cost.is_none(),
                    ..Default::default()
                },
            );
            ui.label(t.muted(format!("/ {}", e.ingredient.unit), type_scale::BODY));
        });
        w::help_text(
            ui,
            lang.text(
                "ทศนิยมได้ 3 ตำแหน่ง เช่น 0.25 กิโลกรัม ต้นทุนใช้คำนวณต้นทุนตามสูตรในหน้าสินค้า",
                "Up to three decimals, e.g. 0.25 kg. Cost feeds the recipe cost on products.",
            ),
        );
        if e.exists {
            ui.horizontal(|ui| {
                w::switch(ui, &mut e.ingredient.active, lang.text("ใช้งานอยู่", "Active"));
                ui.label(t.text(
                    lang.text(
                        "ใช้งานอยู่ (ปิดแล้วใส่ในสูตรใหม่ไม่ได้)",
                        "Active (switched off: not for new recipes)",
                    ),
                    type_scale::BODY,
                ));
            });
        }
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        let valid = stock.is_some()
            && low.is_some()
            && cost.is_some()
            && !e.ingredient.name.trim().is_empty()
            && !e.ingredient.unit.trim().is_empty();
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
                .enabled(valid && !app.busy)
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out
    });
    match response.inner {
        Some(true) => {
            let mut saved = e.ingredient.clone();
            saved.stock = stock.unwrap_or(0);
            saved.low = low.unwrap_or(0);
            saved.cost = cost.flatten();
            app.send(Command::SaveIngredient(saved));
            app.ingredients.editor = Some(e);
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.ingredients.editor = Some(e),
    }
}
fn adjust_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut a) = app.ingredients.adjust.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let quantity = format::parse_qty(&a.quantity);
    let request = IngredientRequest {
        id: a.id,
        ingredient_id: a.ingredient.id,
        kind: a.kind,
        quantity: quantity.unwrap_or(-1),
        reason: a.reason.clone(),
    };
    let after = request
        .validate()
        .ok()
        .and_then(|_| request.after(a.ingredient.stock).ok());
    let response = w::dialog(ctx, "ingredient-adjust", 440.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} · {}",
                match a.kind {
                    IngredientKind::Receive => lang.text("รับวัตถุดิบเข้า", "Receive"),
                    IngredientKind::Remove =>
                        lang.text("นำออก (เสีย/ทิ้ง/ใช้เอง)", "Remove (waste, spoiled, staff use)"),
                    _ => lang.text("นับสต็อกจริง", "Count stock"),
                },
                a.ingredient.name
            ),
            false,
        );
        w::kv(
            ui,
            lang.text("ตอนนี้", "Now"),
            &amount(&a.ingredient, a.ingredient.stock),
            w::Value::Plain,
        );
        ui.horizontal(|ui| {
            ui.label(t.text(
                if a.kind == IngredientKind::Count {
                    lang.text("นับได้", "Counted")
                } else {
                    lang.text("จำนวน", "Quantity")
                },
                type_scale::BODY,
            ));
            let bad = !a.quantity.is_empty() && quantity.is_none();
            let r = w::text_input(
                ui,
                &mut a.quantity,
                w::FieldOpts {
                    hint: "0",
                    limit: 12,
                    width: Some(140.0),
                    invalid: bad,
                    ..Default::default()
                },
            );
            if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            ui.label(t.muted(&a.ingredient.unit, type_scale::BODY));
        });
        if a.kind == IngredientKind::Remove {
            ui.horizontal_wrapped(|ui| {
                for reason in [
                    lang.text("เสีย/หมดอายุ", "Spoiled"),
                    lang.text("ทำหก/ทำพลาด", "Spilled / mistake"),
                    lang.text("พนักงานทาน", "Staff meal"),
                ] {
                    if w::chip(ui, reason, a.reason == reason, false).clicked() {
                        a.reason = reason.into();
                    }
                }
            });
        }
        w::text_input(
            ui,
            &mut a.reason,
            w::FieldOpts {
                hint: if a.kind == IngredientKind::Remove {
                    lang.text("เหตุผล (ต้องใส่)", "Reason (required)")
                } else {
                    lang.text("หมายเหตุ (ไม่บังคับ)", "Note (optional)")
                },
                limit: 240,
                ..Default::default()
            },
        );
        if let Some(after) = after {
            w::kv(
                ui,
                lang.text("หลังบันทึก", "After"),
                &amount(&a.ingredient, after),
                w::Value::Strong,
            );
        }
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
                .enabled(after.is_some() && !app.busy)
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out
    });
    match response.inner {
        Some(true) => {
            app.send(Command::AdjustIngredient(request));
            app.ingredients.adjust = Some(a);
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.ingredients.adjust = Some(a),
    }
}
