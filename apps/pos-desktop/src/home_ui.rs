//! Home (design/docs/05: shift status · today's totals · shortcuts · pending work).
//! Today's totals come from `today_summary`, never the report's selected date,
//! and are labelled as sales — not drawer cash.
use crate::{
    app::{App, Page},
    ui::{Icon, Theme, format, icons, space, type_scale, w},
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

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    scan(app);
    let today = worker::today();
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(t.text(&app.shop.name, type_scale::TITLE_LG)).truncate());
        ui.label(t.muted(format::date(lang, today), type_scale::BODY));
    });
    ui.add_space(space::S2);
    egui::ScrollArea::vertical()
        .id_salt("home")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            today_tiles(app, ui);
            ui.add_space(10.0);
            w::columns(ui, 2, |cols| {
                t.surface().show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width());
                    shift_card(app, ui);
                });
                t.surface().show(&mut cols[1], |ui| {
                    ui.set_width(ui.available_width());
                    pending(app, ui);
                });
            });
            ui.add_space(10.0);
            t.surface().show(ui, |ui| {
                ui.set_width(ui.available_width());
                w::title(ui, lang.text("ทางลัด", "Shortcuts"));
                ui.add_space(space::S2);
                ui.horizontal_wrapped(|ui| {
                    for (page, icon, th, en, key) in [
                        (Page::Sell, Icon::Sell, "ขาย", "Sell", "Alt 2"),
                        (Page::Products, Icon::Products, "สินค้า", "Products", "Alt 5"),
                        (
                            Page::History,
                            Icon::History,
                            "ประวัติขาย",
                            "Sales history",
                            "Alt 3",
                        ),
                        (Page::Reports, Icon::Reports, "รายงาน", "Reports", "Alt 6"),
                    ] {
                        if w::button(lang.text(th, en))
                            .icon(icon)
                            .size(w::Size::Lg)
                            .shortcut(key)
                            .min_width(160.0)
                            .show(ui)
                            .clicked()
                        {
                            app.navigate(page);
                        }
                    }
                });
            });
        });
}
fn today_tiles(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let s = app.data.today_summary.clone();
    if s.date != worker::today().to_string() {
        w::state_view(
            ui,
            Icon::Reports,
            lang.text("กำลังโหลดยอดวันนี้…", "Loading today's activity…"),
            "",
        );
        return;
    }
    let gap = 10.0;
    let width = ((ui.available_width() - gap * 3.0) / 4.0).floor();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            w::stat_tile(
                ui,
                lang.text("ยอดขายวันนี้ (ก่อนคืน)", "Today's gross sales"),
                &format::money(s.gross_sales),
                false,
                &[(
                    format!(
                        "{} {}",
                        format::count(s.sale_count),
                        lang.noun(s.sale_count, "บิล", "bill", "bills")
                    ),
                    None,
                )],
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            w::stat_tile(
                ui,
                lang.text("คืนเงินวันนี้", "Refunds today"),
                &format::signed(format::negative(s.refunds)),
                s.refunds.satang() > 0,
                &[(
                    format!(
                        "{} {}",
                        s.void_count + s.refund_count,
                        lang.noun(s.void_count + s.refund_count, "ครั้ง", "refund", "refunds")
                    ),
                    None,
                )],
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            w::stat_tile(
                ui,
                lang.text("ยอดขายสุทธิวันนี้", "Net sales today"),
                &format::signed(s.net),
                s.net.satang() < 0,
                &[(
                    lang.text("ไม่ใช่เงินในลิ้นชัก", "Not drawer cash").to_owned(),
                    None,
                )],
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            let drawer = app.data.shift.as_ref().and_then(|s| s.expected().ok());
            w::stat_tile(
                ui,
                lang.text("เงินสดที่ควรมีในลิ้นชัก", "Expected drawer cash"),
                &drawer.map_or("—".into(), format::signed),
                drawer.is_some_and(|d| d.satang() < 0),
                &[(
                    if app.data.shift.is_some() {
                        lang.text("กะที่เปิดอยู่", "Open shift")
                    } else {
                        lang.text("ยังไม่เปิดกะ", "No open shift")
                    }
                    .to_owned(),
                    None,
                )],
            );
        });
    });
}
fn shift_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.spacing_mut().item_spacing.y = space::S2;
    w::title(ui, lang.text("กะและลิ้นชัก", "Shift & drawer"));
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    match app.data.shift.clone() {
        Some(shift) => {
            ui.horizontal(|ui| {
                w::badge(ui, lang.text("เปิดกะอยู่", "Shift open"), w::Tone::Success);
                ui.label(t.muted(
                    format!(
                        "{} {} · {}",
                        lang.text("เปิด", "since"),
                        format::clock(shift.opened_at),
                        format::duration(lang, now - shift.opened_at)
                    ),
                    type_scale::LABEL,
                ));
            });
            if let Ok(expected) = shift.expected() {
                w::kv_styled(
                    ui,
                    lang.text("เงินสดที่ควรมี", "Expected cash"),
                    &format::signed(expected),
                    w::Value::Strong,
                    type_scale::MONEY_LG,
                );
            }
        }
        None => {
            w::badge(
                ui,
                lang.text("ยังไม่เปิดกะ", "No open shift"),
                w::Tone::Warning,
            );
            w::help_text(
                ui,
                lang.text(
                    "นับเงินตั้งต้นแล้วเปิดกะ ก่อนรับเงินหรือคืนเงิน",
                    "Count the opening float and open a shift before taking or refunding payment.",
                ),
            );
        }
    }
    let active = !app.cart.lines.is_empty();
    let label = if app.data.shift.is_none() {
        lang.text("เปิดกะ", "Open shift")
    } else if active {
        lang.text("ขายต่อ", "Continue selling")
    } else {
        lang.text("เริ่มขาย", "Start selling")
    };
    ui.horizontal_wrapped(|ui| {
        if w::button(label)
            .primary()
            .size(w::Size::Lg)
            .icon(if app.data.shift.is_none() {
                Icon::Shift
            } else {
                Icon::Sell
            })
            .enabled(app.ready && !app.busy)
            .show(ui)
            .clicked()
        {
            if app.data.shift.is_none() {
                crate::shift_ui::goto_open(app);
            } else {
                app.navigate(Page::Sell);
            }
        }
        if app.data.shift.is_some()
            && w::button(lang.text("นับเงินและปิดกะ", "Count & close"))
                .size(w::Size::Lg)
                .show(ui)
                .clicked()
        {
            app.drawer.selected = None;
            app.drawer.tab = crate::shift_ui::Tab::Close;
            app.navigate(Page::Shift);
        }
    });
}
fn pending(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.spacing_mut().item_spacing.y = space::S2;
    w::title(ui, lang.text("งานค้าง", "Pending work"));
    let mut any = false;
    if let Some(shift) = &app.data.shift
        && app.home.drafts.contains(&shift.id)
    {
        any = true;
        if w::banner(
            ui,
            w::Tone::Info,
            lang.text("มีการนับเงินปิดกะที่ทำค้างไว้", "A drawer count is in progress"),
            Some(lang.text("ทำต่อ", "Continue")),
        ) {
            app.drawer.tab = crate::shift_ui::Tab::Close;
            app.navigate(Page::Shift);
        }
    }
    let week = Duration::from_secs(7 * 24 * 3600);
    let latest_backup = app.home.last_backup.max(app.auto_backup.latest);
    let backup_old = latest_backup.is_none_or(|t| t.elapsed().is_ok_and(|e| e > week));
    if backup_old && app.data.counts.total > 0 {
        any = true;
        let text = match latest_backup {
            None => lang.text("ยังไม่เคยสำรองข้อมูลในเครื่องนี้", "No backup on this device yet"),
            Some(_) => lang.text(
                "สำรองข้อมูลล่าสุดนานกว่า 7 วัน",
                "Last backup is more than 7 days old",
            ),
        };
        if w::banner(
            ui,
            w::Tone::Warning,
            text,
            Some(lang.text("สำรองตอนนี้", "Back up now")),
        ) {
            app.backup();
            app.home.checked = None;
        }
    }
    let held = app.held_bills();
    if !held.is_empty() {
        any = true;
        ui.label(t.text(
            format!("{} {}", lang.text("บิลที่พักไว้", "Held bills"), held.len()),
            type_scale::BODY_STRONG,
        ));
        for cart in held.iter().take(5) {
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Pause, 16.0, t.p.fg_muted);
                let label = if cart.label.is_empty() {
                    format!(
                        "{} {}",
                        lang.text("บิล", "Bill"),
                        &cart.id.simple().to_string()[..6]
                    )
                } else {
                    cart.label.clone()
                };
                ui.label(t.text(label, type_scale::BODY));
                ui.label(t.muted(
                    format!(
                        "{} {}",
                        cart.lines.len(),
                        lang.noun(cart.lines.len() as u64, "รายการ", "line", "lines")
                    ),
                    type_scale::CAPTION,
                ));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if w::button(lang.text("เปิดบิล", "Open"))
                        .size(w::Size::Sm)
                        .enabled(!app.busy)
                        .show(ui)
                        .clicked()
                    {
                        app.resume(cart.id);
                    }
                });
            });
        }
        if held.len() > 5 {
            w::help_text(
                ui,
                &format!("{} {}", lang.text("และอีก", "and"), held.len() - 5),
            );
        }
    }
    if !any {
        ui.horizontal(|ui| {
            icons::show(ui, Icon::CheckCircle, 18.0, t.p.success_fg);
            ui.label(RichText::new(lang.text("ไม่มีงานค้าง", "Nothing pending")).color(t.p.fg_muted));
        });
    }
}
