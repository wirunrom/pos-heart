//! Home (design/docs/05: shift status · today's totals · shortcuts · pending work).
//! Today's totals come from `today_summary`, never the report's selected date,
//! and are labelled as sales — not drawer cash.
use crate::{
    app::{App, Page},
    ui::{Icon, Theme, format, space, type_scale, w},
    worker,
};
use eframe::egui::{self, Align, RichText, vec2};
use std::time::{Duration, Instant, SystemTime};

#[derive(Default)]
pub struct HomeCache {
    checked: Option<Instant>,
    last_backup: Option<SystemTime>,
    drafts: Vec<uuid::Uuid>,
}
fn scan(app: &mut App) {
    let stale = app
        .home
        .checked
        .is_none_or(|t| t.elapsed() > Duration::from_secs(30));
    if !stale {
        return;
    }
    app.home.checked = Some(Instant::now());
    app.home.last_backup = None;
    app.home.drafts.clear();
    if let Ok(entries) = std::fs::read_dir(&app.data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("pos-heart-backup-")
                && (name.ends_with(".sqlite3") || name.ends_with(".posbackup"))
            {
                let modified = entry.metadata().and_then(|m| m.modified()).ok();
                app.home.last_backup = app.home.last_backup.max(modified);
            }
            if let Some(id) = name
                .strip_prefix("drawer-count-")
                .and_then(|n| n.strip_suffix(".json"))
                .and_then(|n| uuid::Uuid::parse_str(n).ok())
            {
                app.home.drafts.push(id);
            }
        }
    }
}

/// Home (design v0.2): shift card + today | to do | held bills and open
/// tables, as three full-height columns on the canvas.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    scan(app);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, egui::Sense::hover());
    let gap = t.pane_gap();
    let narrow = full.width() < 1100.0;
    let w1 = (full.width() * if narrow { 0.4 } else { 0.36 }).floor();
    let w2 = ((full.width() - w1 - gap * 2.0) / 2.0).floor();
    let c1 = egui::Rect::from_min_size(full.min, vec2(w1, full.height()));
    let c2 = egui::Rect::from_min_size(
        egui::pos2(c1.right() + gap, full.top()),
        vec2(w2, full.height()),
    );
    let c3 = egui::Rect::from_min_max(egui::pos2(c2.right() + gap, full.top()), full.max);
    // Column 1: shift card on the chrome, then today's figures.
    let mut col = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(c1)
            .layout(egui::Layout::top_down(Align::Min)),
    );
    let card = shift_card(app, &mut col).rect;
    let today = egui::Rect::from_min_max(egui::pos2(c1.left(), card.bottom() + gap), c1.max);
    let date = format::date(lang, worker::today());
    let (mut head, mut body) = w::pane_uis(ui, today);
    w::pane_title(&mut head, lang.text("วันนี้", "Today"));
    w::pane_count(&mut head, &date);
    egui::Frame::new()
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(&mut body, |ui| today_body(app, ui));
    // Column 2: things to do.
    let tasks = tasks(app);
    let (mut head, mut body) = w::pane_uis(ui, c2);
    w::pane_title(&mut head, lang.text("ต้องทำ", "To do"));
    w::pane_count(&mut head, &tasks.len().to_string());
    egui::ScrollArea::vertical()
        .id_salt("home-tasks")
        .auto_shrink([false, false])
        .show(&mut body, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    if tasks.is_empty() {
                        w::state_view(
                            ui,
                            Icon::CheckCircle,
                            lang.text("ไม่มีงานค้าง", "Nothing pending"),
                            "",
                        );
                    }
                    for task in tasks {
                        if task_card(ui, &task) {
                            run_task(app, task.action);
                        }
                    }
                });
        });
    // Column 3: held bills and open tables.
    let held = app.held_bills();
    let (mut head, mut body) = w::pane_uis(ui, c3);
    w::pane_title(
        &mut head,
        lang.text("บิลที่พักไว้และโต๊ะที่เปิด", "Held bills & open tables"),
    );
    w::pane_count(&mut head, &held.len().to_string());
    egui::ScrollArea::vertical()
        .id_salt("home-held")
        .auto_shrink([false, false])
        .show(&mut body, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    if held.is_empty() {
                        w::state_view(
                            ui,
                            Icon::Pause,
                            lang.text("ไม่มีบิลที่พักไว้", "No held bills"),
                            lang.text(
                                "บิลที่พัก (F4) และโต๊ะที่เปิดอยู่จะแสดงที่นี่",
                                "Held bills (F4) and open tables appear here.",
                            ),
                        );
                    }
                    for cart in &held {
                        if held_card(app, ui, cart) && !app.busy {
                            app.resume(cart.id);
                        }
                    }
                });
        });
    let _ = t;
}
/// Shift status on the chrome: duration, float and the next step.
fn shift_card(app: &mut App, ui: &mut egui::Ui) -> egui::Response {
    let t = Theme::of(ui);
    let lang = app.lang;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    egui::Frame::new()
        .fill(t.p.bg_chrome_hover)
        .corner_radius(t.pane_radius())
        .inner_margin(egui::Margin::same(if t.touch() { 18 } else { 16 }))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = space::S1;
            let big = type_scale::Style {
                size: if t.touch() { 26.0 } else { 22.0 },
                line_height: 30.0,
                weight: 600,
                family: type_scale::Family::Ui,
            };
            match app.data.shift.clone() {
                Some(shift) => {
                    let who = app
                        .data
                        .signed_in
                        .as_ref()
                        .map(|m| format!(" · {}", m.name))
                        .unwrap_or_default();
                    ui.label(t.chrome_muted(
                        format!("{}{who}", lang.text("กะที่เปิดอยู่", "Open shift")),
                        type_scale::CAPTION,
                    ));
                    ui.label(t.chrome(
                        format!(
                            "{} {}",
                            lang.text("เปิดอยู่", "Open for"),
                            format::duration(lang, now - shift.opened_at)
                        ),
                        big,
                    ));
                    ui.label(t.chrome_muted(
                        format!(
                            "{} {} · {} {}",
                            lang.text("เปิด", "Opened"),
                            format::clock(shift.opened_at),
                            lang.text("เงินตั้งต้น", "float"),
                            format::money(shift.opening)
                        ),
                        type_scale::CAPTION,
                    ));
                }
                None => {
                    ui.label(t.chrome_muted(lang.text("กะ", "Shift"), type_scale::CAPTION));
                    ui.label(t.chrome(lang.text("ยังไม่เปิดกะ", "No open shift"), big));
                    ui.label(t.chrome_muted(
                        lang.text(
                            "นับเงินตั้งต้นแล้วเปิดกะ ก่อนรับเงินหรือคืนเงิน",
                            "Count the opening float and open a shift before taking payment.",
                        ),
                        type_scale::CAPTION,
                    ));
                }
            }
            ui.add_space(space::S3);
            let open = app.data.shift.is_some();
            let half = ((ui.available_width() - space::S2) / 2.0).floor();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = space::S2;
                let label = if !open {
                    lang.text("เปิดกะ", "Open shift")
                } else if app.cart.lines.is_empty() {
                    lang.text("เริ่มขาย", "Start selling")
                } else {
                    lang.text("ขายต่อ", "Continue selling")
                };
                let main = ui
                    .allocate_ui_with_layout(
                        vec2(
                            if open { half } else { ui.available_width() },
                            t.m.control_lg,
                        ),
                        egui::Layout::left_to_right(Align::Center),
                        |ui| {
                            w::button(label)
                                .primary()
                                .size(w::Size::Lg)
                                .fill()
                                .shortcut(if open { "Alt 2" } else { "" })
                                .enabled(app.ready && !app.busy)
                                .show(ui)
                        },
                    )
                    .inner;
                if main.clicked() {
                    if open {
                        app.navigate(Page::Sell);
                    } else {
                        crate::shift_ui::goto_open(app);
                    }
                }
                if open
                    && chrome_button(ui, lang.text("ปิดกะ", "Close shift"), t.m.control_lg).clicked()
                {
                    app.drawer.selected = None;
                    app.drawer.tab = crate::shift_ui::Tab::Close;
                    app.navigate(Page::Shift);
                }
            });
        })
        .response
}
/// Secondary button on a chrome surface.
fn chrome_button(ui: &mut egui::Ui, label: &str, height: f32) -> egui::Response {
    let t = Theme::of(ui);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let r = t.control_radius();
    ui.painter().rect(
        rect,
        r,
        if response.hovered() {
            t.p.bg_chrome_raised
        } else {
            t.p.bg_chrome
        },
        egui::Stroke::new(1.0, t.p.border_chrome),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        t.strong_font(type_scale::BODY),
        t.p.fg_chrome,
    );
    w::focus_ring(ui, rect, r, &response);
    response
}
/// Today's figures: net sales first, then the parts (never drawer cash).
fn today_body(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let s = app.data.today_summary.clone();
    if s.date != worker::today().to_string() {
        ui.label(t.muted(
            lang.text("กำลังโหลดยอดวันนี้…", "Loading today's activity…"),
            type_scale::BODY,
        ));
        return;
    }
    ui.horizontal(|ui| {
        ui.label(t.muted(lang.text("ยอดขายสุทธิ", "Net sales"), type_scale::BODY));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format::signed(s.net))
                    .font(t.strong_font(type_scale::MONEY_XL))
                    .color(if s.net.satang() < 0 {
                        t.p.danger_fg
                    } else {
                        t.p.fg_default
                    }),
            );
        });
    });
    sparkline(ui, &t, &s.hourly);
    ui.add_space(space::S2);
    ui.spacing_mut().item_spacing.y = space::S1;
    w::kv(
        ui,
        &format!(
            "{} · {}",
            lang.text("ยอดขายก่อนคืน", "Gross sales"),
            format::count(s.sale_count)
        ),
        &format::money(s.gross_sales),
        w::Value::Plain,
    );
    w::kv(
        ui,
        lang.text("คืนเงิน", "Refunds"),
        &format::signed(format::negative(s.refunds)),
        if s.refunds.satang() > 0 {
            w::Value::Danger
        } else {
            w::Value::Plain
        },
    );
    w::kv(
        ui,
        lang.text("บิล", "Bills"),
        &format::count(s.sale_count),
        w::Value::Plain,
    );
    if let Some(drawer) = app.data.shift.as_ref().and_then(|s| s.expected().ok()) {
        w::kv(
            ui,
            lang.text("เงินสดที่ควรมีในลิ้นชัก", "Expected drawer cash"),
            &format::signed(drawer),
            w::Value::Plain,
        );
    }
    w::help_text(
        ui,
        lang.text(
            "ยอดขายสุทธิไม่ใช่เงินในลิ้นชัก (รวมโอนและบัตร)",
            "Net sales are not drawer cash (they include transfers and cards).",
        ),
    );
}
/// Today's sales by hour as a small line, from the first hour with sales
/// to now.
fn sparkline(ui: &mut egui::Ui, t: &Theme, hourly: &[pos_domain::Money; 24]) {
    let now = format::local(time::OffsetDateTime::now_utc().unix_timestamp())
        .map_or(23, |d| usize::from(d.hour()));
    let first = hourly
        .iter()
        .position(|m| m.satang() > 0)
        .unwrap_or(now)
        .min(now);
    let last = now.max(hourly.iter().rposition(|m| m.satang() > 0).unwrap_or(now));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), egui::Sense::hover());
    let max = hourly.iter().map(|m| m.satang()).max().unwrap_or(0).max(1) as f32;
    let span = (last - first).max(1) as f32;
    let points: Vec<egui::Pos2> = (first..=last)
        .map(|h| {
            egui::pos2(
                rect.left() + (h - first) as f32 / span * rect.width(),
                rect.bottom() - 4.0 - hourly[h].satang() as f32 / max * (rect.height() - 8.0),
            )
        })
        .collect();
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, t.p.chart_grid),
    );
    if points.len() > 1 {
        ui.painter().add(egui::Shape::line(
            points,
            egui::Stroke::new(2.0, t.p.chart_1),
        ));
    }
}
#[derive(Clone, Copy)]
enum TaskAction {
    ContinueCount,
    Backup,
    LowStock,
    Held,
    Printer,
}
struct Task {
    title: String,
    detail: String,
    action: TaskAction,
    button: &'static str,
    tone: w::Tone,
    /// Shown instead of the button when all is well (e.g. "ปกติ").
    badge: Option<&'static str>,
}
fn tasks(app: &App) -> Vec<Task> {
    let lang = app.lang;
    let mut out = Vec::new();
    if let Some(shift) = &app.data.shift
        && app.home.drafts.contains(&shift.id)
    {
        out.push(Task {
            title: lang
                .text("นับเงินปิดกะค้างไว้", "Drawer count in progress")
                .into(),
            detail: lang
                .text("นับต่อจากที่บันทึกไว้", "Continue where you left off")
                .into(),
            action: TaskAction::ContinueCount,
            button: lang.text("ทำต่อ", "Continue"),
            tone: w::Tone::Info,
            badge: None,
        });
    }
    let week = Duration::from_secs(7 * 24 * 3600);
    let latest_backup = app.home.last_backup.max(app.auto_backup.latest);
    if latest_backup.is_none_or(|t| t.elapsed().is_ok_and(|e| e > week))
        && app.data.counts.total > 0
    {
        out.push(Task {
            title: match latest_backup {
                None => lang.text("ยังไม่เคยสำรองข้อมูล", "Never backed up"),
                Some(_) => lang.text("สำรองข้อมูลล่าสุดนานกว่า 7 วัน", "Last backup over 7 days ago"),
            }
            .into(),
            detail: lang
                .text("ข้อมูลอยู่ในเครื่องนี้เท่านั้น", "Data lives only on this device")
                .into(),
            action: TaskAction::Backup,
            button: lang.text("สำรองตอนนี้", "Back up now"),
            tone: w::Tone::Warning,
            badge: None,
        });
    }
    let low = app.data.counts.low_stock + app.data.counts.out_of_stock;
    if low > 0 {
        out.push(Task {
            title: format!(
                "{} {low} {}",
                lang.text("สต็อกต่ำ", "Low stock:"),
                lang.text("รายการ", "items")
            ),
            detail: format!(
                "{} {} · {} {}",
                lang.text("ใกล้หมด", "low"),
                app.data.counts.low_stock,
                lang.text("หมด", "out"),
                app.data.counts.out_of_stock
            ),
            action: TaskAction::LowStock,
            button: lang.text("ดูสินค้า", "View products"),
            tone: w::Tone::Warning,
            badge: None,
        });
    }
    let held = app.held_bills().len();
    if held > 0 {
        out.push(Task {
            title: format!(
                "{} {held} {}",
                lang.text("บิลค้างชำระ", "Unpaid bills:"),
                lang.text("บิล", "")
            ),
            detail: lang
                .text(
                    "ต้องชำระหรือพักก่อนปิดกะ",
                    "Pay or keep them before closing the shift",
                )
                .into(),
            action: TaskAction::Held,
            button: lang.text("ดูบิล", "View bills"),
            tone: w::Tone::Info,
            badge: None,
        });
    }
    let (title, detail, tone) = if app.print.failed.is_some() {
        (
            lang.text("พิมพ์ไม่สำเร็จ", "Printing failed"),
            lang.text("ตรวจเครื่องพิมพ์แล้วพิมพ์ซ้ำ", "Check the printer and print again"),
            w::Tone::Danger,
        )
    } else if app.print.settings.enabled() {
        (
            lang.text("เครื่องพิมพ์พร้อม", "Printer ready"),
            lang.text("ใบเสร็จพิมพ์ได้", "Receipts can print"),
            w::Tone::Success,
        )
    } else {
        (
            lang.text("ยังไม่ได้ตั้งเครื่องพิมพ์", "No printer set up"),
            lang.text("ตั้งค่าได้ในการพิมพ์", "Set one up in Printing"),
            w::Tone::Neutral,
        )
    };
    out.push(Task {
        title: title.into(),
        detail: detail.into(),
        action: TaskAction::Printer,
        button: lang.text("ตั้งค่า", "Settings"),
        badge: matches!(tone, w::Tone::Success).then(|| lang.text("ปกติ", "OK")),
        tone,
    });
    out
}
fn run_task(app: &mut App, action: TaskAction) {
    match action {
        TaskAction::ContinueCount => {
            app.drawer.tab = crate::shift_ui::Tab::Close;
            app.navigate(Page::Shift);
        }
        TaskAction::Backup => {
            app.backup();
            app.home.checked = None;
        }
        TaskAction::LowStock => app.navigate(Page::Products),
        TaskAction::Held => {
            app.navigate(Page::Sell);
            app.sell.tab = crate::sale_ui::CartTab::Held;
        }
        TaskAction::Printer => {
            app.settings_section = crate::settings_ui::Section::Printing;
            app.navigate(Page::Settings);
        }
    }
}
/// A to-do card: status stripe, title, detail and its action. True = act.
fn task_card(ui: &mut egui::Ui, task: &Task) -> bool {
    let t = Theme::of(ui);
    let stripe = match task.tone {
        w::Tone::Danger => t.p.danger_solid,
        w::Tone::Warning => t.p.warning_solid,
        w::Tone::Success => t.p.success_solid,
        w::Tone::Info => t.p.info_solid,
        _ => t.p.border_control,
    };
    let mut clicked = false;
    let r = egui::Frame::new()
        .fill(t.p.bg_raised)
        .stroke(egui::Stroke::new(1.0, t.p.border_subtle))
        .corner_radius(t.radius(crate::ui::tokens::radius::LG))
        .inner_margin(egui::Margin {
            left: 18,
            right: 10,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let text_w = (ui.available_width() - 120.0).max(120.0);
                ui.allocate_ui_with_layout(
                    vec2(text_w, 0.0),
                    egui::Layout::top_down(Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add(
                            egui::Label::new(t.text(&task.title, type_scale::BODY_STRONG)).wrap(),
                        );
                        ui.add(egui::Label::new(t.muted(&task.detail, type_scale::CAPTION)).wrap());
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if let Some(badge) = task.badge {
                        w::badge(ui, badge, w::Tone::Success);
                    } else if w::button(task.button).size(w::Size::Sm).show(ui).clicked() {
                        clicked = true;
                    }
                });
            });
        });
    let rect = r.response.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.min + vec2(8.0, 10.0), vec2(3.0, rect.height() - 20.0)),
        1,
        stripe,
    );
    clicked
}
/// A held bill or open table: name, detail, amount. True = open it.
fn held_card(app: &App, ui: &mut egui::Ui, cart: &pos_domain::Cart) -> bool {
    let t = Theme::of(ui);
    let lang = app.lang;
    let table = cart
        .table
        .and_then(|id| app.data.tables.iter().find(|x| x.id == id))
        .map(|x| x.name.clone());
    let name = match (&table, cart.label.is_empty()) {
        (Some(n), _) if cart.guests > 0 => {
            format!("{n} · {} {}", cart.guests, lang.text("คน", "guests"))
        }
        (Some(n), _) => n.clone(),
        (None, false) => cart.label.clone(),
        (None, true) => format!(
            "{} {}",
            lang.text("บิล", "Bill"),
            &cart.id.simple().to_string()[..6]
        ),
    };
    let lines = cart.lines.len() as u64;
    let detail = format!(
        "{} · {} {}",
        if table.is_some() {
            lang.text("โต๊ะที่เปิด", "Open table")
        } else {
            lang.text("พักไว้", "Held")
        },
        format::count(lines),
        lang.noun(lines, "รายการ", "line", "lines")
    );
    let total = app
        .price_cart(cart)
        .map_or("—".into(), |p| format::money(p.total));
    let r = egui::Frame::new()
        .fill(t.p.bg_raised)
        .stroke(egui::Stroke::new(1.0, t.p.border_subtle))
        .corner_radius(t.radius(crate::ui::tokens::radius::LG))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    vec2((ui.available_width() - 110.0).max(100.0), 0.0),
                    egui::Layout::top_down(Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.add(egui::Label::new(t.text(&name, type_scale::BODY_STRONG)).truncate());
                        ui.label(t.muted(&detail, type_scale::CAPTION));
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(total).font(t.strong_font(type_scale::MONEY)));
                });
            });
        })
        .response
        .interact(egui::Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    if r.hovered() {
        ui.painter().rect_stroke(
            r.rect,
            t.radius(crate::ui::tokens::radius::LG),
            egui::Stroke::new(1.0, t.p.border_control),
            egui::StrokeKind::Inside,
        );
    }
    r.on_hover_text(lang.text("เปิดบิลนี้", "Open this bill"))
        .clicked()
}
