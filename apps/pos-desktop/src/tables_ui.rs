//! Tables: the floor shown inside Sell (a pane beside the bill), and the
//! table list in Settings. A table's bill is an ordinary held bill tagged
//! with the table; tapping a table loads it into the bill panel.
use crate::{
    app::App,
    ui::{Icon, Theme, format, space, tokens::radius, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, Align2, Sense, Stroke, StrokeKind, pos2, vec2};
use pos_domain::{Cart, DiningTable};
use uuid::Uuid;

#[derive(Default)]
pub struct TablesState {
    pub editor: Option<DiningTable>,
    seats: String,
}

/// The open bill on each table.
fn bills(app: &App) -> Vec<(Uuid, Cart)> {
    let mut out: Vec<(Uuid, Cart)> = app
        .data
        .held
        .iter()
        .filter_map(|c| c.table.map(|t| (t, c.clone())))
        .collect();
    // The bill being edited is newer than its held copy.
    if let Some(t) = app.cart.table {
        out.retain(|(_, c)| c.id != app.cart.id);
        if !app.cart.lines.is_empty() {
            out.push((t, app.cart.clone()));
        }
    }
    out
}
/// Units still to send to the kitchen (positive changes only).
pub fn unsent(app: &App, cart: &Cart) -> i64 {
    let kitchen = |id: Uuid| app.known.get(&id).is_some_and(|p| p.kitchen);
    pos_domain::kitchen_changes(cart, kitchen)
        .0
        .iter()
        .map(|(_, d)| *d)
        .filter(|d| *d > 0)
        .sum()
}

/// Active tables, in settings order.
pub fn active(app: &App) -> Vec<DiningTable> {
    app.data
        .tables
        .iter()
        .filter(|t| t.active)
        .cloned()
        .collect()
}
/// Tables with an open bill (shown on the Sell tab).
pub fn busy_count(app: &App) -> usize {
    let bills = bills(app);
    active(app)
        .iter()
        .filter(|t| bills.iter().any(|(id, _)| *id == t.id))
        .count()
}

/// Opens a table in the bill panel: its bill if it has one, otherwise a new
/// bill for it. The previous bill stays held (it is autosaved).
fn open_table(app: &mut App, table: &DiningTable, bill: Option<&Cart>) {
    app.sell.pane = crate::sale_ui::Pane::Products;
    app.sell.tab = crate::sale_ui::CartTab::Current;
    match bill {
        Some(cart) if cart.id == app.cart.id => {}
        Some(cart) => app.resume(cart.id),
        None => {
            if !app.cart.lines.is_empty() || app.cart.table.is_some() {
                app.park_cart();
            }
            app.last_sale = None;
            app.cart.table = Some(table.id);
            app.cart.label = table.name.clone();
        }
    }
    app.sell.focus_search = true;
}
/// Moves the open bill to a free table (or seats a counter bill there).
fn move_bill(app: &mut App, table: &DiningTable) {
    let old = app
        .cart
        .table
        .and_then(|id| app.data.tables.iter().find(|t| t.id == id))
        .map(|t| t.name.clone());
    // A label that was just the old table's name follows the bill.
    if app.cart.label.is_empty() || old.as_deref() == Some(app.cart.label.as_str()) {
        app.cart.label = table.name.clone();
    }
    app.cart.table = Some(table.id);
    app.sell.moving = false;
    app.sell.pane = crate::sale_ui::Pane::Products;
    if !app.cart.lines.is_empty() {
        app.autosave_now();
    }
    app.toast(format!(
        "{} {}",
        app.lang.text("ย้ายบิลไปโต๊ะ", "Bill moved to"),
        table.name
    ));
}
/// A counter / takeaway bill: keeps an open counter bill, parks a table's.
fn counter_bill(app: &mut App) {
    if app.cart.table.is_some() {
        app.park_cart();
    }
    app.sell.moving = false;
    app.sell.pane = crate::sale_ui::Pane::Products;
    app.sell.tab = crate::sale_ui::CartTab::Current;
    app.sell.focus_search = true;
}

/// The floor inside Sell: zones of table cards. Tapping a table loads its
/// bill on the left; in move mode, tapping a free table moves the bill.
pub fn floor(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let tables = active(app);
    let moving = app.sell.moving;
    ui.spacing_mut().item_spacing.y = space::S2;
    if moving {
        let name = if app.cart.label.is_empty() {
            lang.text("บิลนี้", "this bill").to_owned()
        } else {
            app.cart.label.clone()
        };
        if w::banner(
            ui,
            w::Tone::Info,
            &format!(
                "{} {name}",
                lang.text("แตะโต๊ะว่างเพื่อย้าย", "Tap a free table to move")
            ),
            Some(lang.text("ยกเลิก", "Cancel")),
        ) {
            app.sell.moving = false;
        }
    } else {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::S2;
            if w::button(lang.text("ขายหน้าร้าน / กลับบ้าน", "Counter / takeaway"))
                .icon(Icon::Plus)
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                counter_bill(app);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let open = app
                    .data
                    .kitchen
                    .iter()
                    .filter(|k| k.bumped_at.is_none())
                    .count();
                if w::button(format!(
                    "{} · {open}",
                    lang.text("เปิดจอครัว", "Open kitchen screen")
                ))
                .ghost()
                .show(ui)
                .clicked()
                {
                    app.kitchen_open = true;
                }
            });
        });
    }
    let bills = bills(app);
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut zones: Vec<String> = Vec::new();
    for table in &tables {
        if !zones.contains(&table.zone) {
            zones.push(table.zone.clone());
        }
    }
    // Elapsed times tick by the minute.
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(30));
    let mut open: Option<(DiningTable, Option<Cart>)> = None;
    let out = egui::ScrollArea::vertical()
        .id_salt("sell-floor")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for zone in &zones {
                ui.add_space(space::S1);
                ui.label(t.text(
                    if zone.is_empty() {
                        lang.text("ไม่ระบุโซน", "No zone")
                    } else {
                        zone.as_str()
                    },
                    type_scale::BODY_STRONG,
                ));
                // Cards share the row evenly, about 160–200 points each.
                let width = ui.available_width();
                let gap = space::S2;
                let min_w = if t.touch() { 176.0 } else { 152.0 };
                let cols = (((width + gap) / (min_w + gap)).floor() as usize).max(1);
                let card_w = ((width - gap * (cols - 1) as f32) / cols as f32).floor();
                let card_h = if t.touch() { 128.0 } else { 108.0 };
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(gap, gap);
                    for table in tables.iter().filter(|x| &x.zone == zone) {
                        let bill = bills.iter().find(|(id, _)| *id == table.id).map(|(_, c)| c);
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(card_w, card_h), Sense::click());
                        let busy = bill.is_some();
                        let current = app.cart.table == Some(table.id);
                        // Move mode: only free tables are targets.
                        let enabled = !app.busy && (!moving || (!busy && !current));
                        let waiting = bill.map_or(0, |c| unsent(app, c));
                        // design v0.2: free = dashed outline; seated = paper
                        // card with a status stripe and a status badge.
                        let r = t.radius(radius::LG);
                        let faded = moving && !enabled;
                        let fg = if faded { t.p.fg_subtle } else { t.p.fg_default };
                        let pad = if t.touch() { space::S3 } else { 10.0 };
                        let kitchen_open = bill.is_some_and(|c| {
                            app.data
                                .kitchen
                                .iter()
                                .any(|k| k.cart_id == c.id && k.bumped_at.is_none())
                        });
                        if busy || current {
                            let bg = if resp.hovered() && enabled {
                                t.p.bg_hover
                            } else {
                                t.p.bg_raised
                            };
                            let edge = if current {
                                Stroke::new(2.0, t.p.border_selected)
                            } else {
                                Stroke::new(1.0, t.p.border_subtle)
                            };
                            ui.painter().rect_filled(rect, r, bg);
                            ui.painter().rect_stroke(rect, r, edge, StrokeKind::Inside);
                            let stripe = if waiting > 0 {
                                t.p.warning_solid
                            } else {
                                t.p.info_solid
                            };
                            let inner = rect.shrink(edge.width);
                            let ir = r.saturating_sub(edge.width as u8);
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(inner.min, vec2(3.0, inner.height())),
                                egui::CornerRadius { nw: ir, sw: ir, ne: 0, se: 0 },
                                stripe,
                            );
                        } else {
                            if resp.hovered() && enabled {
                                ui.painter().rect_filled(rect, r, t.p.bg_hover);
                            }
                            dashed_rect(ui, rect.shrink(0.5), t.p.border_control);
                        }
                        ui.painter().text(
                            rect.min + vec2(pad + 2.0, pad),
                            Align2::LEFT_TOP,
                            &table.name,
                            t.strong_font(type_scale::TITLE),
                            fg,
                        );
                        let guests = bill.map_or(0, |c| c.guests);
                        let seats = if table.seats > 0 {
                            format!("{guests}/{} {}", table.seats, lang.text("ที่", "seats"))
                        } else {
                            String::new()
                        };
                        ui.painter().text(
                            pos2(rect.right() - pad, rect.top() + pad + 2.0),
                            Align2::RIGHT_TOP,
                            seats,
                            t.font(type_scale::LABEL),
                            t.p.fg_muted,
                        );
                        match bill {
                            Some(cart) => {
                                let total = app
                                    .price_cart(cart)
                                    .map_or("—".into(), |p| format::money(p.total));
                                let since = app
                                    .data
                                    .kitchen
                                    .iter()
                                    .filter(|k| k.cart_id == cart.id)
                                    .map(|k| k.created_at)
                                    .min();
                                let badge_h = if t.touch() { 22.0 } else { 18.0 };
                                let line_y = rect.bottom() - pad - badge_h - space::S1;
                                let minutes = since.map_or(String::new(), |at| {
                                    format!("{} {}", (now - at).max(0) / 60, lang.text("นาที", "min"))
                                });
                                ui.painter().text(
                                    pos2(rect.left() + pad + 2.0, line_y),
                                    Align2::LEFT_BOTTOM,
                                    if current {
                                        format!("{} {minutes}", lang.text("เปิดอยู่ ·", "Open ·"))
                                    } else {
                                        minutes
                                    },
                                    t.font(type_scale::CAPTION),
                                    t.p.fg_muted,
                                );
                                ui.painter().text(
                                    pos2(rect.right() - pad, line_y),
                                    Align2::RIGHT_BOTTOM,
                                    total,
                                    t.strong_font(type_scale::MONEY),
                                    fg,
                                );
                                let badge = if waiting > 0 {
                                    Some((
                                        format!("{} {waiting}", lang.text("ยังไม่ส่งครัว", "Not sent")),
                                        t.p.warning_bg,
                                        t.p.warning_fg,
                                    ))
                                } else if kitchen_open {
                                    Some((
                                        lang.text("กำลังทำ", "In kitchen").to_owned(),
                                        t.p.info_bg,
                                        t.p.info_fg,
                                    ))
                                } else {
                                    None
                                };
                                if let Some((text, bg, color)) = badge {
                                    let b = egui::Rect::from_min_max(
                                        pos2(rect.left() + pad + 2.0, rect.bottom() - pad - badge_h),
                                        pos2(rect.right() - pad, rect.bottom() - pad),
                                    );
                                    ui.painter().rect_filled(b, t.radius(radius::SM), bg);
                                    ui.painter().text(
                                        pos2(b.left() + space::S2, b.center().y),
                                        Align2::LEFT_CENTER,
                                        text,
                                        t.strong_font(type_scale::CAPTION),
                                        color,
                                    );
                                }
                            }
                            None => {
                                ui.painter().text(
                                    pos2(rect.left() + pad + 2.0, rect.bottom() - pad),
                                    Align2::LEFT_BOTTOM,
                                    if current {
                                        lang.text("เปิดอยู่ · ยังไม่สั่ง", "Open · no items")
                                    } else {
                                        lang.text("ว่าง", "Free")
                                    },
                                    t.font(type_scale::BODY),
                                    t.p.fg_subtle,
                                );
                            }
                        }
                        let name = table.name.clone();
                        resp.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name)
                        });
                        if resp.clicked() && enabled {
                            open = Some((table.clone(), bill.cloned()));
                        }
                    }
                });
            }
            ui.add_space(space::S2);
            w::help_text(
                ui,
                if moving {
                    lang.text(
                        "ย้ายได้เฉพาะโต๊ะว่าง ครัวยังเห็นรายการเดิมตามเลขใบครัว",
                        "Only free tables can take the bill. Kitchen tickets keep their numbers.",
                    )
                } else {
                    lang.text(
                        "แตะโต๊ะเพื่อเปิดบิลทางซ้าย · ส่งครัวหรือรับเงินแล้วจะกลับมาที่ผังโต๊ะ",
                        "Tap a table to open its bill on the left · after sending to the kitchen or payment you return here.",
                    )
                },
            );
        });
    w::scroll_fade(ui, &out, t.p.bg_surface);
    if let Some((table, bill)) = open {
        if moving {
            move_bill(app, &table);
        } else {
            open_table(app, &table, bill.as_ref());
        }
    }
}

/// Dashed outline of a free table.
fn dashed_rect(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    let pts = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    ui.painter().extend(egui::Shape::dashed_line(
        &pts,
        Stroke::new(1.0, color),
        5.0,
        4.0,
    ));
}

/// Settings → Tables & zones.
pub fn settings_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("โต๊ะและโซน", "Tables & zones"));
    w::help_text(
        ui,
        lang.text(
            "สำหรับร้านอาหารนั่งทาน เมื่อมีโต๊ะ จะมีหน้า \"โต๊ะ\" (Alt 9) สำหรับเปิดบิลตามโต๊ะ และส่งออเดอร์เข้าครัว",
            "For sit-down service. With tables, the Tables workspace (Alt 9) opens bills by table and sends orders to the kitchen.",
        ),
    );
    let may = crate::staff_ui::may(app, pos_domain::Permission::Settings);
    let tables = app.data.tables.clone();
    let mut zone = None;
    for table in &tables {
        if zone.as_ref() != Some(&table.zone) {
            zone = Some(table.zone.clone());
            ui.label(t.muted(
                if table.zone.is_empty() {
                    lang.text("ไม่ระบุโซน", "No zone")
                } else {
                    table.zone.as_str()
                },
                type_scale::LABEL,
            ));
        }
        ui.horizontal(|ui| {
            ui.label(t.text(&table.name, type_scale::BODY_STRONG));
            if table.seats > 0 {
                ui.label(t.muted(
                    format!("{} {}", table.seats, lang.text("ที่นั่ง", "seats")),
                    type_scale::CAPTION,
                ));
            }
            if !table.active {
                w::badge(ui, lang.text("ปิดใช้งาน", "Off"), w::Tone::Neutral);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if may
                    && w::button(lang.text("แก้ไข", "Edit"))
                        .ghost()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                {
                    app.tables.seats = table.seats.to_string();
                    app.tables.editor = Some(table.clone());
                }
            });
        });
    }
    if may
        && w::button(lang.text("เพิ่มโต๊ะ", "Add table"))
            .icon(Icon::Plus)
            .enabled(app.ready && !app.busy)
            .show(ui)
            .clicked()
    {
        let last = tables.last();
        // Continue the numbering in the same zone: T3 → T4.
        let next = last
            .and_then(|l| {
                let digits: String = l
                    .name
                    .chars()
                    .rev()
                    .take_while(char::is_ascii_digit)
                    .collect();
                let n: u32 = digits.chars().rev().collect::<String>().parse().ok()?;
                let prefix = l.name.trim_end_matches(|c: char| c.is_ascii_digit());
                Some(format!("{prefix}{}", n + 1))
            })
            .unwrap_or_else(|| "1".into());
        app.tables.seats = last.map_or(4, |l| l.seats).to_string();
        app.tables.editor = Some(DiningTable {
            id: Uuid::new_v4(),
            zone: last.map(|l| l.zone.clone()).unwrap_or_default(),
            name: next,
            seats: last.map_or(4, |l| l.seats),
            active: true,
        });
    }
}

pub fn editor_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut table) = app.tables.editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let seats = app
        .tables
        .seats
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|s| *s <= 99);
    let exists = app.data.tables.iter().any(|x| x.id == table.id);
    let response = w::dialog(ctx, "table-editor", 420.0, |ui| {
        w::dialog_title(
            ui,
            if exists {
                lang.text("แก้ไขโต๊ะ", "Edit table")
            } else {
                lang.text("เพิ่มโต๊ะ", "Add table")
            },
            false,
        );
        w::field_label(ui, lang.text("ชื่อโต๊ะ", "Table name"), None);
        w::text_input(
            ui,
            &mut table.name,
            w::FieldOpts {
                hint: "T1",
                limit: 20,
                ..Default::default()
            },
        );
        w::field_label(
            ui,
            lang.text("โซน", "Zone"),
            Some(lang.text("ไม่บังคับ", "optional")),
        );
        w::text_input(
            ui,
            &mut table.zone,
            w::FieldOpts {
                hint: lang.text("เช่น ในร้าน / ระเบียง", "e.g. Inside / Terrace"),
                limit: 40,
                ..Default::default()
            },
        );
        let zones: Vec<String> = app
            .data
            .tables
            .iter()
            .map(|x| x.zone.clone())
            .filter(|z| !z.is_empty())
            .fold(Vec::new(), |mut v, z| {
                if !v.contains(&z) {
                    v.push(z);
                }
                v
            });
        if !zones.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for z in zones {
                    if w::chip(ui, &z, table.zone == z, false).clicked() {
                        table.zone = z;
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label(t.text(lang.text("จำนวนที่นั่ง", "Seats"), type_scale::BODY));
            w::text_input(
                ui,
                &mut app.tables.seats,
                w::FieldOpts {
                    hint: "4",
                    limit: 2,
                    invalid: seats.is_none(),
                    width: Some(70.0),
                    ..Default::default()
                },
            );
        });
        if exists {
            ui.horizontal(|ui| {
                ui.label(t.text(lang.text("ใช้งานอยู่", "Active"), type_scale::BODY));
                w::switch(ui, &mut table.active, lang.text("ใช้งานอยู่", "Active"));
            });
        }
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let valid = seats.is_some() && !table.name.trim().is_empty();
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
            table.seats = seats.unwrap_or(0);
            let command = Command::SaveTable(table.clone());
            app.tables.editor = Some(table);
            app.send(command);
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.tables.editor = Some(table),
    }
}
