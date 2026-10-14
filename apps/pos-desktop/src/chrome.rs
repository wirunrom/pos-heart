//! Window chrome (design v0.2 Navigation): workspace tabs → page toolbar →
//! panes on the graphite canvas → status bar. Everything here is drawn on
//! chrome colours; panes keep the paper palette.
use crate::{
    app::{App, Page},
    sale_ui::Pane,
    ui::{Icon, Theme, format, icons, space, theme as ds_theme, tokens::radius, type_scale, w},
};
use eframe::egui::{
    self, Align, Align2, Color32, Rect, Response, Sense, Stroke, StrokeKind, pos2, vec2,
};

/// The workspaces shown as tabs (Alt 1–7). Activity opens from Reports;
/// Tables is a pane of Sell.
const TABS: [Page; 7] = [
    Page::Home,
    Page::Sell,
    Page::History,
    Page::Shift,
    Page::Products,
    Page::Reports,
    Page::Settings,
];
fn tab_of(page: Page) -> Page {
    match page {
        Page::Tables => Page::Sell,
        Page::Activity => Page::Reports,
        p => p,
    }
}

// ───────────────────────────── Title bar ─────────────────────────────

pub fn title_bar(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let narrow = ui.available_width() < 1180.0;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = if t.soft() { space::S1 } else { 0.0 };
        if app.onboarding {
            ui.label(t.chrome(lang.text("เริ่มต้นร้าน", "Shop setup"), type_scale::BODY_STRONG));
        } else {
            let current = tab_of(app.page);
            for (i, page) in TABS.into_iter().enumerate() {
                if tab(
                    ui,
                    &t,
                    crate::shell::page_label(lang, page),
                    i + 1,
                    page == current,
                )
                .clicked()
                {
                    app.navigate(page);
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = space::S3;
            let context = if app.demo {
                format!(
                    "{} · {}",
                    app.shop.name,
                    lang.text("ข้อมูลตัวอย่าง", "Demo data")
                )
            } else {
                app.shop.name.clone()
            };
            if !narrow || app.onboarding {
                let width = 180.0_f32.min(ui.available_width() * 0.3);
                ui.allocate_ui_with_layout(
                    vec2(width, ui.available_height()),
                    egui::Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(t.chrome_muted(context, type_scale::LABEL)).truncate(),
                        )
                        .on_hover_text(&app.shop.name);
                    },
                );
            }
            if !app.onboarding {
                if app.busy {
                    w::spinner(
                        ui,
                        ui.cursor().left_center() + vec2(-12.0, 0.0),
                        7.0,
                        t.p.fg_chrome_muted,
                    );
                    ui.add_space(24.0);
                }
                crate::shell::command_field(app, ui, t, if narrow { 200.0 } else { 280.0 });
            }
        });
    });
}
/// A workspace tab: selected = chrome-hover fill + 2 px accent underline.
fn tab(ui: &mut egui::Ui, t: &Theme, label: &str, n: usize, active: bool) -> Response {
    let font = if active {
        t.strong_font(type_scale::BODY)
    } else {
        t.font(type_scale::BODY)
    };
    let fg = if active {
        t.p.fg_chrome
    } else {
        t.p.fg_chrome_muted
    };
    let g = ui.painter().layout_no_wrap(label.to_owned(), font, fg);
    let hint = (active && t.m.show_shortcut_hints).then(|| {
        ui.painter().layout_no_wrap(
            format!("Alt {n}"),
            t.font(type_scale::MONEY_SM),
            t.p.fg_chrome_muted,
        )
    });
    let narrow = ui.ctx().content_rect().width() < 1180.0;
    let pad = if t.touch() && !narrow {
        space::S4
    } else if narrow {
        space::S2 + 2.0
    } else {
        space::S3
    };
    let width = g.size().x + pad * 2.0 + hint.as_ref().map_or(0.0, |h| h.size().x + space::S2);
    let height = if t.soft() {
        (t.m.titlebar_h - space::S3).max(26.0)
    } else {
        ui.available_height()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, label));
    let r = t.radius(radius::MD);
    if active || response.hovered() {
        ui.painter().rect_filled(rect, r, t.p.bg_chrome_hover);
    }
    ui.painter().galley(
        pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0),
        g.clone(),
        fg,
    );
    if let Some(h) = hint {
        ui.painter().galley(
            pos2(
                rect.left() + pad + g.size().x + space::S2,
                rect.center().y - h.size().y / 2.0,
            ),
            h,
            t.p.fg_chrome_muted,
        );
    }
    if active {
        let y = rect.bottom() - if t.soft() { 3.0 } else { 1.0 };
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(rect.left() + pad, y - 2.0),
                pos2(rect.right() - pad, y),
            ),
            1,
            t.p.accent_bg,
        );
    }
    w::focus_ring(ui, rect, egui::CornerRadius::same(r), &response);
    response
}

// ───────────────────────────── Toolbar ─────────────────────────────

pub fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = space::S1;
        if app.onboarding {
            return;
        }
        match app.page {
            Page::Sell | Page::Tables => sell_tools(app, ui),
            Page::Home => {
                if tool(
                    ui,
                    Icon::Plus,
                    lang.text("บิลใหม่", "New bill"),
                    Some("Ctrl N"),
                    true,
                )
                .clicked()
                {
                    new_bill(app);
                }
                if tool(
                    ui,
                    Icon::Shift,
                    lang.text("นับเงินและปิดกะ", "Count & close"),
                    None,
                    app.data.shift.is_some(),
                )
                .clicked()
                {
                    app.drawer.selected = None;
                    app.drawer.tab = crate::shift_ui::Tab::Close;
                    app.navigate(Page::Shift);
                }
                if tool(
                    ui,
                    Icon::Download,
                    lang.text("สำรองข้อมูล", "Back up"),
                    None,
                    app.ready && !app.busy,
                )
                .clicked()
                {
                    app.backup();
                }
            }
            Page::Reports => {
                let clicked = tool(
                    ui,
                    Icon::History,
                    lang.text("ประวัติกิจกรรม", "Activity log"),
                    Some("Alt 8"),
                    true,
                )
                .clicked();
                if clicked {
                    app.navigate(Page::Activity);
                }
            }
            Page::Activity => {
                let clicked = tool(
                    ui,
                    Icon::ChevronLeft,
                    lang.text("กลับไปรายงาน", "Back to reports"),
                    None,
                    true,
                )
                .clicked();
                if clicked {
                    app.navigate(Page::Reports);
                }
            }
            Page::Settings => {
                let clicked = tool(
                    ui,
                    Icon::Download,
                    lang.text("สำรองข้อมูล", "Back up"),
                    None,
                    app.ready && !app.busy,
                )
                .clicked();
                if clicked {
                    app.backup();
                }
            }
            _ => {}
        }
    });
}
/// "New bill": keeps the open bill (held) and starts an empty one.
pub fn new_bill(app: &mut App) {
    if app.page != Page::Sell {
        app.navigate(Page::Sell);
    }
    app.sell.tab = crate::sale_ui::CartTab::Current;
    app.hold_cart();
}
fn sell_tools(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let lines = !app.cart.lines.is_empty() && !app.busy;
    if tool(
        ui,
        Icon::Plus,
        lang.text("บิลใหม่", "New bill"),
        Some("Ctrl N"),
        lines,
    )
    .clicked()
    {
        new_bill(app);
    }
    separator(ui, &t);
    ui.label(t.chrome_muted(lang.text("ทั้งบิล", "Whole bill"), type_scale::CAPTION));
    if tool(
        ui,
        Icon::Pause,
        lang.text("พักบิล", "Hold"),
        Some("F4"),
        lines,
    )
    .clicked()
    {
        app.hold_cart();
    }
    if tool(
        ui,
        Icon::Tag,
        lang.text("ส่วนลดบิล", "Bill discount"),
        Some("F8"),
        lines,
    )
    .clicked()
    {
        app.sell.discount_requested = Some(None);
    }
    let units: u32 = app.cart.lines.iter().map(|l| l.quantity).sum();
    if tool(
        ui,
        Icon::Options,
        lang.text("แยกบิล", "Split bill"),
        None,
        lines && units > 1,
    )
    .clicked()
    {
        app.sell.bill_split = Some(crate::sale_ui::BillSplit::new(&app.cart));
    }
    if !app.data.tables.is_empty() {
        let known = &app.known;
        let kitchen = |id: uuid::Uuid| known.get(&id).is_some_and(|p| p.kitchen);
        let changes = pos_domain::kitchen_changes(&app.cart, kitchen).0.len();
        if tool(
            ui,
            Icon::Printer,
            lang.text("ส่งครัว", "Send to kitchen"),
            Some("F10"),
            changes > 0 && !app.busy,
        )
        .clicked()
        {
            app.send_kitchen();
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            let busy = crate::tables_ui::busy_count(app);
            let before = app.sell.pane;
            let tables_label = if busy > 0 {
                format!(
                    "{} · {busy} {}",
                    lang.text("โต๊ะ", "Tables"),
                    lang.text("เปิด", "open")
                )
            } else {
                lang.text("โต๊ะ", "Tables").to_owned()
            };
            if !crate::tables_ui::active(app).is_empty() {
                segmented(
                    ui,
                    &mut app.sell.pane,
                    &[
                        (Pane::Products, lang.text("สินค้า", "Products").to_owned()),
                        (Pane::Tables, tables_label),
                    ],
                );
            }
            if before != app.sell.pane {
                app.sell.moving = false;
            }
        });
    }
}

/// A toolbar command: icon + label (+ shortcut hint), chrome colours.
pub fn tool(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    hint: Option<&str>,
    enabled: bool,
) -> Response {
    let t = Theme::of(ui);
    let fg = if enabled {
        t.p.fg_chrome
    } else {
        t.p.fg_chrome_muted.gamma_multiply(0.6)
    };
    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), t.strong_font(type_scale::LABEL), fg);
    let hint = hint.filter(|_| t.m.show_shortcut_hints).map(|h| {
        ui.painter().layout_no_wrap(
            h.to_owned(),
            t.font(type_scale::MONEY_SM),
            t.p.fg_chrome_muted,
        )
    });
    let icon_size = if t.touch() { 20.0 } else { 16.0 };
    let pad = if t.touch() {
        space::S3
    } else {
        space::S2 + 2.0
    };
    let width = pad
        + icon_size
        + space::S2
        + g.size().x
        + hint.as_ref().map_or(0.0, |h| space::S2 + h.size().x)
        + pad;
    let height = if t.touch() { 48.0 } else { t.m.control_md };
    let (rect, response) = ui.allocate_exact_size(
        vec2(width, height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let r = t.radius(radius::MD);
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, r, t.p.bg_chrome_hover);
    }
    icons::paint_sized(
        ui.painter(),
        icon,
        pos2(rect.left() + pad + icon_size / 2.0, rect.center().y),
        icon_size,
        fg,
    );
    let x = rect.left() + pad + icon_size + space::S2;
    let gw = g.size().x;
    ui.painter()
        .galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
    if let Some(h) = hint {
        ui.painter().galley(
            pos2(x + gw + space::S2, rect.center().y - h.size().y / 2.0),
            h,
            t.p.fg_chrome_muted,
        );
    }
    w::focus_ring(ui, rect, egui::CornerRadius::same(r), &response);
    response
}
pub fn separator(ui: &mut egui::Ui, t: &Theme) {
    let (rect, _) = ui.allocate_exact_size(vec2(space::S3, t.m.control_sm), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, t.p.border_chrome),
    );
}
/// Segmented control on chrome (view switches at the right of a toolbar).
pub fn segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, String)],
) -> bool {
    let t = Theme::of(ui);
    let font = t.strong_font(type_scale::LABEL);
    let pad = if t.touch() { space::S4 } else { space::S3 };
    let height = if t.touch() { 48.0 } else { t.m.control_md };
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, l)| {
            ui.painter()
                .layout_no_wrap(l.clone(), font.clone(), t.p.fg_chrome)
        })
        .collect();
    let inner = 3.0;
    let width: f32 = galleys.iter().map(|g| g.size().x + pad * 2.0).sum::<f32>() + inner * 2.0;
    let (track, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter()
        .rect_filled(track, t.radius(radius::MD) + 1, t.p.bg_chrome);
    let mut x = track.left() + inner;
    let mut changed = false;
    for ((v, label), g) in options.iter().zip(galleys) {
        let w_ = g.size().x + pad * 2.0;
        let rect =
            Rect::from_min_size(pos2(x, track.top() + inner), vec2(w_, height - inner * 2.0));
        let response = ui.interact(rect, ui.id().with(("chrome-seg", label)), Sense::click());
        let on = *value == *v;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label)
        });
        if on || response.hovered() {
            ui.painter()
                .rect_filled(rect, t.radius(radius::MD) - 1, t.p.bg_chrome_hover);
        }
        let fg = if on {
            t.p.fg_chrome
        } else {
            t.p.fg_chrome_muted
        };
        ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
        w::focus_ring(
            ui,
            rect,
            egui::CornerRadius::same(t.radius(radius::MD)),
            &response,
        );
        if response.clicked() && !on {
            *value = *v;
            changed = true;
        }
        x += w_;
    }
    changed
}

// ───────────────────────────── Status bar ─────────────────────────────

pub fn status_bar(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    // Narrow windows keep every item but use short labels.
    let narrow = ui.available_width() < if t.touch() { 1280.0 } else { 1100.0 };
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if let Some(me) = app.data.signed_in.clone() {
            let r = item(
                ui,
                &t,
                &me.name,
                Some(Dot::Avatar(me.name.chars().next().unwrap_or('?'))),
            );
            let role = crate::staff_ui::role_label(app, &me.role);
            egui::Popup::menu(&r)
                .frame(
                    t.raised(2)
                        .inner_margin(egui::Margin::same(space::S1 as i8)),
                )
                .width(220.0)
                .show(|ui| {
                    ui.label(t.muted(&role, type_scale::CAPTION));
                    if crate::shell::menu_row(
                        ui,
                        Icon::Lock,
                        lang.text("ล็อกเครื่อง / สลับผู้ใช้", "Lock / switch user"),
                        "Ctrl L",
                        false,
                    )
                    .clicked()
                    {
                        crate::staff_ui::lock(app);
                    }
                });
            divider(ui, &t);
        }
        let offline = if narrow {
            lang.text("ออฟไลน์", "Offline")
        } else {
            lang.text("ออฟไลน์ · ข้อมูลอยู่ในเครื่องนี้", "Offline · data on this device")
        };
        item(ui, &t, offline, Some(Dot::Hollow)).on_hover_text(lang.text(
            "ทำงานในเครื่องนี้ ไม่ต้องใช้อินเทอร์เน็ต",
            "Works on this device without internet",
        ));
        divider(ui, &t);
        if !app.onboarding {
            let (text, dot) = match &app.data.shift {
                Some(s) if narrow => (
                    format!(
                        "{} {}",
                        lang.text("กะ", "Shift"),
                        format::clock(s.opened_at)
                    ),
                    t.p.success_solid,
                ),
                Some(s) => (
                    format!(
                        "{} · {} {}",
                        lang.text("กะเปิด", "Shift open"),
                        lang.text("เปิด", "since"),
                        format::clock(s.opened_at)
                    ),
                    t.p.success_solid,
                ),
                None => (
                    lang.text("ยังไม่เปิดกะ", "No open shift").to_owned(),
                    t.p.warning_solid,
                ),
            };
            if item(ui, &t, &text, Some(Dot::Solid(dot))).clicked() {
                app.drawer.selected = None;
                app.navigate(Page::Shift);
            }
            divider(ui, &t);
            if let Some(expected) = app.data.shift.as_ref().and_then(|s| s.expected().ok()) {
                if money_item(
                    ui,
                    &t,
                    lang.text("ลิ้นชัก", "Drawer"),
                    &format::signed(expected),
                )
                .clicked()
                {
                    app.drawer.selected = None;
                    app.navigate(Page::Shift);
                }
                divider(ui, &t);
            }
            let (text, dot) = if app.print.failed.is_some() {
                (lang.text("พิมพ์ไม่สำเร็จ", "Print failed"), t.p.danger_solid)
            } else if app.print.settings.enabled() {
                (
                    lang.text("เครื่องพิมพ์พร้อม", "Printer ready"),
                    t.p.success_solid,
                )
            } else if narrow {
                (lang.text("เครื่องพิมพ์", "Printer"), t.p.fg_chrome_muted)
            } else {
                (
                    lang.text("ไม่ได้ตั้งเครื่องพิมพ์", "No printer"),
                    t.p.fg_chrome_muted,
                )
            };
            if item(ui, &t, text, Some(Dot::Solid(dot))).clicked() {
                app.settings_section = crate::settings_ui::Section::Printing;
                app.navigate(Page::Settings);
            }
            divider(ui, &t);
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if app.demo {
                ui.add_space(space::S3);
                ui.label(
                    egui::RichText::new(lang.text("ข้อมูลตัวอย่าง", "Demo data"))
                        .font(t.font(type_scale::CAPTION))
                        .color(t.p.accent_bg),
                );
                divider(ui, &t);
            }
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            ui.add_space(space::S3);
            ui.label(
                egui::RichText::new(format::clock(now))
                    .font(t.font(type_scale::MONEY_SM))
                    .color(t.p.fg_chrome),
            );
            ui.add_space(space::S3);
            divider(ui, &t);
            let mut density = app.density;
            if pair(
                ui,
                &t,
                &mut density,
                [
                    (crate::ui::Density::Touch, lang.text("จอสัมผัส", "Touch")),
                    (
                        crate::ui::Density::Compact,
                        if narrow {
                            lang.text("เมาส์", "Mouse")
                        } else {
                            lang.text("เมาส์/คีย์บอร์ด", "Mouse/keyboard")
                        },
                    ),
                ],
            ) {
                app.set_display(app.look, density);
            }
            divider(ui, &t);
            let other = match lang {
                crate::i18n::Language::Thai => crate::i18n::Language::English,
                crate::i18n::Language::English => crate::i18n::Language::Thai,
            };
            if item(ui, &t, lang.text("ไทย", "English"), None)
                .on_hover_text(lang.text("เปลี่ยนเป็น English", "Switch to Thai"))
                .clicked()
            {
                app.set_language(other);
            }
            divider(ui, &t);
            let mut look = app.look;
            if pair(
                ui,
                &t,
                &mut look.shape,
                [
                    (ds_theme::Shape::Soft, lang.text("โค้ง", "Rounded")),
                    (ds_theme::Shape::Square, lang.text("เหลี่ยม", "Square")),
                ],
            ) {
                app.set_display(look, app.density);
            }
            divider(ui, &t);
            let (th, en) = ds_theme::accent_name(app.look.accent);
            let accent = if narrow {
                lang.text("สีหลัก", "Accent").to_owned()
            } else {
                format!("{}: {}", lang.text("สีหลัก", "Accent"), lang.text(th, en))
            };
            if item(ui, &t, &accent, Some(Dot::Solid(t.p.accent_bg))).clicked() {
                app.display_open = !app.display_open;
            }
        });
    });
}
enum Dot {
    Solid(Color32),
    Hollow,
    Avatar(char),
}
fn item(ui: &mut egui::Ui, t: &Theme, text: &str, dot: Option<Dot>) -> Response {
    let font = t.font(type_scale::CAPTION);
    let g = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, t.p.fg_chrome);
    let lead = match dot {
        Some(Dot::Avatar(_)) => 18.0 + space::S2,
        Some(_) => 7.0 + space::S2,
        None => 0.0,
    };
    let height = ui.available_height();
    let (rect, response) = ui.allocate_exact_size(
        vec2(space::S3 * 2.0 + lead + g.size().x, height),
        Sense::click(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    if response.hovered() {
        ui.painter().rect_filled(rect, 0, t.p.bg_chrome_hover);
    }
    let c = pos2(
        rect.left()
            + space::S3
            + if matches!(dot, Some(Dot::Avatar(_))) {
                9.0
            } else {
                3.5
            },
        rect.center().y,
    );
    match dot {
        Some(Dot::Solid(color)) => {
            ui.painter().circle_filled(c, 3.5, color);
        }
        Some(Dot::Hollow) => {
            ui.painter()
                .circle_stroke(c, 3.0, Stroke::new(1.2, t.p.fg_chrome_muted));
        }
        Some(Dot::Avatar(ch)) => {
            ui.painter().circle_filled(c, 9.0, t.p.accent_bg);
            ui.painter().text(
                c,
                Align2::CENTER_CENTER,
                ch,
                t.strong_font(type_scale::CAPTION),
                t.p.fg_on_accent,
            );
        }
        None => {}
    }
    ui.painter().galley(
        pos2(
            rect.left() + space::S3 + lead,
            rect.center().y - g.size().y / 2.0,
        ),
        g,
        t.p.fg_chrome,
    );
    response
}
fn money_item(ui: &mut egui::Ui, t: &Theme, label: &str, amount: &str) -> Response {
    let a = ui.painter().layout_no_wrap(
        label.to_owned(),
        t.font(type_scale::CAPTION),
        t.p.fg_chrome_muted,
    );
    let b = ui.painter().layout_no_wrap(
        amount.to_owned(),
        t.strong_font(type_scale::MONEY_SM),
        t.p.fg_chrome,
    );
    let height = ui.available_height();
    let (rect, response) = ui.allocate_exact_size(
        vec2(
            space::S3 * 2.0 + a.size().x + space::S2 + b.size().x,
            height,
        ),
        Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{label} {amount}"))
    });
    if response.hovered() {
        ui.painter().rect_filled(rect, 0, t.p.bg_chrome_hover);
    }
    let aw = a.size().x;
    ui.painter().galley(
        pos2(rect.left() + space::S3, rect.center().y - a.size().y / 2.0),
        a,
        t.p.fg_chrome_muted,
    );
    ui.painter().galley(
        pos2(
            rect.left() + space::S3 + aw + space::S2,
            rect.center().y - b.size().y / 2.0,
        ),
        b,
        t.p.fg_chrome,
    );
    response
}
/// Two-way switch in the status bar (drawn right to left: second first).
fn pair<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    t: &Theme,
    value: &mut T,
    options: [(T, &str); 2],
) -> bool {
    let mut changed = false;
    for (v, label) in options {
        let on = *value == v;
        let g = ui.painter().layout_no_wrap(
            label.to_owned(),
            t.font(type_scale::CAPTION),
            t.p.fg_chrome,
        );
        let height = ui.available_height();
        let (rect, response) =
            ui.allocate_exact_size(vec2(g.size().x + space::S3 * 2.0, height), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label)
        });
        let inner = rect.shrink2(vec2(0.0, 3.0));
        if on {
            ui.painter().rect(
                inner,
                t.radius(radius::XS),
                t.p.bg_chrome_hover,
                Stroke::NONE,
                StrokeKind::Inside,
            );
        } else if response.hovered() {
            ui.painter()
                .rect_filled(inner, t.radius(radius::XS), t.p.bg_chrome_raised);
        }
        ui.painter().galley(
            rect.center() - g.size() / 2.0,
            g,
            if on {
                t.p.fg_chrome
            } else {
                t.p.fg_chrome_muted
            },
        );
        if response.clicked() && !on {
            *value = v;
            changed = true;
        }
    }
    changed
}
fn divider(ui: &mut egui::Ui, t: &Theme) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, ui.available_height()), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range().shrink(4.0),
        Stroke::new(1.0, t.p.border_chrome),
    );
}
