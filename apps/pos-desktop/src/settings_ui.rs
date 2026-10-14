//! First-run setup and Settings (.impeccable/surfaces/startup.md).
//! Shop profile saves explicitly; language, theme and density apply immediately.
use crate::{
    app::{App, Page},
    i18n::Language,
    ui::{Density, Theme, space, theme as ds_theme, type_scale, w},
};
use eframe::egui::{self, Align, vec2};

fn profile(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    w::field_label(ui, lang.text("ชื่อร้าน", "Shop name"), None);
    let empty = app.shop_draft.name.trim().is_empty();
    w::text_input(
        ui,
        &mut app.shop_draft.name,
        w::FieldOpts {
            hint: lang.text("เช่น ร้านหัวใจ", "e.g. Heart Shop"),
            limit: 80,
            invalid: empty,
            ..Default::default()
        },
    );
    if app.shop_draft.name.trim().is_empty() {
        w::inline_error(ui, lang.text("ใส่ชื่อร้าน", "Enter a shop name"));
    }
}
fn promptpay_valid(text: &str) -> bool {
    text.trim().is_empty() || pos_domain::PromptPayId::parse(text).is_ok()
}
fn promptpay(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::field_label(
        ui,
        lang.text("พร้อมเพย์สำหรับ QR รับโอน", "PromptPay for transfer QR"),
        Some(lang.text("ไม่บังคับ", "optional")),
    );
    let valid = promptpay_valid(&app.shop_draft.promptpay);
    w::text_input(
        ui,
        &mut app.shop_draft.promptpay,
        w::FieldOpts {
            hint: lang.text("เบอร์มือถือ หรือเลขประจำตัวผู้เสียภาษี", "Mobile number or tax ID"),
            limit: 24,
            invalid: !valid,
            width: Some(ui.available_width().min(320.0)),
            ..Default::default()
        },
    );
    match pos_domain::PromptPayId::parse(&app.shop_draft.promptpay) {
        Ok(id) => {
            let kind = match id.kind() {
                pos_domain::PromptPayKind::Phone => lang.text("เบอร์มือถือ", "Mobile number"),
                pos_domain::PromptPayKind::TaxId => lang.text("เลขประจำตัว 13 หลัก", "13-digit ID"),
                pos_domain::PromptPayKind::EWallet => lang.text("e-Wallet", "e-Wallet"),
            };
            ui.label(t.muted(
                format!(
                    "{kind} · {} · {}",
                    id.masked(),
                    lang.text(
                        "QR ใส่ยอดให้อัตโนมัติเมื่อเลือกรับโอน",
                        "The QR carries the amount when taking a transfer",
                    )
                ),
                type_scale::CAPTION,
            ));
        }
        Err(_) if !valid => w::inline_error(
            ui,
            lang.text(
                "ใช้เบอร์มือถือ 10 หลัก เลขประจำตัว 13 หลัก หรือ e-Wallet 15 หลัก",
                "Use a 10-digit mobile, 13-digit ID or 15-digit e-Wallet number",
            ),
        ),
        Err(_) => w::help_text(
            ui,
            lang.text(
                "ถ้าใส่ไว้ ตอนรับโอนจะแสดง QR พร้อมยอดให้ลูกค้าสแกน แอปไม่ตรวจว่าเงินเข้าแล้ว",
                "When set, taking a transfer shows a QR with the amount for the customer to scan. The app does not check that money arrived.",
            ),
        ),
    }
}
pub fn onboard(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    egui::ScrollArea::vertical().id_salt("setup").show(ui, |ui| {
        let width = ui.available_width().min(560.0);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.add_space(space::S8);
                w::page_title(ui, lang.text("เริ่มต้นร้านของคุณ", "Set up your shop"));
                ui.add(egui::Label::new(t.muted(
                    lang.text(
                        "ตั้งชื่อร้าน แล้วเพิ่มสินค้าเพื่อเริ่มขาย ข้อมูลทั้งหมดเก็บในเครื่องนี้ ไม่ต้องสมัครบัญชี",
                        "Name your shop, then add products to start selling. Everything stays on this device. No account needed.",
                    ),
                    type_scale::BODY,
                )).wrap());
                ui.add_space(space::S4);
                t.surface().inner_margin(egui::Margin::same(space::S6 as i8)).show(ui, |ui| {
                    ui.set_width(width - space::S6 * 2.0);
                    ui.spacing_mut().item_spacing.y = space::S2;
                    profile(app, ui);
                    ui.add_space(space::S4);
                    ui.horizontal(|ui| {
                        if w::button(lang.text("ตั้งค่าภายหลัง", "Set up later"))
                            .ghost()
                            .enabled(!app.busy)
                            .show(ui)
                            .clicked()
                        {
                            app.shop_draft = Default::default();
                            app.save_shop(Some(Page::Home));
                        }
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if w::button(lang.text("บันทึกและเพิ่มสินค้า", "Save & add products"))
                                .primary()
                                .size(w::Size::Lg)
                                .enabled(!app.busy && !app.shop_draft.name.trim().is_empty())
                                .show(ui)
                                .clicked()
                            {
                                app.save_shop(Some(Page::Products));
                            }
                        });
                    });
                });
                ui.add_space(space::S3);
                w::help_text(
                    ui,
                    lang.text(
                        "แก้ไขได้ภายหลังที่ตั้งค่า · ภาษา ธีม และขนาดปุ่ม ปรับได้จากปุ่มมุมขวาบน",
                        "Change this later in Settings · language, theme and density are in the top-right button.",
                    ),
                );
            });
        });
    });
}
/// Settings groups, one at a time (SideNav, components/Navigation.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Section {
    #[default]
    Shop,
    Pricing,
    Promotions,
    Tables,
    Printing,
    Staff,
    Display,
    Data,
}
impl Section {
    pub const ALL: [Self; 8] = [
        Self::Shop,
        Self::Pricing,
        Self::Promotions,
        Self::Tables,
        Self::Printing,
        Self::Staff,
        Self::Display,
        Self::Data,
    ];
    pub fn label(self, lang: Language) -> &'static str {
        match self {
            Self::Shop => lang.text("ร้านและหน้าเริ่มต้น", "Shop & startup"),
            Self::Pricing => lang.text("ราคาและภาษี", "Pricing & tax"),
            Self::Promotions => lang.text("โปรโมชัน", "Promotions"),
            Self::Tables => lang.text("โต๊ะและโซน", "Tables & zones"),
            Self::Printing => lang.text("การพิมพ์", "Printing"),
            Self::Staff => lang.text("พนักงานและสิทธิ์", "Staff & permissions"),
            Self::Display => lang.text("ภาษาและหน้าจอ", "Language & display"),
            Self::Data => lang.text("ข้อมูลและการสำรอง", "Data & backup"),
        }
    }
    /// Debug presets and links: `settings-printing`, …
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "shop" => Self::Shop,
            "pricing" => Self::Pricing,
            "promotions" => Self::Promotions,
            "tables" => Self::Tables,
            "printing" => Self::Printing,
            "staff" => Self::Staff,
            "display" => Self::Display,
            "data" => Self::Data,
            _ => return None,
        })
    }
}
/// Sections holding edits that are not saved yet; they keep their drafts
/// while another section is open.
fn unsaved(app: &App, section: Section) -> bool {
    match section {
        Section::Shop => app.shop_draft != app.shop,
        Section::Pricing => app.pricing_draft.is_some() || app.tax.draft.is_some(),
        Section::Printing => {
            app.print.draft != app.print.settings
                || app
                    .templates
                    .draft
                    .as_ref()
                    .is_some_and(|d| *d != app.shop.templates)
        }
        Section::Promotions | Section::Tables | Section::Staff | Section::Display => false,
        Section::Data => app.auto_backup.dirty(),
    }
}
/// One SideNav row; `unsaved` is the tooltip of the warning dot.
fn section_item(
    ui: &mut egui::Ui,
    t: &Theme,
    label: &str,
    unsaved: Option<&str>,
    active: bool,
) -> egui::Response {
    let dirty = unsaved.is_some();
    let width = ui.available_width();
    let pad = space::S3;
    let dot = if dirty { space::S2 + space::S2 } else { 0.0 };
    let (g, _) = crate::ui::text::wrapped(
        ui.painter(),
        label,
        if active {
            t.strong_font(type_scale::BODY)
        } else {
            t.font(type_scale::BODY)
        },
        if active { t.p.fg_default } else { t.p.fg_muted },
        width - pad * 2.0 - dot,
        0,
    );
    let h = (g.size().y + space::S2 * 2.0).max(t.m.control_md);
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, label));
    let bg = if active {
        t.p.bg_sunken
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        egui::Color32::TRANSPARENT
    };
    let corner = egui::CornerRadius::same(8);
    ui.painter().rect_filled(rect, corner, bg);
    ui.painter().galley(
        egui::pos2(rect.left() + pad, rect.center().y - g.size().y / 2.0),
        g,
        t.p.fg_default,
    );
    if dirty {
        let (_, fg) = w::Tone::Warning.colors(t);
        ui.painter().circle_filled(
            egui::pos2(rect.right() - pad - space::S1, rect.center().y),
            space::S1,
            fg,
        );
    }
    w::focus_ring(ui, rect, corner, &response);
    match unsaved {
        Some(text) => response.on_hover_text(text),
        None => response,
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::page_title(ui, lang.text("ตั้งค่า", "Settings"));
    ui.add_space(space::S2);
    let full = ui.available_width();
    let nav_w = if full < 1100.0 { 196.0 } else { 220.0 };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = space::S3;
        ui.allocate_ui_with_layout(vec2(nav_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            ui.set_width(nav_w);
            t.surface()
                .inner_margin(egui::Margin::same(space::S2 as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for section in Section::ALL {
                        let active = app.settings_section == section;
                        let dirty = unsaved(app, section)
                            .then(|| lang.text("มีการแก้ไขที่ยังไม่บันทึก", "Unsaved changes"));
                        if section_item(ui, &t, section.label(lang), dirty, active).clicked() {
                            app.settings_section = section;
                        }
                    }
                });
        });
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .id_salt(("settings", app.settings_section))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let width = ui.available_width().min(760.0);
                    ui.set_max_width(width);
                    match app.settings_section {
                        Section::Shop => card(ui, &t, |ui| shop_card(app, ui)),
                        Section::Pricing => {
                            card(ui, &t, |ui| crate::tax_ui::pricing_card(app, ui));
                            card(ui, &t, |ui| crate::tax_ui::settings_card(app, ui));
                        }
                        Section::Promotions => {
                            card(ui, &t, |ui| crate::promo_ui::settings_card(app, ui))
                        }
                        Section::Tables => {
                            card(ui, &t, |ui| crate::tables_ui::settings_card(app, ui))
                        }
                        Section::Printing => {
                            card(ui, &t, |ui| crate::templates_ui::settings_card(app, ui));
                            card(ui, &t, |ui| {
                                let manager =
                                    crate::staff_ui::may(app, pos_domain::Permission::Settings);
                                ui.add_enabled_ui(manager, |ui| {
                                    crate::print_ui::settings_card(app, ui)
                                });
                            });
                        }
                        Section::Staff => {
                            card(ui, &t, |ui| crate::auto_lock_ui::card(app, ui));
                            card(ui, &t, |ui| crate::staff_ui::settings_card(app, ui));
                        }
                        Section::Display => card(ui, &t, |ui| display_card(app, ui)),
                        Section::Data => card(ui, &t, |ui| data_card(app, ui)),
                    }
                    #[cfg(debug_assertions)]
                    if app.preview_scroll_bottom {
                        ui.scroll_to_cursor(Some(Align::BOTTOM));
                        app.preview_scroll_bottom = false;
                    }
                });
        });
    });
}
fn card(ui: &mut egui::Ui, t: &Theme, add: impl FnOnce(&mut egui::Ui)) {
    t.surface().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = space::S2;
        add(ui);
    });
    ui.add_space(space::S3);
}
fn shop_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("ร้านและหน้าเริ่มต้น", "Shop & startup"));
    if app.shop_readonly {
        w::banner(
                ui,
                w::Tone::Danger,
                lang.text(
                    "อ่าน shop.json ไม่ได้ ปิดแอปแล้วตรวจไฟล์ก่อนแก้ข้อมูลร้าน ไฟล์จะไม่ถูกเขียนทับ",
                    "Cannot read shop.json. Close the app and check the file before editing. It will not be overwritten.",
                ),
                None,
            );
    }
    let dirty = app.shop_draft != app.shop;
    if dirty {
        w::banner(
            ui,
            w::Tone::Warning,
            lang.text("มีการแก้ไขที่ยังไม่บันทึก", "Unsaved changes"),
            None,
        );
    }
    let manager = crate::staff_ui::may(app, pos_domain::Permission::Settings);
    if !manager {
        w::help_text(
            ui,
            lang.text("บทบาทของคุณแก้ไขส่วนนี้ไม่ได้", "Your role cannot change this."),
        );
    }
    ui.add_enabled_ui(!app.shop_readonly && manager, |ui| {
            profile(app, ui);
            ui.add_space(space::S2);
            promptpay(app, ui);
            ui.add_space(space::S2);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - 60.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(t.text(lang.text("เปิดแอปเข้าหน้าขายทันที", "Open directly to Sell"), type_scale::BODY_STRONG));
                    w::help_text(
                        ui,
                        lang.text(
                            "เหมาะกับเครื่องแคชเชียร์ ถ้าปิดไว้จะเปิดหน้าหลัก (มีผลเมื่อบันทึก)",
                            "For a dedicated counter. Otherwise the app opens Home (applies when saved).",
                        ),
                    );
                });
                w::switch(ui, &mut app.shop_draft.start_in_sell, lang.text("เปิดแอปเข้าหน้าขายทันที", "Open directly to Sell"));
            });
            ui.add_space(space::S2);
            ui.horizontal(|ui| {
                if w::button(lang.text("ทิ้งการแก้ไข", "Discard changes"))
                    .ghost()
                    .enabled(dirty)
                    .show(ui)
                    .clicked()
                {
                    app.shop_draft = app.shop.clone();
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if w::button(lang.text("บันทึก", "Save"))
                        .primary()
                        .enabled(
                            dirty
                                && !app.shop_draft.name.trim().is_empty()
                                && promptpay_valid(&app.shop_draft.promptpay),
                        )
                        .show(ui)
                        .clicked()
                    {
                        app.save_shop(None);
                    }
                });
            });
        });
}
fn display_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(
        ui,
        lang.text("ภาษา ธีม และขนาด", "Language, theme & density"),
    );
    w::help_text(
        ui,
        lang.text(
            "มีผลทันทีและจำไว้ในเครื่องนี้",
            "Applies immediately and is remembered on this device.",
        ),
    );
    let mut next = lang;
    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(160.0, 24.0),
            egui::Label::new(t.text(lang.text("ภาษา", "Language"), type_scale::BODY)),
        );
        if w::segmented(
            ui,
            &mut next,
            &[(Language::Thai, "ไทย"), (Language::English, "English")],
        ) {
            app.set_language(next);
        }
    });
    let mut appearance = app.appearance;
    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(160.0, 24.0),
            egui::Label::new(t.text(lang.text("ธีม", "Theme"), type_scale::BODY)),
        );
        if w::segmented(
            ui,
            &mut appearance,
            &[
                (ds_theme::Appearance::Light, lang.text("สว่าง", "Light")),
                (ds_theme::Appearance::Dark, lang.text("มืด", "Dark")),
                (ds_theme::Appearance::System, lang.text("ตามระบบ", "System")),
            ],
        ) {
            app.set_display(appearance, app.density);
        }
    });
    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(160.0, 24.0),
            egui::Label::new(t.text(lang.text("เต็มจอ", "Full screen"), type_scale::BODY)),
        );
        let mut full = app.fullscreen;
        if w::switch(ui, &mut full, lang.text("เต็มจอ", "Full screen")).changed() {
            app.set_fullscreen(full);
        }
        ui.label(t.muted(
            lang.text(
                "เหมาะกับเครื่องแคชเชียร์ · F11 สลับได้ทุกเมื่อ",
                "For a dedicated counter · F11 toggles any time",
            ),
            type_scale::CAPTION,
        ));
    });
    let mut pictures = app.tile_images;
    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(160.0, 24.0),
            egui::Label::new(t.text(
                lang.text("รูปสินค้าในหน้าขาย", "Product pictures"),
                type_scale::BODY,
            )),
        );
        if w::switch(
            ui,
            &mut pictures,
            lang.text("รูปสินค้าในหน้าขาย", "Product pictures"),
        )
        .changed()
        {
            app.set_tile_images(pictures);
        }
        ui.label(t.muted(
            lang.text(
                "ปิดเพื่อให้ปุ่มสินค้าเล็กลงและเห็นได้มากขึ้น",
                "Off: smaller tiles, more on screen",
            ),
            type_scale::CAPTION,
        ));
    });
    let mut density = app.density;
    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(160.0, 24.0),
            egui::Label::new(t.text(lang.text("ขนาดปุ่มและตัวอักษร", "Density"), type_scale::BODY)),
        );
        if w::segmented(
            ui,
            &mut density,
            &[
                (
                    Density::Compact,
                    lang.text("กะทัดรัด · เมาส์/คีย์บอร์ด", "Compact · mouse/keyboard"),
                ),
                (Density::Touch, lang.text("จอสัมผัส", "Touch")),
            ],
        ) {
            app.set_display(app.appearance, density);
        }
    });
}
fn data_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("ข้อมูลและการสำรอง", "Data & backup"));
    ui.label(t.text(
        if app.demo {
            lang.text(
                "กำลังใช้ข้อมูลตัวอย่าง แยกจากข้อมูลร้านจริง",
                "Using demo data, separate from your real shop.",
            )
        } else {
            lang.text(
                "ข้อมูลร้านเก็บในเครื่องนี้ ใช้งานออฟไลน์ได้",
                "Shop data is stored on this device and works offline.",
            )
        },
        type_scale::BODY,
    ));
    w::field_label(ui, lang.text("ตำแหน่งข้อมูล", "Data folder"), None);
    ui.add(
        egui::Label::new(t.muted(app.data_dir.display().to_string(), type_scale::MONEY_SM))
            .wrap()
            .selectable(true),
    );
    crate::backup_ui::controls(app, ui);
    if crate::staff_ui::may(app, pos_domain::Permission::Reports)
        && w::button(lang.text("ประวัติกิจกรรม", "Activity log"))
            .show(ui)
            .clicked()
    {
        app.page = crate::app::Page::Activity;
        app.send(crate::worker::Command::Activity(app.activity.query.clone()));
    }
}
