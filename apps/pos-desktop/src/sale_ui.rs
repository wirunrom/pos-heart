//! Sell workspace (design/docs/components/ScreenSell.md): cart + payment left,
//! categories and product tiles right (or the table floor, once tables exist). Keyboard: F2 / search, ↑↓ lines, + − qty,
//! F3 qty, F6 note, Del remove (undo), F4 hold, F9 take payment.
use crate::{
    app::{App, Page},
    shell::menu_row,
    ui::{Icon, Theme, format, icons, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Id, Key, Modifiers, Rect, RichText, Sense, Stroke,
    StrokeKind, pos2, vec2,
};
use pos_domain::{Cart, Money, PaymentMethod, PaymentRequest, Product};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CartTab {
    #[default]
    Current,
    Held,
}
/// What the right side of Sell shows. Tables appears once tables exist.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    #[default]
    Products,
    Tables,
}
#[derive(Default)]
pub struct SellState {
    pub tab: CartTab,
    pub pane: Pane,
    /// Choosing a free table for the open bill on the floor.
    pub moving: bool,
    /// Table bill whose payment section is open; table bills otherwise show
    /// a short ordering footer so more dishes fit.
    pub paying: Option<uuid::Uuid>,
    pub category: Option<String>,
    pub current: Option<usize>,
    pub flash: Option<(usize, Instant)>,
    pub scroll_to_current: bool,
    pub focus_search: bool,
    pub note_edit: Option<usize>,
    pub qty_edit: Option<(usize, String)>,
    pub editing_label: bool,
    pub confirm_large_cash: bool,
    pub large_cash_ok: bool,
    pub focus_cash: bool,
    pub hold_requested: bool,
    pub pay_requested: bool,
    pub remove_requested: Option<usize>,
    pub sheet: Option<OptionSheet>,
    pub edit_requested: Option<usize>,
    pub discount: Option<crate::discount_ui::DiscountEditor>,
    pub discount_requested: Option<Option<usize>>,
    pub kitchen_requested: bool,
}
/// Options sheet for adding a product with options, or editing a line's options.
pub struct OptionSheet {
    product: Product,
    chosen: Vec<pos_domain::LineOption>,
    quantity: u32,
    note: String,
    edit: Option<usize>,
}
impl OptionSheet {
    pub fn new(product: Product, edit: Option<(usize, pos_domain::CartLine)>) -> Self {
        match edit {
            Some((index, line)) => Self {
                product,
                chosen: line.options,
                quantity: line.quantity,
                note: line.note,
                edit: Some(index),
            },
            None => {
                // Required single-choice groups start on their first choice.
                let chosen = product
                    .options
                    .iter()
                    .filter(|g| g.required && !g.multiple)
                    .filter_map(|g| {
                        g.choices.first().map(|c| pos_domain::LineOption {
                            group_id: g.id,
                            choice_id: c.id,
                        })
                    })
                    .collect();
                Self {
                    product,
                    chosen,
                    quantity: 1,
                    note: String::new(),
                    edit: None,
                }
            }
        }
    }
}
pub fn search_id() -> Id {
    Id::new("sell-search")
}
fn cash_id() -> Id {
    Id::new("sell-cash")
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    keys(app, ui.ctx());
    let full = ui.available_rect_before_wrap();
    let cart_w = t.m.cart_width(full.width() + 20.0);
    let cart_rect = Rect::from_min_size(full.min, vec2(cart_w, full.height()));
    let right_rect = Rect::from_min_max(pos2(cart_rect.right() + 10.0, full.top()), full.max);
    ui.allocate_rect(full, Sense::hover());
    let mut cart_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(cart_rect)
            .layout(egui::Layout::top_down(Align::Min)),
    );
    t.surface()
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(&mut cart_ui, |ui| {
            ui.set_min_size(cart_rect.size() - vec2(24.0, 24.0));
            ui.set_max_size(cart_rect.size() - vec2(24.0, 24.0));
            cart_panel(app, ui);
        });
    let mut right_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(right_rect)
            .layout(egui::Layout::top_down(Align::Min)),
    );
    t.surface()
        .inner_margin(egui::Margin::same(space::S2 as i8))
        .show(&mut right_ui, |ui| {
            ui.set_min_size(right_rect.size() - vec2(16.0, 16.0));
            ui.set_max_size(right_rect.size() - vec2(16.0, 16.0));
            let tables = !crate::tables_ui::active(app).is_empty();
            if !tables {
                app.sell.pane = Pane::Products;
                app.sell.moving = false;
            }
            if tables {
                pane_tabs(app, ui);
            }
            if app.sell.pane == Pane::Tables {
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::top_down(Align::Min),
                    |ui| crate::tables_ui::floor(app, ui),
                );
                return;
            }
            ui.horizontal_top(|ui| {
                let nav_w = if right_rect.width() < 700.0 {
                    140.0
                } else {
                    172.0
                };
                ui.allocate_ui_with_layout(
                    vec2(nav_w, ui.available_height()),
                    egui::Layout::top_down(Align::Min),
                    |ui| categories(app, ui),
                );
                ui.add_space(space::S2);
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::top_down(Align::Min),
                    |ui| products(app, ui),
                );
            });
        });
    // A pay request the footer did not handle (no bill, Held tab) expires.
    app.sell.pay_requested = false;
}

/// Products / Tables switch above the right side of Sell.
fn pane_tabs(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let busy = crate::tables_ui::busy_count(app);
    let before = app.sell.pane;
    let rect = ui
        .horizontal(|ui| {
            ui.add_space(space::S2);
            w::tabs(
                ui,
                &mut app.sell.pane,
                &[
                    (Pane::Products, lang.text("สินค้า", "Products"), None),
                    (Pane::Tables, lang.text("โต๊ะ", "Tables"), Some(busy)),
                ],
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(t.muted("Alt 9", type_scale::CAPTION))
                    .on_hover_text(lang.text("สลับสินค้า/โต๊ะ", "Switch products / tables"));
            });
        })
        .response
        .rect;
    ui.painter().line_segment(
        [
            pos2(rect.left(), rect.bottom() + 0.5),
            pos2(rect.right(), rect.bottom() + 0.5),
        ],
        Stroke::new(1.0, t.p.border_default),
    );
    ui.add_space(space::S2);
    if before != app.sell.pane {
        app.sell.moving = false;
    }
}

// ───────────────────────────── Keyboard ─────────────────────────────

fn keys(app: &mut App, ctx: &egui::Context) {
    let overlay = app.payment_review.is_some()
        || app.sell.sheet.is_some()
        || app.sell.discount.is_some()
        || app.palette.visible
        || app.held_delete.is_some()
        || app.sell.confirm_large_cash
        || app.receipt.is_some();
    if overlay {
        return;
    }
    let focused = ctx.memory(|m| m.focused());
    let search_focused = focused == Some(search_id());
    let typing_elsewhere = focused.is_some() && !search_focused;
    let search_empty = app.search.is_empty();
    let n = app.cart.lines.len();
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::F2) {
            app.sell.focus_search = true;
        }
        if !typing_elsewhere && !search_focused && i.consume_key(Modifiers::NONE, Key::Slash) {
            app.sell.focus_search = true;
        }
        if i.consume_key(Modifiers::NONE, Key::F10) && app.sell.tab == CartTab::Current {
            app.sell.kitchen_requested = true;
        }
        if i.consume_key(Modifiers::NONE, Key::F4) {
            app.sell.hold_requested = true;
        }
        if app.sell.paying.is_some() && i.consume_key(Modifiers::NONE, Key::Escape) {
            app.sell.paying = None;
        }
        // F9 only means "pay" on the current bill; elsewhere it is ignored.
        if i.consume_key(Modifiers::NONE, Key::F9) && app.sell.tab == CartTab::Current {
            app.sell.pay_requested = true;
        }
        if typing_elsewhere || n == 0 || app.sell.tab != CartTab::Current {
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::F8) {
            app.sell.discount_requested = Some(None);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
            app.sell.current = Some(app.sell.current.map_or(0, |c| (c + 1).min(n - 1)));
            app.sell.scroll_to_current = true;
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            app.sell.current = Some(app.sell.current.map_or(n - 1, |c| c.saturating_sub(1)));
            app.sell.scroll_to_current = true;
        }
        let Some(current) = app.sell.current.filter(|c| *c < n) else {
            return;
        };
        if search_empty {
            let plus = i.consume_key(Modifiers::NONE, Key::Plus)
                || i.consume_key(Modifiers::SHIFT, Key::Equals)
                || i.consume_key(Modifiers::NONE, Key::Equals);
            if plus {
                let q = &mut app.cart.lines[current].quantity;
                *q = q.saturating_add(1).min(999_999);
            }
            let minus = i.consume_key(Modifiers::NONE, Key::Minus);
            if minus {
                let q = &mut app.cart.lines[current].quantity;
                if *q > 1 {
                    *q -= 1;
                }
            }
            // The focused search box must not also receive the typed character.
            if plus || minus {
                i.events.retain(
                    |e| !matches!(e, egui::Event::Text(t) if matches!(t.as_str(), "+" | "=" | "-")),
                );
            }
            if i.consume_key(Modifiers::NONE, Key::Delete) {
                app.sell.remove_requested = Some(current);
            }
        }
        if i.consume_key(Modifiers::NONE, Key::F3) {
            app.sell.qty_edit = Some((current, app.cart.lines[current].quantity.to_string()));
        }
        if i.consume_key(Modifiers::NONE, Key::F6) {
            app.sell.edit_requested = Some(current);
        }
        if i.consume_key(Modifiers::NONE, Key::F7) {
            app.sell.discount_requested = Some(Some(current));
        }
    });
    if let Some(target) = app.sell.discount_requested.take() {
        crate::discount_ui::open(app, target);
    }
    if let Some(index) = app.sell.remove_requested.take() {
        app.remove_line(index);
    }
    if let Some(index) = app.sell.edit_requested.take() {
        edit_line(app, index);
    }
    if std::mem::take(&mut app.sell.hold_requested) {
        app.hold_cart();
    }
    // Keep the search box focused for barcode scanners when nothing else is.
    if focused.is_none() && !app.display_open {
        app.sell.focus_search = true;
    }
}

// ───────────────────────────── Cart ─────────────────────────────

fn cart_panel(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    // Table bills live on the floor, not in Held.
    let held: Vec<Cart> = app
        .held_bills()
        .into_iter()
        .filter(|c| c.table.is_none())
        .collect();
    ui.spacing_mut().item_spacing.y = space::S2;
    let tabs_rect = ui
        .scope(|ui| {
            w::tabs(
                ui,
                &mut app.sell.tab,
                &[
                    (CartTab::Current, lang.text("บิลปัจจุบัน", "Current bill"), None),
                    (CartTab::Held, lang.text("พักไว้", "Held"), Some(held.len())),
                ],
            );
        })
        .response
        .rect;
    ui.painter().line_segment(
        [
            pos2(tabs_rect.left() - 4.0, tabs_rect.bottom() + 0.5),
            pos2(
                tabs_rect.left() + ui.available_width() + 4.0,
                tabs_rect.bottom() + 0.5,
            ),
        ],
        Stroke::new(1.0, t.p.border_default),
    );
    match app.sell.tab {
        CartTab::Current => current_bill(app, ui),
        CartTab::Held => held_list(app, ui, held),
    }
}

fn current_bill(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    // Header: bill name + actions.
    ui.horizontal(|ui| {
        let actions_w = t.m.control_md * 3.0 + space::S2 * 2.0;
        ui.allocate_ui_with_layout(
            vec2(ui.available_width() - actions_w - space::S2, t.m.control_lg),
            egui::Layout::top_down(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                if app.cart.table.is_some() {
                    guests_caption(app, ui);
                } else {
                    ui.label(t.muted(lang.text("ชื่อบิล", "Bill name"), type_scale::CAPTION));
                }
                if app.sell.editing_label {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut app.cart.label)
                            .id(Id::new("bill-label"))
                            .font(t.strong_font(type_scale::TITLE))
                            .hint_text(
                                lang.text("โต๊ะ / ลูกค้า / กลับบ้าน", "Table / customer / takeaway"),
                            )
                            .desired_width(f32::INFINITY)
                            .char_limit(160),
                    );
                    if !r.has_focus() && !r.gained_focus() && r.lost_focus() {
                        app.sell.editing_label = false;
                    }
                    if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
                        r.request_focus();
                    }
                    if ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
                        app.sell.editing_label = false;
                    }
                } else {
                    let title = if app.cart.label.is_empty() {
                        t.muted(
                            lang.text("ไม่มีชื่อ · แตะเพื่อตั้ง", "Untitled · tap to name"),
                            type_scale::TITLE,
                        )
                    } else {
                        t.text(&app.cart.label, type_scale::TITLE)
                    };
                    let r = ui
                        .add(egui::Label::new(title).truncate().sense(Sense::click()))
                        .on_hover_text(lang.text("ตั้งชื่อบิล", "Name this bill"));
                    if r.clicked() && !app.busy {
                        app.sell.editing_label = true;
                    }
                }
            },
        );
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = space::S2;
            let has_lines = !app.cart.lines.is_empty() && !app.busy;
            ui.add_enabled_ui(has_lines, |ui| {
                let more =
                    w::icon_button_sized(ui, Icon::More, lang.text("เพิ่มเติม", "More"), None, false);
                egui::Popup::menu(&more)
                    .frame(
                        t.raised(2)
                            .inner_margin(egui::Margin::same(space::S1 as i8)),
                    )
                    .width(220.0)
                    .show(|ui| {
                        if menu_row(
                            ui,
                            Icon::Pencil,
                            lang.text("ตั้งชื่อบิล", "Name bill"),
                            "",
                            false,
                        )
                        .clicked()
                        {
                            app.sell.editing_label = true;
                        }
                        if !crate::tables_ui::active(app).is_empty() {
                            let label = if app.cart.table.is_some() {
                                lang.text("ย้ายโต๊ะ", "Move to another table")
                            } else {
                                lang.text("ให้บิลนี้นั่งโต๊ะ", "Seat this bill at a table")
                            };
                            if menu_row(ui, Icon::Sell, label, "", false).clicked() {
                                app.sell.moving = true;
                                app.sell.pane = Pane::Tables;
                            }
                        }
                        if menu_row(
                            ui,
                            Icon::Tag,
                            lang.text("ส่วนลดทั้งบิล", "Bill discount"),
                            "F8",
                            false,
                        )
                        .clicked()
                        {
                            app.sell.discount_requested = Some(None);
                        }
                        if menu_row(
                            ui,
                            Icon::Printer,
                            lang.text("ดูใบแจ้งยอด (เช็กบิล)", "Check bill"),
                            "",
                            false,
                        )
                        .clicked()
                        {
                            app.preview_check_bill();
                        }
                        if app.data.pricing.service_bp > 0 {
                            let label = if app.cart.service_charge {
                                lang.text(
                                    "ไม่คิด service charge (กลับบ้าน)",
                                    "No service charge (takeaway)",
                                )
                            } else {
                                lang.text("คิด service charge", "Add service charge")
                            };
                            if menu_row(ui, Icon::Tag, label, "", false).clicked() {
                                app.cart.service_charge = !app.cart.service_charge;
                                app.payment_review = None;
                            }
                        }
                        if menu_row(ui, Icon::Trash, lang.text("ล้างบิล", "Clear bill"), "", false)
                            .clicked()
                        {
                            app.clear_cart();
                        }
                    });
                if w::icon_button(ui, Icon::Pause, lang.text("พักบิล (F4)", "Hold bill (F4)"))
                    .clicked()
                {
                    app.hold_cart();
                }
                let has_current = app.sell.current.is_some_and(|c| c < app.cart.lines.len());
                ui.add_enabled_ui(has_current, |ui| {
                    if w::icon_button(
                        ui,
                        Icon::Note,
                        lang.text("หมายเหตุบรรทัด (F6)", "Line note (F6)"),
                    )
                    .clicked()
                    {
                        app.sell.note_edit = app.sell.current;
                    }
                });
            });
        });
    });
    // Footer is a bottom panel so totals and the pay button never scroll away.
    egui::Panel::bottom("cart-footer")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: space::S2 as i8,
            bottom: 0,
        }))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::S2;
            footer(app, ui);
        });
    egui::CentralPanel::no_frame().show(ui, |ui| {
        let out = egui::ScrollArea::vertical()
            .id_salt("cart-lines")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if app.cart.lines.is_empty() {
                    if let Some(sale) = app.last_sale.clone() {
                        paid_state(app, ui, &sale);
                    } else if app.cart.table.is_some() {
                        w::state_view(
                            ui,
                            Icon::Sell,
                            lang.text("โต๊ะนี้ยังไม่ได้สั่ง", "Nothing ordered yet"),
                            lang.text(
                                "แตะสินค้าทางขวาเพื่อสั่ง แล้วกดส่งครัว (F10)",
                                "Tap products on the right, then send to the kitchen (F10).",
                            ),
                        );
                    } else if !crate::tables_ui::active(app).is_empty() {
                        w::state_view(
                            ui,
                            Icon::Sell,
                            lang.text("พร้อมเริ่มบิลใหม่", "Ready for a new bill"),
                            lang.text(
                                "เลือกโต๊ะในแท็บโต๊ะ หรือแตะสินค้าเพื่อขายหน้าร้าน/กลับบ้าน",
                                "Pick a table in the Tables tab, or tap products for a counter / takeaway sale.",
                            ),
                        );
                    } else {
                        w::state_view(
                            ui,
                            Icon::Sell,
                            lang.text("พร้อมเริ่มบิลใหม่", "Ready for a new bill"),
                            lang.text(
                                "แตะสินค้าทางขวา หรือสแกน/พิมพ์รหัสแล้วกด Enter",
                                "Tap a product on the right, or scan/type a code and press Enter.",
                            ),
                        );
                    }
                }
                let mut i = 0;
                while i < app.cart.lines.len() {
                    cart_line(app, ui, i);
                    i += 1;
                }
            });
        w::scroll_fade(ui, &out, Theme::of(ui).p.bg_surface);
    });
}

/// "Table · 4 guests ▾" above a table bill's name; tap to set the guests.
fn guests_caption(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let guests = if app.cart.guests > 0 {
        format!("{} {}", app.cart.guests, lang.text("คน", "guests"))
    } else {
        lang.text("ระบุจำนวนคน", "set guests").to_owned()
    };
    let r = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.label(t.muted(
                format!("{} · {guests}", lang.text("โต๊ะ", "Table")),
                type_scale::CAPTION,
            ));
            icons::show(ui, Icon::ChevronDown, 12.0, t.p.fg_muted);
        })
        .response
        .interact(Sense::click())
        .on_hover_text(lang.text("จำนวนคน", "Guests"));
    egui::Popup::menu(&r)
        .frame(
            t.raised(2)
                .inner_margin(egui::Margin::same(space::S2 as i8)),
        )
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.label(t.text(lang.text("จำนวนคน", "Guests"), type_scale::BODY_STRONG));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = space::S2;
                let n = app.cart.guests;
                if ui
                    .add_enabled_ui(n > 0, |ui| {
                        w::icon_button(ui, Icon::Minus, lang.text("ลดจำนวนคน", "Fewer guests"))
                    })
                    .inner
                    .clicked()
                {
                    app.cart.guests = n.saturating_sub(1);
                }
                ui.label(t.text(n.to_string(), type_scale::MONEY));
                if ui
                    .add_enabled_ui(n < 99, |ui| {
                        w::icon_button(ui, Icon::Plus, lang.text("เพิ่มจำนวนคน", "More guests"))
                    })
                    .inner
                    .clicked()
                {
                    app.cart.guests = (n + 1).min(99);
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = space::S1;
                for n in 1..=6 {
                    if w::button(n.to_string())
                        .size(w::Size::Sm)
                        .ghost()
                        .show(ui)
                        .clicked()
                    {
                        app.cart.guests = n;
                        ui.close();
                    }
                }
            });
        });
}

fn paid_state(app: &mut App, ui: &mut egui::Ui, sale: &pos_domain::Sale) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.add_space(space::S6);
    ui.vertical_centered(|ui| {
        icons::show(ui, Icon::CheckCircle, 32.0, t.p.success_fg);
        ui.label(
            t.text(
                lang.text("รับชำระเรียบร้อย", "Payment complete"),
                type_scale::TITLE,
            )
            .color(t.p.success_fg),
        );
        ui.label(t.muted(
            format!(
                "{} {} · {}",
                lang.text("บิล", "Receipt"),
                format::receipt_number(sale.number),
                lang.payment_method(sale.payment_method)
            ),
            type_scale::BODY,
        ));
        ui.add_space(space::S3);
        if sale.payment_method == PaymentMethod::Cash {
            ui.label(t.muted(lang.text("เงินทอน", "Change due"), type_scale::LABEL));
            ui.label(
                RichText::new(format::baht(sale.change)).font(t.strong_font(type_scale::MONEY_XL)),
            );
        } else {
            ui.label(t.muted(lang.text("ยอดชำระ", "Amount paid"), type_scale::LABEL));
            ui.label(
                RichText::new(format::baht(sale.total)).font(t.strong_font(type_scale::MONEY_XL)),
            );
        }
        ui.add_space(space::S3);
        if w::button(lang.text("ดูใบเสร็จ", "View receipt"))
            .icon(Icon::History)
            .show(ui)
            .clicked()
        {
            app.receipt = Some(pos_domain::SaleRecord {
                sale: sale.clone(),
                cancellation: None,
                refunds: Vec::new(),
                tax_invoice: None,
            });
        }
        if !(app.print.enabled() && app.print.settings.auto_print) {
            let record = pos_domain::SaleRecord {
                sale: sale.clone(),
                cancellation: None,
                refunds: Vec::new(),
                tax_invoice: None,
            };
            crate::print_ui::receipt_button(app, ui, &record, false);
        }
        ui.add_space(space::S2);
        w::help_text(
            ui,
            lang.text(
                "เพิ่มสินค้าเพื่อเริ่มบิลใหม่",
                "Add a product to start the next bill.",
            ),
        );
    });
}

fn cart_line(app: &mut App, ui: &mut egui::Ui, index: usize) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let line = app.cart.lines[index].clone();
    let product = app.known.get(&line.product_id).cloned();
    let current = app.sell.current == Some(index);
    let name = product.as_ref().map_or(
        lang.text("ไม่พบสินค้า", "Unavailable item").to_owned(),
        |p| p.name.clone(),
    );
    let gross = app.price(&line);
    let line_off = line.discount.zip(gross).and_then(|(d, g)| d.off(g).ok());
    let amount = gross
        .map(|g| Money::from_satang(g.satang() - line_off.map_or(0, |o| o.satang())).unwrap_or(g))
        .map_or("—".into(), format::money);
    let pad = if t.touch() { space::S3 } else { space::S2 };
    let width = ui.available_width();
    let qty_w = 34.0;
    let amount_font = t.font(type_scale::MONEY);
    let amount_w = w::text_width(ui, &amount, amount_font.clone());
    let name_w = (width - pad * 2.0 - qty_w - space::S3 * 2.0 - amount_w).max(60.0);
    let (name_g, _) = crate::ui::text::wrapped(
        ui.painter(),
        &name,
        t.font(type_scale::BODY),
        t.p.fg_default,
        name_w,
        0,
    );
    // Secondary line: options and note (accent), unit price, availability.
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = name_w;
    let mut push = |text: String, color: Color32, font: egui::FontId| {
        let gap = if job.text.is_empty() { 0.0 } else { 8.0 };
        job.append(&text, gap, egui::TextFormat::simple(font, color));
    };
    let accent = t.strong_font(type_scale::CAPTION);
    if let Some(p) = &product
        && let Ok((_, options)) = pos_domain::price_with_options(p.price, &p.options, &line.options)
    {
        for o in options {
            // Non-breaking space keeps an option's price with its name.
            let text = if o.price.satang() > 0 {
                format!("+{}\u{a0}+{}", o.name, format::money(o.price))
            } else {
                o.name
            };
            push(text, t.p.fg_accent, accent.clone());
        }
    } else if !line.options.is_empty() {
        push(
            lang.text(
                "ตัวเลือกเปลี่ยน · เลือกใหม่ (F6)",
                "Options changed · choose again (F6)",
            )
            .into(),
            t.p.danger_fg,
            accent.clone(),
        );
    }
    if !line.note.is_empty() {
        push(line.note.clone(), t.p.fg_accent, accent.clone());
    }
    if line.quantity > 1
        && let Some(unit) = product
            .as_ref()
            .and_then(|p| p.unit_price(&line.options).ok())
    {
        push(
            format!("× {}", format::money(unit)),
            t.p.fg_muted,
            t.font(type_scale::MONEY_SM),
        );
    }
    if let Some(rule) = line.discount {
        push(
            match line_off {
                Some(off) => format!(
                    "{}\u{a0}{}",
                    crate::discount_ui::label(lang, rule),
                    format::minus(off)
                ),
                None => lang
                    .text(
                        "ส่วนลดเกินราคา · แก้ (F7)",
                        "Discount exceeds price · fix (F7)",
                    )
                    .into(),
            },
            t.p.danger_fg,
            t.font(type_scale::CAPTION),
        );
    }
    if let Some(p) = &product
        && !p.on_sale
    {
        push(
            lang.text("ปิดขาย", "Not for sale").into(),
            t.p.danger_fg,
            accent,
        );
    }
    let sub = (!job.text.is_empty()).then(|| ui.painter().layout_job(job));
    let sub_h = sub.as_ref().map_or(0.0, |g| g.size().y + 2.0);
    let row_h = (name_g.size().y.max(26.0) + sub_h + pad * 2.0).max(t.m.row_height);
    let (rect, response) = ui.allocate_exact_size(vec2(width, row_h), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            true,
            current,
            format!("{} {} {}", line.quantity, name, amount),
        )
    });
    let flash = app
        .sell
        .flash
        .filter(|(i, at)| *i == index && at.elapsed().as_secs_f32() < 0.6)
        .map(|(_, at)| 1.0 - at.elapsed().as_secs_f32() / 0.6);
    if flash.is_some() {
        ui.ctx().request_repaint();
    }
    let bg = if current {
        t.p.bg_selected
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
    if let Some(f) = flash {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(6),
            t.p.accent_soft.gamma_multiply(f),
        );
    }
    if index > 0 && !current && app.sell.current != Some(index - 1) {
        ui.painter().line_segment(
            [
                rect.left_top() + vec2(pad, 0.0),
                rect.right_top() - vec2(pad, 0.0),
            ],
            Stroke::new(1.0, t.p.border_subtle),
        );
    }
    let qty_rect = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(qty_w, 26.0));
    ui.painter().rect_filled(
        qty_rect,
        CornerRadius::same(6),
        if current {
            t.p.bg_inverse
        } else {
            t.p.bg_sunken
        },
    );
    ui.painter().text(
        qty_rect.center(),
        Align2::CENTER_CENTER,
        format::count(u64::from(line.quantity)),
        t.strong_font(type_scale::MONEY_SM),
        if current {
            t.p.fg_inverse
        } else {
            t.p.fg_default
        },
    );
    let name_pos = pos2(qty_rect.right() + space::S3, rect.top() + pad + 2.0);
    let name_h = name_g.size().y;
    ui.painter().galley(name_pos, name_g, t.p.fg_default);
    ui.painter().text(
        pos2(rect.right() - pad, rect.top() + pad + 2.0),
        Align2::RIGHT_TOP,
        &amount,
        amount_font,
        t.p.fg_default,
    );
    if let Some(g) = sub {
        let pos = pos2(name_pos.x, name_pos.y + name_h + 2.0);
        ui.painter().galley(pos, g, t.p.fg_muted);
    }
    if response.clicked() {
        app.sell.current = Some(index);
    }
    if current && std::mem::take(&mut app.sell.scroll_to_current) {
        response.scroll_to_me(None);
    }
    if current {
        line_editor(app, ui, index);
    }
}

fn line_editor(app: &mut App, ui: &mut egui::Ui, index: usize) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let side = if t.touch() { t.m.control_md } else { 32.0 };
    ui.add_enabled_ui(!app.busy, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(if t.touch() { space::S3 } else { space::S2 });
            ui.spacing_mut().item_spacing.x = space::S1;
            if w::icon_button_sized(
                ui,
                Icon::Minus,
                lang.text("ลดจำนวน (−)", "Decrease (−)"),
                Some(side),
                true,
            )
            .clicked()
            {
                let q = &mut app.cart.lines[index].quantity;
                if *q > 1 {
                    *q -= 1;
                } else {
                    app.remove_line(index);
                    return;
                }
            }
            let editing = app.sell.qty_edit.as_ref().is_some_and(|(i, _)| *i == index);
            if editing {
                if let Some((_, text)) = &mut app.sell.qty_edit {
                    let r = ui.add(
                        egui::TextEdit::singleline(text)
                            .id(Id::new(("qty", index)))
                            .font(t.strong_font(type_scale::MONEY))
                            .horizontal_align(Align::Center)
                            .vertical_align(Align::Center)
                            .min_size(vec2(56.0, side))
                            .desired_width(56.0)
                            .char_limit(6),
                    );
                    if !r.has_focus() && !r.lost_focus() {
                        r.request_focus();
                    }
                    text.retain(|c| c.is_ascii_digit());
                    let done = r.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter));
                    if done {
                        if let Ok(q) = text.parse::<u32>()
                            && (1..=999_999).contains(&q)
                        {
                            app.cart.lines[index].quantity = q;
                        }
                        app.sell.qty_edit = None;
                    }
                }
            } else {
                let (r, resp) = ui.allocate_exact_size(vec2(56.0, side), Sense::click());
                ui.painter().rect(
                    r,
                    t.control_radius(),
                    t.p.bg_surface,
                    Stroke::new(1.0, t.p.border_control),
                    StrokeKind::Inside,
                );
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    app.cart.lines[index].quantity.to_string(),
                    t.strong_font(type_scale::MONEY),
                    t.p.fg_default,
                );
                if resp
                    .on_hover_text(lang.text("พิมพ์จำนวน (F3)", "Type quantity (F3)"))
                    .clicked()
                {
                    app.sell.qty_edit = Some((index, app.cart.lines[index].quantity.to_string()));
                }
            }
            if w::icon_button_sized(
                ui,
                Icon::Plus,
                lang.text("เพิ่มจำนวน (+)", "Increase (+)"),
                Some(side),
                true,
            )
            .clicked()
            {
                let q = &mut app.cart.lines[index].quantity;
                *q = q.saturating_add(1).min(999_999);
            }
            ui.add_space(space::S2);
            let has_options = app
                .known
                .get(&app.cart.lines[index].product_id)
                .is_some_and(|p| !p.options.is_empty());
            let (icon, label) = if has_options {
                (Icon::Options, lang.text("ตัวเลือก", "Options"))
            } else {
                (Icon::Note, lang.text("หมายเหตุ", "Note"))
            };
            // Narrow carts drop the shortcut hints, then the discount label;
            // F6 / F7 and the tooltips still name them.
            let room = ui.available_width() - side - space::S2;
            let label_w = |ui: &egui::Ui, s: &str| {
                // Small button: text + 16 icon + gap + padding (widgets::Button).
                w::text_width(ui, s, t.strong_font(type_scale::LABEL)) + 16.0 + space::S2 + 20.0
            };
            let hint_w = if t.m.show_shortcut_hints {
                w::text_width(ui, "F6", t.font(type_scale::MONEY_SM)) + 20.0
            } else {
                0.0
            };
            let texts =
                label_w(ui, label) + label_w(ui, lang.text("ส่วนลด", "Discount")) + space::S1 * 3.0;
            let hints = room > texts + hint_w * 2.0;
            let mut edit = w::button(label).icon(icon).ghost().size(w::Size::Sm);
            if hints {
                edit = edit.shortcut("F6");
            }
            if edit.show(ui).clicked() {
                app.sell.edit_requested = Some(index);
            }
            let discount_label = lang.text("ส่วนลด (F7)", "Discount (F7)");
            let discount_text = ui.available_width() - side - space::S2
                > label_w(ui, lang.text("ส่วนลด", "Discount")) + if hints { hint_w } else { 0.0 };
            let clicked = if discount_text {
                let mut b = w::button(lang.text("ส่วนลด", "Discount"))
                    .icon(Icon::Tag)
                    .ghost()
                    .size(w::Size::Sm);
                if hints {
                    b = b.shortcut("F7");
                }
                b.show(ui).on_hover_text(discount_label).clicked()
            } else {
                w::icon_button_sized(ui, Icon::Tag, discount_label, Some(side), false).clicked()
            };
            if clicked {
                app.sell.discount_requested = Some(Some(index));
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if w::icon_button_sized(
                    ui,
                    Icon::Trash,
                    lang.text("นำออก (Del)", "Remove (Del)"),
                    Some(side),
                    false,
                )
                .clicked()
                {
                    app.remove_line(index);
                }
            });
        });
        if app.sell.note_edit == Some(index) && index < app.cart.lines.len() {
            ui.horizontal(|ui| {
                ui.add_space(if t.touch() { space::S3 } else { space::S2 });
                let id = Id::new(("note", index));
                let r = w::text_input(
                    ui,
                    &mut app.cart.lines[index].note,
                    w::FieldOpts {
                        hint: lang.text("เช่น ไม่หวาน / ไม่ใส่ผัก", "e.g. less sweet / no vegetables"),
                        limit: 240,
                        id: Some(id),
                        width: Some(ui.available_width() - space::S2),
                        ..Default::default()
                    },
                );
                if ui.memory(|m| m.focused().is_none()) && !r.lost_focus() {
                    r.request_focus();
                }
                if r.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    app.sell.note_edit = None;
                    app.cart.lines[index].note = app.cart.lines[index].note.trim().to_owned();
                }
            });
        }
    });
    ui.add_space(space::S1);
}

fn cash_short(app: &App) -> Option<Money> {
    let total = app.total().ok()?;
    let cash = format::parse_money(&app.cash).ok()?;
    (cash < total).then(|| Money::from_satang(total.satang() - cash.satang()).ok())?
}
/// "Exact" plus notes rounded up from the total (1,323.50 → 1,400 / 1,500 / 2,000).
pub fn quick_cash(total: Money) -> Vec<Money> {
    let t = total.satang();
    let mut out: Vec<i64> = Vec::new();
    for step in [10_000_i64, 50_000, 100_000] {
        let up = (t / step + 1) * step;
        if up > t && !out.contains(&up) {
            out.push(up);
        }
    }
    let mut extra = 100_000;
    while out.len() < 3 {
        let up = (t / extra + 2) * extra;
        if !out.contains(&up) {
            out.push(up);
        }
        extra *= 2;
    }
    out.sort_unstable();
    out.into_iter()
        .take(3)
        .filter_map(|v| Money::from_satang(v).ok())
        .collect()
}

/// "Send to kitchen (F10)" when the shop has tables and something changed.
fn kitchen_button(app: &mut App, ui: &mut egui::Ui) {
    if app.data.tables.is_empty() {
        return;
    }
    let lang = app.lang;
    let kitchen = |id: uuid::Uuid| app.known.get(&id).is_some_and(|p| p.kitchen);
    let changes = pos_domain::kitchen_changes(&app.cart, kitchen).0;
    let new: i64 = changes.iter().map(|(_, d)| *d).filter(|d| *d > 0).sum();
    let cancel: i64 = changes
        .iter()
        .map(|(_, d)| *d)
        .filter(|d| *d < 0)
        .sum::<i64>()
        .abs();
    let label = match (new, cancel) {
        (0, 0) => lang.text("ส่งครัวแล้ว", "Kitchen up to date").to_owned(),
        (n, 0) => format!(
            "{} · {} {n}",
            lang.text("ส่งครัว", "Send to kitchen"),
            lang.text("ใหม่", "new")
        ),
        (0, c) => format!("{} · {c}", lang.text("ส่งยกเลิกเข้าครัว", "Send cancellation")),
        (n, c) => format!("{} · +{n} / −{c}", lang.text("ส่งครัว", "Send to kitchen")),
    };
    let ready = !changes.is_empty() && !app.busy;
    // Short windows keep bill lines visible with a smaller button.
    let t = Theme::of(ui);
    let short = ui.ctx().content_rect().height() < if t.touch() { 900.0 } else { 640.0 };
    let clicked = w::button(label)
        .icon(Icon::Sell)
        .size(if short { w::Size::Md } else { w::Size::Lg })
        .fill()
        .shortcut("F10")
        .enabled(ready)
        .show(ui)
        .clicked();
    if (clicked || std::mem::take(&mut app.sell.kitchen_requested)) && ready {
        app.send_kitchen();
    }
}
fn footer(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    if app.cart.lines.is_empty() {
        if app.data.shift.is_none() {
            ui.add_space(space::S2);
            shift_gate(app, ui);
        } else {
            ui.add_space(space::S2);
            w::button(lang.text("รับเงิน", "Take payment"))
                .primary()
                .size(w::Size::Xl)
                .fill()
                .shortcut("F9")
                .enabled(false)
                .show(ui);
        }
        app.sell.pay_requested = false;
        return;
    }
    if app.cart.table.is_some() {
        if app.sell.paying != Some(app.cart.id) {
            ordering_footer(app, ui);
            return;
        }
        ui.horizontal(|ui| {
            if w::button(lang.text("กลับไปสั่ง", "Back to ordering"))
                .icon(Icon::ChevronLeft)
                .ghost()
                .size(w::Size::Sm)
                .shortcut("Esc")
                .show(ui)
                .clicked()
            {
                app.sell.paying = None;
            }
        });
        let waiting = crate::tables_ui::unsent(app, &app.cart);
        if waiting > 0 {
            w::inline_error(
                ui,
                &format!(
                    "{} {waiting} {}",
                    lang.text("ยังไม่ได้ส่งครัว", "Not sent to the kitchen:"),
                    lang.text("รายการ", "items")
                ),
            );
        }
    }
    let price = app.bill_price();
    let total = price.as_ref().map(|p| p.total).map_err(|e| *e);
    let units: u64 = app.cart.lines.iter().map(|l| u64::from(l.quantity)).sum();
    let cash = format::parse_money(&app.cash).ok();
    let change = total
        .as_ref()
        .ok()
        .and_then(|t| cash.and_then(|c| t.change_from(c).ok()));
    // Short windows (touch at 1000×700) merge rows so several bill lines stay visible.
    let compact = ui.ctx().content_rect().height() < if t.touch() { 900.0 } else { 640.0 };
    t.sunken().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = space::S1;
        if !compact {
            w::kv(
                ui,
                &format!(
                    "{} · {} {} · {} {}",
                    lang.text("รวมรายการ", "Items total"),
                    app.cart.lines.len(),
                    lang.noun(app.cart.lines.len() as u64, "รายการ", "line", "lines"),
                    format::count(units),
                    lang.noun(units, "ชิ้น", "item", "items")
                ),
                &price.as_ref().map_or("—".into(), |p| format::money(p.subtotal)),
                w::Value::Plain,
            );
        }
        if app.cart.has_discount() {
            let mut label = lang.text("ส่วนลด", "Discount").to_owned();
            if let Some(rule) = app.cart.discount {
                label = format!(
                    "{label} · {} {}",
                    lang.text("ทั้งบิล", "bill"),
                    format::discount_rule(rule)
                );
            }
            if !app.cart.discount_reason.is_empty() {
                label = format!("{label} · {}", app.cart.discount_reason);
            }
            match &price {
                Ok(p) => w::kv(ui, &label, &format::minus(p.discounts()), w::Value::Danger),
                Err(_) => w::inline_error(
                    ui,
                    lang.text(
                        "ส่วนลดมากกว่ายอด · แก้ส่วนลด (F7 / F8)",
                        "A discount is more than the price · fix it (F7 / F8)",
                    ),
                ),
            }
        }
        if let Ok(p) = &price {
            let mut rows: Vec<(String, String)> = Vec::new();
            if p.service_charge.satang() > 0 {
                rows.push((
                    format!(
                        "Service charge {}",
                        format::discount_rule(pos_domain::Discount::Percent(
                            app.data.pricing.service_bp.max(1)
                        ))
                    ),
                    format::money(p.service_charge),
                ));
            } else if app.data.pricing.service_bp > 0 && !app.cart.service_charge {
                rows.push((
                    lang.text("ไม่คิด service charge บิลนี้", "No service charge on this bill")
                        .into(),
                    format::money(Money::ZERO),
                ));
            }
            if p.vat_added.satang() > 0 {
                rows.push((
                    format!(
                        "VAT {}",
                        crate::tax_ui::rate_label(app.data.tax.rate_bp)
                    ),
                    format::money(p.vat_added),
                ));
            }
            if p.rounding != 0 {
                rows.push((
                    lang.text("ปัดเศษเงินสด", "Cash rounding").into(),
                    format::signed(pos_domain::SignedMoney::from_satang(p.rounding)),
                ));
            }
            if compact && rows.len() > 1 {
                // One line on short windows so bill lines stay visible.
                let text: Vec<String> = rows.iter().map(|(l, v)| format!("{l} {v}")).collect();
                ui.label(t.muted(text.join(" · "), type_scale::CAPTION));
            } else {
                for (label, value) in rows {
                    w::kv(ui, &label, &value, w::Value::Plain);
                }
            }
        }
        ui.horizontal(|ui| {
            ui.label(t.text(lang.text("ยอดชำระ", "Amount due"), type_scale::BODY_STRONG));
            if compact {
                ui.label(t.muted(
                    format!("{} {}", format::count(units), lang.noun(units, "ชิ้น", "item", "items")),
                    type_scale::CAPTION,
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(total.as_ref().map_or("—".into(), |m| format::baht(*m))).font(
                        t.strong_font(if compact {
                            type_scale::MONEY_LG
                        } else {
                            type_scale::MONEY_XL
                        }),
                    ),
                );
            });
        });
        let cash_method = app.payment_method == PaymentMethod::Cash;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!app.busy, |ui| {
                let before = app.payment_method;
                w::segmented(
                    ui,
                    &mut app.payment_method,
                    &[
                        (PaymentMethod::Cash, lang.payment_method(PaymentMethod::Cash)),
                        (PaymentMethod::Transfer, lang.payment_method(PaymentMethod::Transfer)),
                        (PaymentMethod::Card, lang.payment_method(PaymentMethod::Card)),
                    ],
                );
                if before != app.payment_method {
                    app.payment_review = None;
                }
            });
        });
        if compact && cash_method {
            // Cash received and change share one row in compact mode.
            ui.horizontal(|ui| {
                let field_w = (ui.available_width() * 0.52).clamp(120.0, 220.0);
                cash_input(app, ui, field_w);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(change.map_or("—".into(), format::money))
                            .font(t.strong_font(type_scale::MONEY_LG)),
                    );
                    ui.label(t.muted(lang.text("ทอน", "Change"), type_scale::LABEL));
                });
            });
        }
        if cash_method && !compact {
            ui.horizontal(|ui| {
                ui.label(t.muted(lang.text("รับเงินสด", "Cash received"), type_scale::BODY));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let field_w = (ui.available_width() * 0.62).clamp(150.0, 220.0);
                    cash_input(app, ui, field_w);
                });
            });
        }
        if cash_method {
            if let Some(short) = cash_short(app) {
                w::inline_error(
                    ui,
                    &format!(
                        "{} {}",
                        lang.text("รับเงินน้อยกว่ายอดชำระ", "Cash is short of the amount due by"),
                        format::money(short)
                    ),
                );
            }
            if !compact {
                w::kv_styled(
                    ui,
                    lang.text("เงินทอน", "Change"),
                    &change.map_or("—".into(), format::money),
                    w::Value::Strong,
                    type_scale::MONEY_LG,
                );
            }
        } else {
            w::help_text(
                ui,
                lang.text(
                    "ตรวจยอดกับธนาคารหรือเครื่องรับบัตรด้วยตัวเองก่อนยืนยัน แอปไม่โอน ไม่ตัดบัตร และไม่นับเข้าลิ้นชัก",
                    "Verify with your bank or card terminal before confirming. The app does not transfer, charge cards, or add to the drawer.",
                ),
            );
        }
    });
    if app.payment_method == PaymentMethod::Cash
        && let Ok(total) = total
    {
        let options = quick_cash(total);
        let mut labels = vec![(lang.text("พอดี", "Exact").to_owned(), cash == Some(total))];
        labels.extend(options.iter().map(|m| {
            (
                format::count(u64::try_from(m.satang() / 100).unwrap_or_default()),
                cash == Some(*m),
            )
        }));
        ui.add_enabled_ui(!app.busy, |ui| {
            if let Some(i) = w::chip_row(ui, &labels) {
                let value = if i == 0 { total } else { options[i - 1] };
                app.cash = format::money(value);
            }
        });
    }
    if app.cart.table.is_none() {
        kitchen_button(app, ui);
    }
    if app.data.shift.is_none() {
        shift_gate(app, ui);
        app.sell.pay_requested = false;
        return;
    }
    let ready = !app.busy
        && total.is_ok()
        && (app.payment_method != PaymentMethod::Cash || change.is_some());
    let label = match app.payment_method {
        PaymentMethod::Cash => lang.text("รับเงินสด", "Take cash"),
        PaymentMethod::Transfer if !app.shop.promptpay.is_empty() => {
            lang.text("แสดง QR รับโอน", "Show transfer QR")
        }
        PaymentMethod::Transfer => lang.text("ตรวจรับเงินโอน", "Review transfer"),
        PaymentMethod::Card => lang.text("ตรวจรับชำระบัตร", "Review card payment"),
    };
    let clicked = w::button(label)
        .primary()
        .size(w::Size::Xl)
        .fill()
        .money(total.as_ref().map_or(String::new(), |m| format::baht(*m)))
        .shortcut("F9")
        .enabled(ready)
        .loading(app.busy)
        .show(ui)
        .clicked();
    let requested = std::mem::take(&mut app.sell.pay_requested);
    if requested && app.payment_method == PaymentMethod::Cash && !ready {
        // F9 first moves focus to the cash field (OrderSummary.md).
        app.sell.focus_cash = true;
    } else if clicked || (requested && ready) {
        take_payment(app);
    }
}
/// A table bill while ordering: one total line, Send to kitchen and Take
/// payment. Take payment (F9) opens the full payment section.
fn ordering_footer(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let price = app.bill_price();
    let units: u64 = app.cart.lines.iter().map(|l| u64::from(l.quantity)).sum();
    t.sunken().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(t.text(lang.text("ยอดรวม", "Total"), type_scale::BODY_STRONG));
            let mut detail = vec![format!(
                "{} {}",
                format::count(units),
                lang.noun(units, "ชิ้น", "item", "items")
            )];
            if let Ok(p) = &price {
                if p.service_charge.satang() > 0 {
                    detail.push(format!("SC {}", format::money(p.service_charge)));
                }
                if p.vat_added.satang() > 0 {
                    detail.push(format!("VAT {}", format::money(p.vat_added)));
                }
                if p.discounts().satang() > 0 {
                    detail.push(format!(
                        "{} {}",
                        lang.text("ลด", "off"),
                        format::money(p.discounts())
                    ));
                }
            }
            ui.label(t.muted(detail.join(" · "), type_scale::CAPTION));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(price.as_ref().map_or("—".into(), |p| format::baht(p.total)))
                        .font(t.strong_font(type_scale::MONEY_LG)),
                );
            });
        });
        if price.is_err() {
            w::inline_error(
                ui,
                lang.text(
                    "ส่วนลดมากกว่ายอด · แก้ส่วนลด (F7 / F8)",
                    "A discount is more than the price · fix it (F7 / F8)",
                ),
            );
        }
    });
    let pay = |app: &mut App| {
        app.sell.paying = Some(app.cart.id);
        app.sell.focus_cash = true;
    };
    let requested = std::mem::take(&mut app.sell.pay_requested);
    let can_pay = app.data.shift.is_some() && price.is_ok() && !app.busy;
    // Stacked: the kitchen label's length varies (+2 / −1, English).
    kitchen_button(app, ui);
    if w::button(lang.text("รับเงิน", "Take payment"))
        .primary()
        .size(w::Size::Lg)
        .fill()
        .money(
            price
                .as_ref()
                .map_or(String::new(), |p| format::baht(p.total)),
        )
        .shortcut("F9")
        .enabled(can_pay)
        .show(ui)
        .clicked()
    {
        pay(app);
    }
    if requested && can_pay {
        pay(app);
    }
    if app.data.shift.is_none() {
        shift_gate(app, ui);
    }
}
fn cash_input(app: &mut App, ui: &mut egui::Ui, field_w: f32) {
    let short = cash_short(app).is_some();
    let r = w::money_input(
        ui,
        &mut app.cash,
        w::MoneyOpts {
            id: cash_id(),
            primary: true,
            width: Some(field_w),
            large: true,
            invalid: short,
            enabled: !app.busy,
        },
    );
    if app.sell.focus_cash {
        r.request_focus();
        app.sell.focus_cash = false;
    }
    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        app.sell.pay_requested = true;
    }
}
fn shift_gate(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    w::banner(
        ui,
        w::Tone::Warning,
        lang.text("เปิดกะก่อนรับเงิน", "Open a shift before taking payment"),
        None,
    );
    if w::button(lang.text("เปิดกะ", "Open shift"))
        .primary()
        .size(w::Size::Xl)
        .icon(Icon::Shift)
        .fill()
        .enabled(app.ready && !app.busy)
        .show(ui)
        .clicked()
    {
        app.drawer.selected = None;
        app.navigate(Page::Shift);
    }
}
fn take_payment(app: &mut App) {
    let Some(shift) = app.data.shift.as_ref().map(|s| s.id) else {
        return;
    };
    let Ok(total) = app.total() else {
        return;
    };
    if app.payment_method == PaymentMethod::Cash {
        let Ok(cash) = format::parse_money(&app.cash) else {
            app.sell.focus_cash = true;
            return;
        };
        if cash < total {
            return;
        }
        // Unusually large tender (> 10× due) needs a second look.
        if total.satang() > 0 && cash.satang() / 10 > total.satang() && !app.sell.large_cash_ok {
            app.sell.confirm_large_cash = true;
            return;
        }
        app.sell.large_cash_ok = false;
        app.send(Command::Pay(
            app.cart.clone(),
            PaymentRequest {
                method: PaymentMethod::Cash,
                amount: cash,
            },
            shift,
        ));
    } else {
        app.error = None;
        app.payment_review = Some(PaymentReview {
            cart: app.cart.clone(),
            payment: PaymentRequest {
                method: app.payment_method,
                amount: total,
            },
            shift_id: shift,
        });
    }
}

fn held_list(app: &mut App, ui: &mut egui::Ui, held: Vec<Cart>) {
    let t = Theme::of(ui);
    let lang = app.lang;
    egui::ScrollArea::vertical()
        .id_salt("held-bills")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if held.is_empty() {
                w::state_view(
                    ui,
                    Icon::Pause,
                    lang.text("ไม่มีบิลที่พักไว้", "No held bills"),
                    lang.text(
                        "กด F4 หรือปุ่มพักบิลเพื่อเก็บบิลไว้ แล้วเปิดคืนจากที่นี่",
                        "Press F4 or Hold to set a bill aside, then reopen it here.",
                    ),
                );
            }
            for cart in held {
                let label = if cart.label.is_empty() {
                    format!(
                        "{} {}",
                        lang.text("บิล", "Bill"),
                        &cart.id.simple().to_string()[..6]
                    )
                } else {
                    cart.label.clone()
                };
                let units: u64 = cart.lines.iter().map(|l| u64::from(l.quantity)).sum();
                let total = cart.lines.iter().try_fold(Money::ZERO, |sum, l| {
                    app.known
                        .get(&l.product_id)
                        .and_then(|p| p.price.checked_mul(l.quantity).ok())
                        .and_then(|m| sum.checked_add(m).ok())
                });
                ui.push_id(cart.id, |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(space::S2 as i8, space::S2 as i8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.set_width((ui.available_width() - 150.0).max(80.0));
                                    ui.add(
                                        egui::Label::new(t.text(&label, type_scale::BODY_STRONG))
                                            .truncate(),
                                    );
                                    ui.label(t.muted(
                                        format!(
                                            "{} {} · {} {}{}",
                                            cart.lines.len(),
                                            lang.noun(
                                                cart.lines.len() as u64,
                                                "รายการ",
                                                "line",
                                                "lines"
                                            ),
                                            format::count(units),
                                            lang.noun(units, "ชิ้น", "item", "items"),
                                            total.map_or(String::new(), |m| format!(
                                                " · {}",
                                                format::baht(m)
                                            ))
                                        ),
                                        type_scale::CAPTION,
                                    ));
                                });
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    ui.add_enabled_ui(!app.busy, |ui| {
                                        if w::icon_button_sized(
                                            ui,
                                            Icon::Trash,
                                            lang.text("ลบบิลพัก", "Delete held bill"),
                                            None,
                                            false,
                                        )
                                        .clicked()
                                        {
                                            app.held_delete = Some(cart.id);
                                        }
                                        if w::button(lang.text("เปิดบิล", "Open"))
                                            .size(w::Size::Sm)
                                            .show(ui)
                                            .on_hover_text(lang.text(
                                                "บิลปัจจุบันถูกเก็บไว้อัตโนมัติ",
                                                "The current bill is kept automatically.",
                                            ))
                                            .clicked()
                                        {
                                            app.resume(cart.id);
                                        }
                                    });
                                });
                            });
                        });
                    w::divider(ui);
                });
            }
        });
}

// ───────────────────────────── Products ─────────────────────────────

fn categories(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    egui::ScrollArea::vertical()
        .id_salt("sell-categories")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let mut entries = vec![(
                None,
                lang.text("ทั้งหมด", "All").to_owned(),
                app.data.counts.total,
            )];
            entries.extend(app.data.counts.categories.iter().map(|(c, n)| {
                (
                    Some(c.clone()),
                    if c.is_empty() {
                        lang.text("ไม่มีหมวด", "Uncategorised").to_owned()
                    } else {
                        c.clone()
                    },
                    *n,
                )
            }));
            for (value, label, n) in entries {
                if nav_item(ui, &t, &label, n, app.sell.category == value).clicked() {
                    app.sell.category = value;
                    app.refresh();
                }
            }
        });
}
/// Side navigation row with a count (categories, saved views).
pub fn nav_item(ui: &mut egui::Ui, t: &Theme, label: &str, n: u64, active: bool) -> egui::Response {
    let width = ui.available_width();
    let pad = space::S3;
    let count = format::count(n);
    let count_w = w::text_width(ui, &count, t.font(type_scale::MONEY_SM)) + 8.0;
    let (g, _) = crate::ui::text::wrapped(
        ui.painter(),
        label,
        if active {
            t.strong_font(type_scale::BODY)
        } else {
            t.font(type_scale::BODY)
        },
        if active { t.p.fg_default } else { t.p.fg_muted },
        width - pad * 2.0 - count_w - space::S2,
        0,
    );
    let h = (g.size().y + space::S2 * 2.0).max(t.m.control_md);
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, label));
    let bg = if active {
        t.p.bg_sunken
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    ui.painter().galley(
        pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0),
        g,
        t.p.fg_default,
    );
    ui.painter().text(
        pos2(rect.right() - pad, rect.center().y),
        Align2::RIGHT_CENTER,
        count,
        if active {
            t.strong_font(type_scale::MONEY_SM)
        } else {
            t.font(type_scale::MONEY_SM)
        },
        if active { t.p.fg_default } else { t.p.fg_muted },
    );
    w::focus_ring(ui, rect, CornerRadius::same(8), &response);
    response
}

fn products(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let search = w::search_input(
        ui,
        &mut app.search,
        lang.text(
            "ค้นหาชื่อ รหัส หรือสแกนบาร์โค้ด",
            "Search name, SKU or scan barcode",
        ),
        Some("F2"),
        search_id(),
    );
    if std::mem::take(&mut app.sell.focus_search) {
        search.request_focus();
    }
    if search.changed() {
        app.search_changed = Some(Instant::now());
    }
    if search.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        enter_search(app);
        app.sell.focus_search = true;
    }
    ui.add_space(space::S2);
    let in_cart = |id: uuid::Uuid| -> u32 {
        app.cart
            .lines
            .iter()
            .filter(|l| l.product_id == id)
            .map(|l| l.quantity)
            .fold(0_u32, u32::saturating_add)
    };
    let list = app.data.products.clone();
    let counts: Vec<u32> = list.iter().map(|p| in_cart(p.id)).collect();
    let mut add = None;
    egui::ScrollArea::vertical()
        .id_salt("sell-products")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if list.is_empty() {
                empty_products(app, ui);
                return;
            }
            let gap = if t.touch() { 10.0 } else { space::S2 };
            let width = ui.available_width();
            let cols = ((width + gap) / (t.m.tile_min_width + gap))
                .floor()
                .max(1.0) as usize;
            let tile_w = ((width - gap * (cols as f32 - 1.0)) / cols as f32).floor();
            for (row, chunk) in list.chunks(cols).enumerate() {
                let heights: Vec<f32> = chunk
                    .iter()
                    .map(|p| tile_height(ui, &t, p, tile_w))
                    .collect();
                let row_h = heights.iter().copied().fold(t.m.tile_min_height, f32::max);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (i, p) in chunk.iter().enumerate() {
                        let n = counts[row * cols + i];
                        if product_tile(ui, &t, lang, p, vec2(tile_w, row_h), n, app.busy) {
                            add = Some(p.clone());
                        }
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
        });
    if let Some(p) = add {
        app.add(p);
    }
}
fn enter_search(app: &mut App) {
    let code = app.search.trim().to_owned();
    if code.is_empty() {
        return;
    }
    let exact =
        app.data.products.iter().find(|p| {
            p.sku.eq_ignore_ascii_case(&code) || p.barcode.as_deref() == Some(code.as_str())
        });
    if let Some(p) = exact.cloned() {
        if p.on_sale && p.stock != Some(0) {
            app.add(p);
            app.search.clear();
            app.search_changed = Some(Instant::now());
        }
        return;
    }
    let results_current = app.search_changed.is_none();
    match app
        .data
        .products
        .iter()
        .find(|p| p.on_sale && p.stock != Some(0))
    {
        Some(p) if results_current => {
            let p = p.clone();
            app.add(p);
            app.search.clear();
            app.search_changed = Some(Instant::now());
        }
        _ => app.send(Command::Lookup(code)),
    }
}
fn empty_products(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    if app.data.counts.total == 0 {
        w::state_view(
            ui,
            Icon::Products,
            lang.text("ยังไม่มีสินค้า", "No products yet"),
            lang.text(
                "เพิ่มสินค้าหรือเมนูแรก แล้วแตะเพื่อขายได้ทันที",
                "Add your first product or menu item, then tap it to sell.",
            ),
        );
        ui.vertical_centered(|ui| {
            if w::button(lang.text("เพิ่มสินค้า", "Add product"))
                .primary()
                .icon(Icon::Plus)
                .show(ui)
                .clicked()
            {
                app.catalog.editor = Some(crate::catalog_ui::Editor::default());
                app.navigate(Page::Products);
            }
            if w::button(lang.text("โหลดสินค้าตัวอย่าง", "Load sample products"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                app.send(Command::Demo);
            }
        });
    } else {
        let filters = app.sell.category.as_ref().map_or(String::new(), |c| {
            format!(" · {} {c}", lang.text("หมวด", "in"))
        });
        w::state_view(
            ui,
            Icon::Search,
            &format!(
                "{} “{}”{filters}",
                lang.text("ไม่พบ", "No match for"),
                app.search.trim()
            ),
            lang.text(
                "ลองชื่อ รหัส หรือบาร์โค้ดอื่น หรือล้างตัวกรอง",
                "Try another name, code or barcode, or clear the filters.",
            ),
        );
        ui.vertical_centered(|ui| {
            if w::button(lang.text("ล้างตัวกรอง", "Clear filters"))
                .show(ui)
                .clicked()
            {
                app.search.clear();
                app.sell.category = None;
                app.refresh();
            }
        });
    }
}
fn tile_pad(t: &Theme) -> egui::Vec2 {
    if t.touch() {
        vec2(space::S3, space::S3)
    } else {
        vec2(space::S3, 10.0)
    }
}
fn tile_height(ui: &egui::Ui, t: &Theme, p: &Product, width: f32) -> f32 {
    let pad = tile_pad(t);
    let (g, _) = crate::ui::text::wrapped(
        ui.painter(),
        &p.name,
        t.font(type_scale::BODY),
        t.p.fg_default,
        width - pad.x * 2.0 - 26.0,
        3,
    );
    pad.y * 2.0 + g.size().y + 6.0 + 22.0
}
fn product_tile(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: crate::i18n::Language,
    p: &Product,
    size: egui::Vec2,
    in_cart: u32,
    busy: bool,
) -> bool {
    let out = p.stock == Some(0);
    let unavailable = out || !p.on_sale;
    let (rect, response) = ui.allocate_exact_size(
        size,
        if unavailable || busy {
            Sense::hover()
        } else {
            Sense::click()
        },
    );
    if !ui.is_rect_visible(rect) {
        return false;
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            !unavailable,
            format!(
                "{} {} {}",
                lang.text("เพิ่ม", "Add"),
                p.name,
                format::baht(p.price)
            ),
        )
    });
    let pressed = response.is_pointer_button_down_on();
    let r = if pressed { rect.shrink(1.5) } else { rect };
    let (bg, stroke) = if unavailable {
        (t.p.bg_sunken, Stroke::new(1.0, t.p.border_default))
    } else if in_cart > 0 {
        (t.p.bg_raised, Stroke::new(2.0, t.p.tile_selected_border()))
    } else if response.hovered() {
        (t.p.bg_raised, Stroke::new(1.0, t.p.border_control))
    } else {
        (t.p.bg_raised, Stroke::new(1.0, t.p.border_default))
    };
    ui.painter()
        .rect(r, CornerRadius::same(8), bg, stroke, StrokeKind::Inside);
    let pad = tile_pad(t);
    let text_color = if unavailable {
        t.p.fg_muted
    } else {
        t.p.fg_default
    };
    let (g, truncated) = crate::ui::text::wrapped(
        ui.painter(),
        &p.name,
        t.font(type_scale::BODY),
        text_color,
        r.width() - pad.x * 2.0 - 26.0,
        3,
    );
    ui.painter().galley(r.min + pad, g, text_color);
    if in_cart > 0 {
        w::paint_count(ui, pos2(r.right() - 18.0, r.top() + 18.0), in_cart, false);
    }
    let foot_y = r.bottom() - pad.y - 10.0;
    let price = ui.painter().text(
        pos2(r.right() - pad.x, foot_y),
        Align2::RIGHT_CENTER,
        format::money(p.price),
        t.strong_font(type_scale::MONEY),
        text_color,
    );
    let state = if !p.on_sale {
        Some((
            lang.text("ปิดขาย", "Not for sale").to_owned(),
            t.p.danger_fg,
            true,
        ))
    } else if out {
        Some((
            lang.text("หมด", "Out of stock").to_owned(),
            t.p.danger_fg,
            true,
        ))
    } else if p.is_low_stock() {
        Some((
            format!("{} {}", lang.text("เหลือ", "Left"), p.stock.unwrap_or(0)),
            t.p.warning_fg,
            true,
        ))
    } else if !p.options.is_empty() {
        Some((
            lang.text("มีตัวเลือก", "Options").to_owned(),
            t.p.fg_muted,
            false,
        ))
    } else {
        p.stock.map(|n| {
            (
                format!(
                    "{} {}",
                    lang.text("เหลือ", "Left"),
                    format::count(u64::from(n))
                ),
                t.p.fg_muted,
                false,
            )
        })
    };
    if let Some((text, color, strong)) = state {
        let font = if strong {
            t.strong_font(type_scale::CAPTION)
        } else {
            t.font(type_scale::CAPTION)
        };
        // Products with options show the options icon before the label.
        let icon_w = if p.options.is_empty() || out || !p.on_sale || p.is_low_stock() {
            0.0
        } else {
            icons::paint_sized(
                ui.painter(),
                Icon::Options,
                pos2(r.left() + pad.x + 7.0, foot_y),
                14.0,
                color,
            );
            18.0
        };
        let max_w = (price.left() - r.left() - pad.x - icon_w - space::S2).max(20.0);
        let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
        job.wrap.max_width = max_w;
        job.wrap.max_rows = 1;
        let g = ui.painter().layout_job(job);
        let h = g.size().y;
        ui.painter()
            .galley(pos2(r.left() + pad.x + icon_w, foot_y - h / 2.0), g, color);
    }
    w::focus_ring(ui, r, CornerRadius::same(8), &response);
    let tip = if !p.on_sale {
        format!(
            "{}\n{}",
            p.name,
            lang.text(
                "ปิดขายอยู่ เปิดขายได้ที่หน้าสินค้า",
                "Not for sale. Turn selling on in Products."
            )
        )
    } else if out {
        format!(
            "{}\n{}",
            p.name,
            lang.text(
                "สินค้าหมด เพิ่มสต็อกที่หน้าสินค้า",
                "Out of stock. Add stock in Products."
            )
        )
    } else if truncated {
        p.name.clone()
    } else {
        String::new()
    };
    let response = if tip.is_empty() {
        response
    } else {
        response.on_hover_text(tip)
    };
    response.clicked()
}

// ───────────────────────────── Dialogs ─────────────────────────────

/// Pins the reviewed amount and shift until submission; the database revalidates both.
pub struct PaymentReview {
    cart: Cart,
    payment: PaymentRequest,
    shift_id: uuid::Uuid,
}
impl PaymentReview {
    #[cfg(debug_assertions)]
    pub fn new(cart: Cart, payment: PaymentRequest, shift_id: uuid::Uuid) -> Self {
        Self {
            cart,
            payment,
            shift_id,
        }
    }
}
pub fn payment_dialog(app: &mut App, ctx: &egui::Context) {
    large_cash_dialog(app, ctx);
    let Some(review) = app.payment_review.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let promptpay = (review.payment.method == PaymentMethod::Transfer)
        .then(|| app.shop.promptpay_id())
        .flatten();
    let width = if promptpay.is_some() { 600.0 } else { 480.0 };
    let shop_name = app.shop.name.clone();
    let response = w::dialog(ctx, "review-payment", width, |ui| {
        if let Some(id) = &promptpay {
            w::dialog_title(
                ui,
                lang.text(
                    "ให้ลูกค้าสแกน QR พร้อมเพย์",
                    "Ask the customer to scan the PromptPay QR",
                ),
                false,
            );
            ui.add_space(space::S2);
            ui.horizontal_top(|ui| {
                let side = if t.touch() { 232.0 } else { 208.0 };
                let payload = id.payload(Some(review.payment.amount));
                if let Some(r) = crate::ui::qr::show(ui, &payload, side) {
                    r.on_hover_text(lang.text("QR พร้อมเพย์พร้อมยอด", "PromptPay QR with amount"));
                }
                ui.add_space(space::S4);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = space::S1;
                    ui.label(t.muted(lang.text("ยอดโอน", "Amount"), type_scale::LABEL));
                    ui.label(
                        RichText::new(format::baht(review.payment.amount))
                            .font(t.strong_font(type_scale::MONEY_XL)),
                    );
                    ui.add_space(space::S2);
                    ui.label(t.muted(lang.text("โอนเข้า", "Pays to"), type_scale::LABEL));
                    w::name_label(ui, &shop_name, type_scale::BODY_STRONG, t.p.fg_default);
                    ui.label(t.text(
                        format!("{} {}", lang.text("พร้อมเพย์", "PromptPay"), id.masked()),
                        type_scale::BODY,
                    ));
                    ui.add_space(space::S2);
                    ui.add(
                        egui::Label::new(t.text(
                            lang.text(
                                "เมื่อลูกค้าโอนแล้ว ตรวจว่ายอดนี้เข้าแอปธนาคารของร้านก่อนยืนยัน",
                                "After the customer pays, find this amount in the shop's banking app before confirming.",
                            ),
                            type_scale::BODY,
                        ))
                        .wrap(),
                    );
                });
            });
            w::help_text(
                ui,
                lang.text(
                    "QR สร้างในเครื่องนี้ แอปไม่ได้เชื่อมธนาคารและไม่ทราบว่าเงินเข้าแล้วหรือยัง",
                    "The QR is made on this device. The app is not connected to the bank and cannot tell whether money arrived.",
                ),
            );
        } else {
            w::dialog_title(
                ui,
                if review.payment.method == PaymentMethod::Card {
                    lang.text("ได้รับชำระผ่านบัตรแล้วใช่ไหม?", "Was the card payment approved?")
                } else {
                    lang.text("ได้รับเงินโอนเข้าบัญชีแล้วใช่ไหม?", "Has the transfer arrived?")
                },
                false,
            );
            ui.label(
                RichText::new(format::baht(review.payment.amount))
                    .font(t.strong_font(type_scale::MONEY_XL)),
            );
            ui.add(
                egui::Label::new(t.text(
                    if review.payment.method == PaymentMethod::Card {
                        lang.text(
                            "ตรวจว่าเครื่องรับบัตรอนุมัติยอดนี้ก่อนยืนยัน",
                            "Check the card terminal approved this amount before confirming.",
                        )
                    } else {
                        lang.text(
                            "ตรวจยอดนี้ในแอปธนาคารของร้านก่อนยืนยัน",
                            "Find this amount in the shop's banking app before confirming.",
                        )
                    },
                    type_scale::BODY,
                ))
                .wrap(),
            );
            w::help_text(
            ui,
            lang.text(
                "แอปบันทึกการขายเท่านั้น ไม่ตรวจยอดธนาคาร ไม่ตัดบัตร และไม่เพิ่มเงินสดในลิ้นชัก",
                "This records the sale only. No bank verification, card charge, or cash drawer increase.",
            ),
        );
        }
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            let confirm = if promptpay.is_some() {
                lang.text("เงินเข้าแล้ว ยืนยัน", "Money arrived, confirm")
            } else {
                lang.text("ยืนยันรับชำระแล้ว", "Confirm payment received")
            };
            if w::button(confirm)
                .primary()
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("กลับไปแก้บิล", "Back to bill"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out
    });
    match response.inner {
        Some(false) => return,
        None if !app.busy && response.should_close() => return,
        Some(true) => app.send(Command::Pay(
            review.cart.clone(),
            review.payment,
            review.shift_id,
        )),
        None => {}
    }
    app.payment_review = Some(review);
}
fn large_cash_dialog(app: &mut App, ctx: &egui::Context) {
    if !app.sell.confirm_large_cash {
        return;
    }
    let lang = app.lang;
    let t = Theme::current(ctx);
    let cash = format::parse_money(&app.cash).unwrap_or_default();
    let total = app.total().unwrap_or_default();
    let r = w::dialog(ctx, "large-cash", 460.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} {}?",
                lang.text("รับเงินสด", "Take cash of"),
                format::baht(cash)
            ),
            false,
        );
        ui.label(t.text(
            format!(
                "{} {} · {} {}",
                lang.text("ยอดชำระ", "Amount due"),
                format::baht(total),
                lang.text("เงินทอน", "change"),
                total.change_from(cash).map_or("—".into(), format::baht)
            ),
            type_scale::BODY,
        ));
        w::help_text(
            ui,
            lang.text(
                "จำนวนเงินมากกว่ายอดชำระเกิน 10 เท่า ตรวจว่าพิมพ์ถูกก่อนรับเงิน",
                "The cash is more than 10× the amount due. Check it was typed correctly.",
            ),
        );
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ใช่ รับเงิน", "Yes, take cash"))
                .primary()
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("แก้จำนวนเงิน", "Edit amount"))
                .ghost()
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out
    });
    match r.inner {
        Some(true) => {
            app.sell.confirm_large_cash = false;
            app.sell.large_cash_ok = true;
            take_payment(app);
        }
        Some(false) => {
            app.sell.confirm_large_cash = false;
            app.sell.focus_cash = true;
        }
        None if r.should_close() => app.sell.confirm_large_cash = false,
        None => {}
    }
}

/// F6 / Options: products with options reopen the sheet; others edit the note.
fn edit_line(app: &mut App, index: usize) {
    let Some(line) = app.cart.lines.get(index).cloned() else {
        return;
    };
    match app.known.get(&line.product_id).cloned() {
        Some(p) if !p.options.is_empty() => {
            app.sell.sheet = Some(OptionSheet::new(p, Some((index, line))));
        }
        _ => app.sell.note_edit = Some(index),
    }
}
/// Side sheet (design/docs/components/Overlay.md) for choosing options.
pub fn options_sheet(app: &mut App, ctx: &egui::Context) {
    let Some(mut sheet) = app.sell.sheet.take() else {
        return;
    };
    let t = Theme::current(ctx);
    let lang = app.lang;
    let screen = ctx.content_rect();
    let width = 440.0_f32.min(screen.width() - 40.0);
    let priced =
        pos_domain::price_with_options(sheet.product.price, &sheet.product.options, &sheet.chosen);
    let mut close = false;
    let mut confirm = false;
    egui::Area::new(Id::new("options-scrim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0, t.p.overlay_scrim);
            if response.clicked() {
                close = true;
            }
        });
    egui::Area::new(Id::new("options-sheet"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(screen.right() - width, screen.top()))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t.p.bg_raised)
                .shadow(t.shadow(3))
                .inner_margin(egui::Margin::same(space::S5 as i8))
                .show(ui, |ui| {
                    let inner = vec2(width - space::S5 * 2.0, screen.height() - space::S5 * 2.0);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    ui.spacing_mut().item_spacing.y = space::S2;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(inner.x - 44.0);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(t.muted(
                                if sheet.edit.is_some() {
                                    lang.text("แก้ตัวเลือก", "Edit options")
                                } else {
                                    lang.text("เลือกตัวเลือก", "Choose options")
                                },
                                type_scale::LABEL,
                            ));
                            w::name_label(
                                ui,
                                &sheet.product.name,
                                type_scale::TITLE,
                                t.p.fg_default,
                            );
                            ui.label(t.muted(format::baht(sheet.product.price), type_scale::MONEY));
                        });
                        ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                            if w::icon_button_sized(
                                ui,
                                Icon::Close,
                                lang.text("ปิด (Esc)", "Close (Esc)"),
                                Some(36.0),
                                false,
                            )
                            .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                    w::divider(ui);
                    let footer_h = t.m.control_xl + t.m.control_md + space::S6 * 2.0;
                    egui::ScrollArea::vertical()
                        .id_salt("options-body")
                        .auto_shrink([false, false])
                        .max_height(ui.available_height() - footer_h)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = space::S2;
                            for group in sheet.product.options.clone() {
                                ui.add_space(space::S2);
                                ui.horizontal(|ui| {
                                    ui.label(t.text(&group.name, type_scale::BODY_STRONG));
                                    let (text, tone) = match (group.required, group.multiple) {
                                        (true, false) => {
                                            (lang.text("ต้องเลือก 1", "Pick 1"), w::Tone::Warning)
                                        }
                                        (true, true) => {
                                            (lang.text("ต้องเลือก", "Required"), w::Tone::Warning)
                                        }
                                        (false, true) => {
                                            (lang.text("เลือกได้หลายอย่าง", "Any"), w::Tone::Neutral)
                                        }
                                        (false, false) => {
                                            (lang.text("ไม่บังคับ", "Optional"), w::Tone::Neutral)
                                        }
                                    };
                                    w::badge(ui, text, tone);
                                });
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
                                    for choice in &group.choices {
                                        let picked = sheet.chosen.iter().any(|c| {
                                            c.group_id == group.id && c.choice_id == choice.id
                                        });
                                        let label = if choice.price.satang() > 0 {
                                            format!(
                                                "{}  +{}",
                                                choice.name,
                                                format::money(choice.price)
                                            )
                                        } else {
                                            choice.name.clone()
                                        };
                                        if w::chip(ui, &label, picked, false).clicked() {
                                            if picked {
                                                if group.multiple || !group.required {
                                                    sheet
                                                        .chosen
                                                        .retain(|c| c.choice_id != choice.id);
                                                }
                                            } else {
                                                if !group.multiple {
                                                    sheet.chosen.retain(|c| c.group_id != group.id);
                                                }
                                                sheet.chosen.push(pos_domain::LineOption {
                                                    group_id: group.id,
                                                    choice_id: choice.id,
                                                });
                                            }
                                        }
                                    }
                                });
                            }
                            ui.add_space(space::S3);
                            w::field_label(
                                ui,
                                lang.text("หมายเหตุ", "Note"),
                                Some(lang.text("(ไม่บังคับ)", "(optional)")),
                            );
                            w::text_input(
                                ui,
                                &mut sheet.note,
                                w::FieldOpts {
                                    hint: lang.text("เช่น แยกน้ำแข็ง", "e.g. ice on the side"),
                                    limit: 240,
                                    ..Default::default()
                                },
                            );
                        });
                    w::divider(ui);
                    ui.horizontal(|ui| {
                        ui.label(t.text(lang.text("จำนวน", "Quantity"), type_scale::BODY));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = space::S1;
                            if w::icon_button(ui, Icon::Plus, lang.text("เพิ่มจำนวน", "Increase"))
                                .clicked()
                            {
                                sheet.quantity = sheet.quantity.saturating_add(1).min(999_999);
                            }
                            let (r, _) =
                                ui.allocate_exact_size(vec2(56.0, t.m.control_md), Sense::hover());
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                format::count(u64::from(sheet.quantity)),
                                t.strong_font(type_scale::MONEY),
                                t.p.fg_default,
                            );
                            ui.add_enabled_ui(sheet.quantity > 1, |ui| {
                                if w::icon_button(ui, Icon::Minus, lang.text("ลดจำนวน", "Decrease"))
                                    .clicked()
                                {
                                    sheet.quantity -= 1;
                                }
                            });
                        });
                    });
                    match &priced {
                        Err(_) => w::inline_error(
                            ui,
                            lang.text("เลือกตัวเลือกที่ต้องเลือกให้ครบ", "Choose every required option."),
                        ),
                        Ok(_) => {
                            ui.add_space(space::S1);
                        }
                    }
                    let total = priced
                        .as_ref()
                        .ok()
                        .and_then(|(unit, _)| unit.checked_mul(sheet.quantity).ok());
                    ui.horizontal(|ui| {
                        if w::button(lang.text("ยกเลิก", "Cancel"))
                            .ghost()
                            .show(ui)
                            .clicked()
                        {
                            close = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let label = if sheet.edit.is_some() {
                                lang.text("บันทึก", "Save")
                            } else {
                                lang.text("เพิ่มลงบิล", "Add to bill")
                            };
                            confirm |= w::button(label)
                                .primary()
                                .size(w::Size::Lg)
                                .money(total.map_or(String::new(), format::baht))
                                .shortcut("Enter")
                                .enabled(total.is_some())
                                .show(ui)
                                .clicked();
                        });
                    });
                });
        });
    let typing = ctx.memory(|m| m.focused().is_some());
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            close = true;
        }
        if !typing && i.consume_key(Modifiers::NONE, Key::Enter) && priced.is_ok() {
            confirm = true;
        }
    });
    if confirm && priced.is_ok() {
        let line = pos_domain::CartLine {
            product_id: sheet.product.id,
            quantity: sheet.quantity,
            note: sheet.note.trim().to_owned(),
            options: sheet.chosen.clone(),
            // Editing options keeps the line's discount.
            discount: sheet
                .edit
                .and_then(|i| app.cart.lines.get(i))
                .and_then(|l| l.discount),
        };
        match sheet.edit {
            Some(index) if index < app.cart.lines.len() => {
                app.cart.lines[index] = line;
                app.sell.current = Some(index);
            }
            _ => app.add_line(sheet.product.clone(), line),
        }
        return;
    }
    if !close {
        app.sell.sheet = Some(sheet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(s: &str) -> Money {
        format::parse_money(s).unwrap_or_default()
    }
    #[test]
    fn quick_cash_rounds_up_to_distinct_notes() {
        assert_eq!(
            quick_cash(m("1323.50")),
            vec![m("1400"), m("1500"), m("2000")]
        );
        assert_eq!(quick_cash(m("45")), vec![m("100"), m("500"), m("1000")]);
        assert_eq!(quick_cash(m("500")), vec![m("600"), m("1000"), m("2000")]);
        assert!(quick_cash(m("0")).iter().all(|v| v.satang() > 0));
    }
}
