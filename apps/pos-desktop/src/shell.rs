//! App frame: Topbar (WorkspaceSwitcher · context · CommandBar · StatusPill),
//! toasts and the Ctrl K command palette (design/docs/05 and components/Navigation.md).
use crate::{
    app::{App, Page},
    i18n::Language,
    ui::{Icon, Theme, format, icons, space, theme as ds_theme, tokens::radius, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, Key, Modifiers, RichText, Sense, Stroke, StrokeKind, vec2};
use std::time::{Duration, Instant};

pub const PAGES: [(Page, Icon, &str, &str); 9] = [
    (Page::Home, Icon::Home, "หน้าหลัก", "Home"),
    (Page::Sell, Icon::Sell, "ขาย", "Sell"),
    (Page::History, Icon::History, "ประวัติขาย", "Sales history"),
    (Page::Shift, Icon::Shift, "กะและลิ้นชัก", "Shift & drawer"),
    (Page::Products, Icon::Products, "สินค้า", "Products"),
    (Page::Reports, Icon::Reports, "รายงาน", "Reports"),
    (Page::Settings, Icon::Settings, "ตั้งค่า", "Settings"),
    (Page::Activity, Icon::History, "ประวัติกิจกรรม", "Activity log"),
    (Page::Tables, Icon::Sell, "โต๊ะ", "Tables"),
];
pub fn page_label(lang: Language, page: Page) -> &'static str {
    PAGES
        .iter()
        .find(|(p, ..)| *p == page)
        .map_or("", |(_, _, th, en)| lang.text(th, en))
}
// ───────────────────────────── Toasts ─────────────────────────────

pub enum Undo {
    RestoreLine(usize, pos_domain::CartLine),
    RestoreCart(pos_domain::Cart),
    ResumeHeld(uuid::Uuid),
    SetOnSale(Vec<uuid::Uuid>, bool),
}
pub struct Toast {
    pub text: String,
    pub undo: Option<Undo>,
    pub until: Instant,
}
impl App {
    /// Result of an action just taken. Undoable toasts last 6 s, others 4 s.
    pub fn toast(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), None);
    }
    pub fn toast_undo(&mut self, text: impl Into<String>, undo: Undo) {
        self.push_toast(text.into(), Some(undo));
    }
    fn push_toast(&mut self, text: String, undo: Option<Undo>) {
        let life = if undo.is_some() { 6 } else { 4 };
        self.toasts.push(Toast {
            text,
            undo,
            until: Instant::now() + Duration::from_secs(life),
        });
        // Never stack more than three (Feedback.md).
        while self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }
    pub fn undo_last(&mut self) {
        let Some(index) = self.toasts.iter().rposition(|t| t.undo.is_some()) else {
            return;
        };
        let toast = self.toasts.remove(index);
        if let Some(undo) = toast.undo {
            self.apply_undo(undo);
        }
    }
    fn apply_undo(&mut self, undo: Undo) {
        match undo {
            Undo::RestoreLine(index, line) => {
                let index = index.min(self.cart.lines.len());
                self.cart.lines.insert(index, line);
                self.sell.current = Some(index);
                self.sell.tab = crate::sale_ui::CartTab::Current;
            }
            Undo::RestoreCart(cart) => {
                if self.cart.lines.is_empty() {
                    self.cart = cart;
                    self.sell.current = None;
                }
            }
            Undo::ResumeHeld(id) => self.resume(id),
            Undo::SetOnSale(ids, on_sale) => self.send(Command::SetOnSale(ids, on_sale)),
        }
    }
}
pub fn toasts(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    app.toasts.retain(|t| t.until > now);
    if app.toasts.is_empty()
        || crate::staff_ui::locked(app)
        || ctx.memory(|m| m.top_modal_layer().is_some())
    {
        return;
    }
    if let Some(next) = app.toasts.iter().map(|t| t.until).min() {
        ctx.request_repaint_after(next.saturating_duration_since(now));
    }
    let t = Theme::current(ctx);
    let lang = app.lang;
    let mut undo = None;
    let mut dismiss = None;
    egui::Area::new(egui::Id::new("toasts"))
        .order(egui::Order::Tooltip)
        .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -space::S6))
        .interactable(true)
        .show(ctx, |ui| {
            for (i, toast) in app.toasts.iter().enumerate() {
                egui::Frame::new()
                    .fill(t.p.bg_inverse)
                    .corner_radius(t.radius(8))
                    .shadow(t.shadow(2))
                    .inner_margin(egui::Margin::symmetric(space::S4 as i8, space::S2 as i8))
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&toast.text)
                                        .font(t.font(type_scale::BODY))
                                        .color(t.p.fg_inverse),
                                )
                                .wrap(),
                            );
                            if toast.undo.is_some() {
                                let label = RichText::new(lang.text("เลิกทำ", "Undo"))
                                    .font(t.strong_font(type_scale::BODY))
                                    .color(t.p.accent_bg);
                                if ui
                                    .add(egui::Button::new(label).frame(false))
                                    .on_hover_text("Ctrl Z")
                                    .clicked()
                                {
                                    undo = Some(i);
                                }
                            }
                            let close = icons::show(ui, Icon::Close, 16.0, t.p.fg_inverse)
                                .interact(Sense::click())
                                .on_hover_text(lang.text("ปิด", "Dismiss"));
                            if close.clicked() {
                                dismiss = Some(i);
                            }
                        });
                    });
                ui.add_space(space::S2);
            }
        });
    if let Some(i) = undo {
        let toast = app.toasts.remove(i);
        if let Some(u) = toast.undo {
            app.apply_undo(u);
        }
    } else if let Some(i) = dismiss {
        app.toasts.remove(i);
    }
}

// ───────────────────────────── Topbar ─────────────────────────────

pub fn menu_row(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    hint: &str,
    active: bool,
) -> egui::Response {
    let t = Theme::of(ui);
    let height = t.m.control_md;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, label));
    let bg = if active {
        t.p.accent_soft
    } else if response.hovered() || response.has_focus() {
        t.p.bg_hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, t.radius(radius::SM), bg);
    icons::paint_sized(
        ui.painter(),
        icon,
        egui::pos2(rect.left() + 20.0, rect.center().y),
        18.0,
        t.p.fg_default,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 40.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        if active {
            t.strong_font(type_scale::BODY)
        } else {
            t.font(type_scale::BODY)
        },
        t.p.fg_default,
    );
    // Keyboard hints hide in touch density; values such as prices always show.
    let shortcut = hint.starts_with("Alt ");
    if !hint.is_empty() && (t.m.show_shortcut_hints || !shortcut) {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            hint,
            if shortcut {
                t.font(type_scale::MONEY_SM)
            } else {
                t.font(type_scale::MONEY)
            },
            t.p.fg_muted,
        );
    }
    response
}
pub(crate) fn command_field(app: &mut App, ui: &mut egui::Ui, t: Theme, width: f32) {
    let lang = app.lang;
    let height = t.m.control_sm.max(26.0);
    // Too little room beside the tabs: a square search button.
    let room = ui.available_width() - space::S3;
    let compact = room < 160.0;
    let width = if compact { height } else { width.min(room) };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            lang.text("ค้นหาหรือสั่งงาน", "Search or run a command"),
        )
    });
    let r8 = t.radius(radius::MD);
    ui.painter().rect(
        rect,
        r8,
        if response.hovered() {
            t.p.bg_chrome_hover
        } else {
            t.p.bg_chrome_raised
        },
        Stroke::new(1.0, t.p.border_chrome),
        StrokeKind::Inside,
    );
    icons::paint_sized(
        ui.painter(),
        Icon::Search,
        if compact {
            rect.center()
        } else {
            egui::pos2(rect.left() + 20.0, rect.center().y)
        },
        16.0,
        t.p.fg_chrome_muted,
    );
    // The hint and Ctrl K shorten or hide instead of being cut off.
    let long = lang.text("ค้นหาหรือสั่งงาน", "Search or run a command");
    let long_w = ui
        .painter()
        .layout_no_wrap(
            long.to_owned(),
            t.font(type_scale::BODY),
            t.p.fg_chrome_muted,
        )
        .size()
        .x;
    let kbd_room = if t.m.show_shortcut_hints { 70.0 } else { 0.0 };
    let fits_long = 38.0 + long_w + kbd_room + space::S2 <= width;
    let show_kbd = t.m.show_shortcut_hints && 38.0 + 60.0 + kbd_room <= width;
    if !compact {
        ui.painter().with_clip_rect(rect.shrink(2.0)).text(
            egui::pos2(rect.left() + 38.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            if fits_long {
                long
            } else {
                lang.text("ค้นหา", "Search")
            },
            t.font(type_scale::BODY),
            t.p.fg_chrome_muted,
        );
    }
    if show_kbd && !compact {
        let g = ui.painter().layout_no_wrap(
            "Ctrl K".into(),
            t.font(type_scale::MONEY_SM),
            t.p.fg_chrome_muted,
        );
        let r = egui::Rect::from_min_size(
            egui::pos2(
                rect.right() - g.size().x - 20.0,
                rect.center().y - g.size().y / 2.0 - 2.0,
            ),
            g.size() + vec2(8.0, 4.0),
        );
        ui.painter().rect(
            r,
            t.radius(radius::XS),
            t.p.bg_chrome,
            Stroke::new(1.0, t.p.border_chrome),
            StrokeKind::Inside,
        );
        ui.painter()
            .galley(r.min + vec2(4.0, 2.0), g, t.p.fg_chrome_muted);
    }
    w::focus_ring(ui, rect, egui::CornerRadius::same(r8), &response);
    if response.clicked() {
        app.palette.open();
    }
}

// ───────────────────────────── Display preferences ─────────────────────────────

pub(crate) fn display_popover(app: &mut App, ctx: &egui::Context) {
    if !app.display_open {
        return;
    }
    let t = Theme::current(ctx);
    let lang = app.lang;
    let mut open = true;
    let response = egui::Area::new(egui::Id::new("display-prefs"))
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::RIGHT_BOTTOM,
            vec2(-space::S3, -(t.m.statusbar_h + space::S1)),
        )
        .show(ctx, |ui| {
            t.raised(2)
                .inner_margin(egui::Margin::same(space::S4 as i8))
                .show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.spacing_mut().item_spacing.y = space::S2;
                    ui.horizontal(|ui| {
                        w::title(ui, lang.text("การแสดงผล", "Display"));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if w::icon_button_sized(
                                ui,
                                Icon::Close,
                                lang.text("ปิด", "Close"),
                                Some(32.0),
                                false,
                            )
                            .clicked()
                            {
                                open = false;
                            }
                        });
                    });
                    w::field_label(ui, lang.text("ภาษา", "Language"), None);
                    let mut next = lang;
                    if w::segmented(
                        ui,
                        &mut next,
                        &[(Language::Thai, "ไทย"), (Language::English, "English")],
                    ) {
                        app.set_language(next);
                    }
                    w::field_label(ui, lang.text("สีหลัก", "Accent colour"), None);
                    let mut look = app.look;
                    if w::accent_picker(ui, &mut look.accent, |a| {
                        let (th, en) = ds_theme::accent_name(a);
                        lang.text(th, en).to_owned()
                    }) {
                        app.set_display(look, app.density);
                    }
                    w::field_label(ui, lang.text("ขอบ", "Corners"), None);
                    if w::segmented(
                        ui,
                        &mut look.shape,
                        &[
                            (ds_theme::Shape::Soft, lang.text("โค้ง", "Rounded")),
                            (ds_theme::Shape::Square, lang.text("เหลี่ยม", "Square")),
                        ],
                    ) {
                        app.set_display(look, app.density);
                    }
                    w::field_label(ui, lang.text("ขนาดปุ่มและตัวอักษร", "Density"), None);
                    let mut density = app.density;
                    if w::segmented(
                        ui,
                        &mut density,
                        &[
                            (crate::ui::Density::Compact, lang.text("กะทัดรัด", "Compact")),
                            (crate::ui::Density::Touch, lang.text("จอสัมผัส", "Touch")),
                        ],
                    ) {
                        app.set_display(app.look, density);
                    }
                    ui.horizontal(|ui| {
                        w::field_label(ui, lang.text("เต็มจอ (F11)", "Full screen (F11)"), None);
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let mut full = app.fullscreen;
                            if w::switch(ui, &mut full, lang.text("เต็มจอ", "Full screen")).changed()
                            {
                                app.set_fullscreen(full);
                            }
                        });
                    });
                    w::help_text(
                        ui,
                        lang.text(
                            "จำค่าไว้ในเครื่องนี้ มีผลทันที",
                            "Saved on this device. Applies immediately.",
                        ),
                    );
                });
        })
        .response;
    // The status bar's accent item toggles the popover itself.
    let ui_bottom = ctx.content_rect().bottom() - t.m.statusbar_h;
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| !response.rect.contains(p) && p.y < ui_bottom);
    if !open || clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.display_open = false;
    }
}

// ───────────────────────────── Command palette (Ctrl K) ─────────────────────────────

#[derive(Default)]
pub struct Palette {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    focus: bool,
}
impl Palette {
    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.selected = 0;
        self.focus = true;
    }
}
#[derive(Clone)]
enum Action {
    Go(Page),
    Settings(crate::settings_ui::Section),
    Purchasing,
    NewBill,
    Hold,
    OpenShift,
    Backup,
    ToggleShape,
    ToggleLanguage,
    ToggleDensity,
    ToggleFullscreen,
    AddProduct(Box<pos_domain::Product>),
    Receipt(Box<pos_domain::SaleRecord>),
}
struct Entry {
    group: &'static str,
    icon: Icon,
    label: String,
    detail: String,
    action: Action,
}
fn entries(app: &App) -> Vec<Entry> {
    let lang = app.lang;
    let q = app.palette.query.trim().to_lowercase();
    let matches = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
    let mut out = Vec::new();
    let commands = lang.text("คำสั่ง", "Commands");
    let pages = lang.text("ไปที่", "Go to");
    for (i, (page, icon, th, en)) in PAGES.iter().enumerate() {
        if matches(th) || matches(en) {
            out.push(Entry {
                group: pages,
                icon: *icon,
                label: lang.text(th, en).into(),
                detail: format!("Alt {}", i + 1),
                action: Action::Go(*page),
            });
        }
    }
    // Settings sections only appear when searched, to keep the list short.
    if !q.is_empty() {
        for section in crate::settings_ui::Section::ALL {
            let (th, en) = (
                section.label(Language::Thai),
                section.label(Language::English),
            );
            if matches(th) || matches(en) {
                out.push(Entry {
                    group: pages,
                    icon: Icon::Settings,
                    label: format!(
                        "{} › {}",
                        lang.text("ตั้งค่า", "Settings"),
                        section.label(lang)
                    ),
                    detail: String::new(),
                    action: Action::Settings(section),
                });
            }
        }
    }
    let mut command = |th: &'static str, en: &'static str, icon, action, ok: bool| {
        if ok && (matches(th) || matches(en)) {
            out.push(Entry {
                group: commands,
                icon,
                label: lang.text(th, en).into(),
                detail: String::new(),
                action,
            });
        }
    };
    command(
        "จัดซื้อ / ผู้ขาย / รับของ",
        "Purchasing / suppliers / receiving",
        Icon::Products,
        Action::Purchasing,
        crate::staff_ui::access(app, pos_domain::Permission::Catalog) != pos_domain::Access::Deny
            || crate::staff_ui::access(app, pos_domain::Permission::Stock)
                != pos_domain::Access::Deny,
    );
    command(
        "พักบิล",
        "Hold bill",
        Icon::Pause,
        Action::Hold,
        !app.cart.lines.is_empty(),
    );
    command(
        "เริ่มบิลใหม่",
        "New bill",
        Icon::Plus,
        Action::NewBill,
        app.cart.lines.is_empty(),
    );
    command(
        "เปิดกะ",
        "Open shift",
        Icon::Shift,
        Action::OpenShift,
        app.data.shift.is_none(),
    );
    command(
        "ปิดกะ / นับเงิน",
        "Close shift / count drawer",
        Icon::Shift,
        Action::OpenShift,
        app.data.shift.is_some(),
    );
    command(
        "สำรองข้อมูล",
        "Back up database",
        Icon::Database,
        Action::Backup,
        app.ready,
    );
    command(
        "สลับขอบโค้ง/เหลี่ยม",
        "Toggle rounded/square corners",
        Icon::Sliders,
        Action::ToggleShape,
        true,
    );
    command(
        "เปลี่ยนภาษา English",
        "เปลี่ยนภาษาเป็นไทย",
        Icon::Sliders,
        Action::ToggleLanguage,
        true,
    );
    command(
        "สลับโหมดจอสัมผัส",
        "Toggle touch density",
        Icon::Sliders,
        Action::ToggleDensity,
        true,
    );
    command(
        "เต็มจอ / ออกจากเต็มจอ",
        "Toggle full screen",
        Icon::Sliders,
        Action::ToggleFullscreen,
        true,
    );
    if !q.is_empty() {
        let products = lang.text("สินค้า", "Products");
        for p in app
            .known
            .values()
            .filter(|p| {
                p.active
                    && (matches(&p.name)
                        || matches(&p.sku)
                        || p.barcode.as_deref().is_some_and(matches))
            })
            .take(8)
        {
            out.push(Entry {
                group: products,
                icon: Icon::Products,
                label: p.name.clone(),
                detail: format::money(p.price),
                action: Action::AddProduct(Box::new(p.clone())),
            });
        }
        let bills = lang.text("บิล", "Bills");
        for r in app
            .data
            .sales
            .iter()
            .filter(|r| {
                matches(&format::receipt_number(r.sale.number))
                    || matches(&r.sale.number.to_string())
                    || matches(&r.sale.label)
            })
            .take(6)
        {
            out.push(Entry {
                group: bills,
                icon: Icon::History,
                label: format!("{} {}", format::receipt_number(r.sale.number), r.sale.label),
                detail: format::money(r.sale.total),
                action: Action::Receipt(Box::new(r.clone())),
            });
        }
    }
    out
}
pub fn palette(app: &mut App, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) && !app.onboarding {
        if app.palette.visible {
            app.palette.visible = false;
        } else {
            app.palette.open();
        }
    }
    if !app.palette.visible {
        return;
    }
    let t = Theme::current(ctx);
    let lang = app.lang;
    let list = entries(app);
    if app.palette.selected >= list.len() {
        app.palette.selected = list.len().saturating_sub(1);
    }
    let (down, up, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if down && !list.is_empty() {
        app.palette.selected = (app.palette.selected + 1) % list.len();
    }
    if up && !list.is_empty() {
        app.palette.selected = (app.palette.selected + list.len() - 1) % list.len();
    }
    let mut chosen = enter.then_some(app.palette.selected);
    let modal = egui::Modal::new(egui::Id::new("command-palette"))
        .backdrop_color(t.p.overlay_scrim)
        .area(
            egui::Modal::default_area(egui::Id::new("command-palette"))
                .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 72.0)),
        )
        .frame(
            t.raised(3)
                .inner_margin(egui::Margin::same(space::S3 as i8)),
        )
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).min(560.0));
            let id = egui::Id::new("command-query");
            let response = w::search_input(
                ui,
                &mut app.palette.query,
                lang.text("ค้นหาสินค้า บิล หรือคำสั่ง", "Search products, bills or commands"),
                Some("Esc"),
                id,
            );
            if app.palette.focus {
                response.request_focus();
                app.palette.focus = false;
            }
            if response.changed() {
                app.palette.selected = 0;
            }
            ui.add_space(space::S2);
            let max_h = (ctx.content_rect().height() - 260.0).clamp(160.0, 420.0);
            let groups = list
                .iter()
                .map(|e| e.group)
                .collect::<std::collections::BTreeSet<_>>()
                .len();
            let wanted = list.len() as f32 * t.m.control_md + groups as f32 * 28.0;
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .min_scrolled_height(wanted.clamp(120.0, max_h))
                .show(ui, |ui| {
                    if list.is_empty() {
                        w::state_view(
                            ui,
                            Icon::Search,
                            &format!(
                                "{} “{}”",
                                lang.text("ไม่พบ", "No match for"),
                                app.palette.query.trim()
                            ),
                            lang.text(
                                "ลองชื่อสินค้า เลขบิล หรือชื่อหน้า",
                                "Try a product, receipt number or page name.",
                            ),
                        );
                    }
                    let mut group = "";
                    for (i, e) in list.iter().enumerate() {
                        if e.group != group {
                            group = e.group;
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                w::eyebrow(ui, group);
                            });
                        }
                        let r =
                            menu_row(ui, e.icon, &e.label, &e.detail, i == app.palette.selected);
                        if i == app.palette.selected && (up || down) {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() {
                            chosen = Some(i);
                        }
                    }
                });
        });
    if modal.should_close() {
        app.palette.visible = false;
        return;
    }
    if let Some(i) = chosen
        && let Some(entry) = list.into_iter().nth(i)
    {
        app.palette.visible = false;
        run(app, entry.action);
    }
}
fn run(app: &mut App, action: Action) {
    match action {
        Action::Go(page) => app.navigate(page),
        Action::Settings(section) => {
            app.settings_section = section;
            app.navigate(Page::Settings);
        }
        Action::Purchasing => {
            if app.catalog.dirty() {
                app.error = Some(
                    app.lang
                        .text(
                            "บันทึกหรือทิ้งร่างสินค้าก่อนเปิดจัดซื้อ",
                            "Save or discard the product draft before opening Purchasing.",
                        )
                        .into(),
                );
                return;
            }
            app.navigate(Page::Products);
            if app.page == Page::Products {
                app.purchasing.open = true;
                app.categories.open = false;
                app.option_sets.open = false;
                app.ingredients.open = false;
                app.catalog.editor = None;
                app.send(Command::LoadPurchases(app.purchasing.offset));
            }
        }
        Action::NewBill => app.navigate(Page::Sell),
        Action::Hold => app.hold_cart(),
        Action::OpenShift => {
            app.drawer.selected = None;
            app.navigate(Page::Shift);
        }
        Action::Backup => app.backup(),
        Action::ToggleShape => {
            let mut look = app.look;
            look.shape = match look.shape {
                ds_theme::Shape::Soft => ds_theme::Shape::Square,
                ds_theme::Shape::Square => ds_theme::Shape::Soft,
            };
            app.set_display(look, app.density);
        }
        Action::ToggleLanguage => app.set_language(match app.lang {
            Language::Thai => Language::English,
            Language::English => Language::Thai,
        }),
        Action::ToggleDensity => app.set_display(
            app.look,
            match app.density {
                crate::ui::Density::Compact => crate::ui::Density::Touch,
                crate::ui::Density::Touch => crate::ui::Density::Compact,
            },
        ),
        Action::ToggleFullscreen => app.set_fullscreen(!app.fullscreen),
        Action::AddProduct(p) => {
            if app.page != Page::Sell {
                app.navigate(Page::Sell);
            }
            app.add(*p);
        }
        Action::Receipt(r) => app.receipt = Some(*r),
    }
}

/// Alt 1–7 workspace keys, Ctrl Z undo. Runs before screens consume keys.
pub fn global_keys(app: &mut App, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
        app.set_fullscreen(!app.fullscreen);
    }
    if app.onboarding || app.palette.visible {
        return;
    }
    if matches!(app.page, Page::Sell | Page::Tables | Page::Home)
        && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::N))
    {
        crate::chrome::new_bill(app);
    }
    let keys = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    for (i, key) in keys.into_iter().enumerate() {
        let hit = ctx.input_mut(|inp| {
            let hit = inp.consume_key(Modifiers::ALT, key);
            if hit {
                // macOS turns Option+digit into a character (e.g. "¡"); drop it.
                inp.events.retain(|e| !matches!(e, egui::Event::Text(_)));
            }
            hit
        });
        if hit {
            app.navigate(PAGES[i].0);
        }
    }
    // The Sell search keeps focus for scanners; while it is empty, Ctrl Z
    // means "undo the last action" rather than text undo.
    let focused = ctx.memory(|m| m.focused());
    let idle_search = focused == Some(crate::sale_ui::search_id()) && app.search.is_empty();
    let text_focused = focused.is_some() && ctx.egui_wants_keyboard_input() && !idle_search;
    if !text_focused && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
        app.undo_last();
    }
}
