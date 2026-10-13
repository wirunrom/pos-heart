//! Settings → Printed documents: the shop's receipt, check bill and kitchen
//! ticket layouts, edited beside a live preview of exactly what prints.
use crate::{
    app::App,
    print::{
        docs,
        render::{Doc, TextSize},
        template::{self, DocLanguage, Templates},
    },
    ui::{Theme, space, type_scale, w},
};
use eframe::egui::{self, Align};
use pos_domain::{KitchenTicket, SaleRecord};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Receipt,
    Bill,
    Kitchen,
}
#[derive(Default)]
pub struct TemplatesState {
    pub draft: Option<Templates>,
    pub tab: Tab,
    pub logo_path: String,
}

/// A receipt to preview: the latest sale, or a sample before the first one.
fn sample_receipt(app: &App) -> Option<SaleRecord> {
    if let Some(r) = app.data.sales.first() {
        return Some(r.clone());
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let sale = serde_json::json!({
        "id": uuid::Uuid::nil(), "number": 1, "created_at": now, "label": "โต๊ะ 3",
        "lines": [
            {"product_id": uuid::Uuid::nil(), "sku": "F-001", "name": "ข้าวกะเพราไก่ไข่ดาว",
             "unit_price": 6500, "quantity": 2, "note": "ไม่เผ็ด", "total": 13000,
             "options": [{"group": "ไข่", "name": "ไข่ดาว", "price": 0}]},
            {"product_id": uuid::Uuid::nil(), "sku": "D-002", "name": "ชาไทยเย็น",
             "unit_price": 4500, "quantity": 1, "note": "", "total": 4500}
        ],
        "total": 17500, "payment_method": "cash", "cash": 20000, "change": 2500
    });
    Some(SaleRecord {
        sale: serde_json::from_value(sale).ok()?,
        cancellation: None,
        refunds: Vec::new(),
        tax_invoice: None,
    })
}
fn sample_ticket(app: &App) -> KitchenTicket {
    if let Some(t) = app.data.kitchen.first() {
        return t.clone();
    }
    KitchenTicket {
        id: uuid::Uuid::nil(),
        number: 12,
        cart_id: uuid::Uuid::nil(),
        table: "T3".into(),
        guests: 4,
        created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        lines: vec![
            pos_domain::TicketLine {
                product_id: uuid::Uuid::nil(),
                name: "ข้าวกะเพราไก่ไข่ดาว".into(),
                quantity: 2,
                options: vec!["ไข่ดาว".into()],
                note: "ไม่เผ็ด".into(),
                done: false,
            },
            pos_domain::TicketLine {
                product_id: uuid::Uuid::nil(),
                name: "ผัดไทยกุ้งสด".into(),
                quantity: -1,
                options: Vec::new(),
                note: String::new(),
                done: false,
            },
        ],
        bumped_at: None,
        staff: Default::default(),
    }
}
fn preview_doc(app: &App, t: &Templates, tab: Tab) -> Option<Doc> {
    match tab {
        Tab::Receipt => sample_receipt(app).map(|r| app.receipt_doc(&t.receipt, &r, false)),
        Tab::Bill => {
            let cart = if app.cart.lines.is_empty() {
                app.data
                    .held
                    .iter()
                    .find(|c| !c.lines.is_empty())
                    .cloned()?
            } else {
                app.cart.clone()
            };
            app.check_bill_doc(&t.receipt, &cart)
        }
        Tab::Kitchen => Some(docs::kitchen_ticket(
            &sample_ticket(app),
            &t.kitchen,
            app.lang,
        )),
    }
}

/// Checkboxes in equal columns. `horizontal_wrapped` would squeeze the last
/// wrapping labels down to one letter per line.
fn toggle_grid(ui: &mut egui::Ui, toggles: Vec<(&mut bool, &str)>) {
    let width = ui.available_width();
    let cols = ((width / 180.0).floor() as usize).clamp(1, 3);
    let col_w = (width - space::S3 * (cols - 1) as f32) / cols as f32;
    let mut toggles = toggles.into_iter().peekable();
    while toggles.peek().is_some() {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = space::S3;
            for (value, label) in toggles.by_ref().take(cols) {
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, 0.0),
                    egui::Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(col_w);
                        w::check_label(ui, value, label);
                    },
                );
            }
        });
    }
}

fn size_row(ui: &mut egui::Ui, lang: crate::i18n::Language, size: &mut TextSize) {
    ui.horizontal(|ui| {
        ui.label(lang.text("ขนาดตัวอักษร", "Text size"));
        w::segmented(
            ui,
            size,
            &[
                (TextSize::Small, lang.text("เล็ก", "Small")),
                (TextSize::Normal, lang.text("ปกติ", "Normal")),
                (TextSize::Large, lang.text("ใหญ่", "Large")),
            ],
        );
    });
}
fn language_row(ui: &mut egui::Ui, lang: crate::i18n::Language, value: &mut DocLanguage) {
    ui.horizontal(|ui| {
        ui.label(lang.text("ภาษาบนใบพิมพ์", "Printed language"));
        w::segmented(
            ui,
            value,
            &[
                (DocLanguage::App, lang.text("ตามแอป", "As the app")),
                (DocLanguage::Thai, "ไทย"),
                (DocLanguage::English, "English"),
            ],
        );
    });
}

pub fn settings_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("รูปแบบใบพิมพ์", "Printed documents"));
    w::help_text(
        ui,
        lang.text(
            "ปรับหัวและท้ายใบ โลโก้ ขนาดตัวอักษร ภาษา และรายละเอียดที่จะพิมพ์ ภาพด้านขวาคือสิ่งที่จะออกจากเครื่องพิมพ์จริง ใบกำกับภาษียังพิมพ์ข้อมูลที่กฎหมายกำหนดครบเสมอ",
            "Adjust the header and footer, logo, text size, language and details. The preview on the right is exactly what prints. Tax invoices always keep the legally required lines.",
        ),
    );
    let may = crate::staff_ui::may(app, pos_domain::Permission::Settings);
    let saved = app.shop.templates.clone();
    let mut draft = app.templates.draft.take().unwrap_or_else(|| saved.clone());
    let mut tab = app.templates.tab;
    ui.horizontal(|ui| {
        w::segmented(
            ui,
            &mut tab,
            &[
                (Tab::Receipt, lang.text("ใบเสร็จ", "Receipt")),
                (Tab::Bill, lang.text("ใบแจ้งยอด", "Check bill")),
                (Tab::Kitchen, lang.text("ใบครัว", "Kitchen ticket")),
            ],
        );
    });
    app.templates.tab = tab;
    let full = ui.available_width();
    let preview_w = if full > 700.0 { 280.0 } else { 220.0 };
    let mut import: Option<String> = None;
    let mut remove_logo = false;
    ui.horizontal_top(|ui| {
        let controls_w = (full - preview_w - space::S6 - space::S2).max(200.0);
        ui.allocate_ui_with_layout(
            egui::vec2(controls_w, 0.0),
            egui::Layout::top_down(Align::Min),
            |ui| {
                // Text areas would otherwise stretch the card past its siblings.
                ui.set_width(controls_w);
                ui.set_max_width(controls_w);
                ui.spacing_mut().item_spacing.y = space::S2;
                ui.add_enabled_ui(may, |ui| match tab {
                    Tab::Receipt | Tab::Bill => {
                        let r = &mut draft.receipt;
                        if tab == Tab::Receipt {
                            w::field_label(ui, lang.text("ชื่อใบ", "Title"), Some(lang.text("เว้นว่าง = ใบเสร็จรับเงิน", "empty = Receipt")));
                            w::text_input(
                                ui,
                                &mut r.title,
                                w::FieldOpts {
                                    hint: lang.text("เช่น ใบเสร็จรับเงิน / Receipt", "e.g. Receipt"),
                                    limit: 60,
                                    ..Default::default()
                                },
                            );
                        }
                        w::field_label(ui, lang.text("หัวใบ (ใต้ชื่อร้าน)", "Header (under the shop name)"), None);
                        w::text_area(ui, &mut r.header, lang.text("ที่อยู่ เบอร์โทร LINE เวลาเปิด-ปิด…", "Address, phone, LINE, hours…"), 400, 3);
                        w::field_label(ui, lang.text("ท้ายใบ", "Footer"), None);
                        w::text_area(ui, &mut r.footer, lang.text("ขอบคุณที่อุดหนุน · รหัส Wi-Fi · เงื่อนไขคืนสินค้า…", "Thank you · Wi-Fi password · return policy…"), 400, 3);
                        size_row(ui, lang, &mut r.size);
                        language_row(ui, lang, &mut r.language);
                        w::field_label(ui, lang.text("แสดงบนใบ", "Show"), None);
                        let mut toggles = vec![
                            (&mut r.show_label, lang.text("ชื่อบิล/โต๊ะ", "Bill name / table")),
                            (&mut r.show_staff, lang.text("ชื่อพนักงาน", "Staff name")),
                            (&mut r.show_unit_price, lang.text("ราคาต่อชิ้น", "Unit price")),
                            (&mut r.show_options, lang.text("ตัวเลือกสินค้า", "Options")),
                            (&mut r.show_notes, lang.text("หมายเหตุ", "Notes")),
                            (&mut r.show_sku, lang.text("รหัสสินค้า", "Product code")),
                            (&mut r.show_cash, lang.text("รับเงิน/เงินทอน", "Cash & change")),
                        ];
                        if tab == Tab::Bill {
                            toggles.push((&mut r.bill_qr, lang.text("QR พร้อมเพย์บนใบแจ้งยอด", "PromptPay QR on check bills")));
                        }
                        toggle_grid(ui, toggles);
                        if tab == Tab::Bill && app.shop.promptpay.is_empty() {
                            w::help_text(ui, lang.text("ใส่เลขพร้อมเพย์ใน ตั้งค่า → ร้านและหน้าเริ่มต้น ก่อน จึงจะพิมพ์ QR ได้", "Add a PromptPay ID in Settings → Shop & startup to print the QR."));
                        }
                        w::field_label(ui, lang.text("โลโก้", "Logo"), None);
                        ui.horizontal(|ui| {
                            w::check_label(ui, &mut r.logo, lang.text("พิมพ์โลโก้", "Print the logo"));
                            ui.label(t.muted(
                                if app.logo.is_some() {
                                    lang.text("มีโลโก้แล้ว", "Logo set")
                                } else {
                                    lang.text("ยังไม่มีโลโก้", "No logo yet")
                                },
                                type_scale::CAPTION,
                            ));
                        });
                        ui.horizontal(|ui| {
                            w::text_input(
                                ui,
                                &mut app.templates.logo_path,
                                w::FieldOpts {
                                    hint: lang.text("วางตำแหน่งไฟล์ .png/.jpg หรือลากไฟล์มาวางในหน้าต่าง", "Paste a .png/.jpg path, or drop a file on the window"),
                                    limit: 1024,
                                    width: Some((ui.available_width() - 150.0).max(120.0)),
                                    ..Default::default()
                                },
                            );
                            if w::button(lang.text("ใช้ภาพนี้", "Use image"))
                                .size(w::Size::Sm)
                                .enabled(!app.templates.logo_path.trim().is_empty())
                                .show(ui)
                                .clicked()
                            {
                                import = Some(app.templates.logo_path.trim().to_owned());
                            }
                        });
                        if app.logo.is_some()
                            && w::button(lang.text("ลบโลโก้", "Remove logo"))
                                .ghost()
                                .size(w::Size::Sm)
                                .show(ui)
                                .clicked()
                        {
                            remove_logo = true;
                        }
                        w::help_text(ui, lang.text("ภาพถูกย่อและแปลงเป็นขาวดำสำหรับเครื่องพิมพ์ความร้อน ภาพตัดพื้นหลัง (โปร่งใส) จะออกมาสวยที่สุด", "Images are resized and converted to black and white for thermal paper; transparent backgrounds work best."));
                    }
                    Tab::Kitchen => {
                        let k = &mut draft.kitchen;
                        size_row(ui, lang, &mut k.size);
                        language_row(ui, lang, &mut k.language);
                        w::field_label(ui, lang.text("แสดงบนใบ", "Show"), None);
                        toggle_grid(
                            ui,
                            vec![
                                (&mut k.show_options, lang.text("ตัวเลือกสินค้า", "Options")),
                                (&mut k.show_guests, lang.text("จำนวนคน", "Guests")),
                                (&mut k.show_staff, lang.text("ชื่อพนักงาน", "Staff name")),
                            ],
                        );
                        w::help_text(ui, lang.text("หมายเหตุและใบยกเลิกพิมพ์เสมอ เพื่อไม่ให้ครัวทำผิด", "Notes and cancellations always print so the kitchen never misses them."));
                    }
                });
                let dirty = draft != saved;
                let valid = draft.validate().is_ok();
                if !valid {
                    w::inline_error(ui, lang.text("หัวหรือท้ายใบยาวเกินไป (ไม่เกิน 8 บรรทัด 400 ตัวอักษร)", "Header or footer is too long (8 lines, 400 characters)"));
                }
                ui.horizontal(|ui| {
                    if w::button(lang.text("ค่าเริ่มต้น", "Defaults"))
                        .ghost()
                        .enabled(may && draft != Templates::default())
                        .show(ui)
                        .clicked()
                    {
                        draft = Templates::default();
                    }
                    if w::button(lang.text("ทิ้งการแก้ไข", "Discard"))
                        .ghost()
                        .enabled(dirty)
                        .show(ui)
                        .clicked()
                    {
                        draft = saved.clone();
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if w::button(lang.text("บันทึก", "Save"))
                            .primary()
                            .enabled(may && dirty && valid)
                            .show(ui)
                            .clicked()
                        {
                            let mut shop = app.shop.clone();
                            shop.templates = draft.clone();
                            match shop.save(&app.data_dir) {
                                Ok(()) => {
                                    app.shop_draft.templates = draft.clone();
                                    app.shop = shop;
                                    app.toast(lang.text("บันทึกรูปแบบใบพิมพ์แล้ว", "Document layout saved."));
                                }
                                Err(e) => app.error = Some(e.to_string()),
                            }
                        }
                    });
                });
            },
        );
        ui.add_space(space::S6);
        ui.allocate_ui_with_layout(egui::vec2(preview_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            ui.set_width(preview_w);
            ui.label(t.muted(
                format!(
                    "{} · {}",
                    lang.text("ตัวอย่างที่จะพิมพ์", "Print preview"),
                    match crate::print_ui::preview_paper(app) {
                        crate::print::Paper::Mm58 => "58 mm",
                        crate::print::Paper::Mm80 => "80 mm",
                    }
                ),
                type_scale::CAPTION,
            ));
            match preview_doc(app, &draft, tab) {
                Some(doc) => {
                    let key = crate::print_ui::doc_key(&doc);
                    crate::print_ui::paper(app, ui, &doc, key, preview_w);
                }
                None => w::help_text(
                    ui,
                    lang.text(
                        "ยังไม่มีบิลเปิดอยู่ เปิดบิลหรือพักบิลไว้ก่อนเพื่อดูใบแจ้งยอด",
                        "No open bill yet. Open or hold a bill to preview a check bill.",
                    ),
                ),
            }
        });
    });
    // A file dropped on the window becomes the logo.
    if let Some(path) = ui
        .ctx()
        .input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        && tab != Tab::Kitchen
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ["png", "jpg", "jpeg"].contains(&e.to_ascii_lowercase().as_str()))
    {
        import = Some(path.display().to_string());
    }
    if let Some(path) = import {
        match template::import_logo(std::path::Path::new(&path), &app.data_dir) {
            Ok(logo) => {
                app.logo = Some(logo);
                draft.receipt.logo = true;
                app.templates.logo_path.clear();
                app.toast(lang.text("ใช้โลโก้ใหม่แล้ว", "Logo updated."));
            }
            Err(e) => {
                app.error = Some(format!(
                    "{}: {e}",
                    lang.text("ใช้ภาพนี้ไม่ได้", "Cannot use this image")
                ))
            }
        }
    }
    if remove_logo {
        let _ = std::fs::remove_file(app.data_dir.join(template::LOGO_FILE));
        app.logo = None;
        draft.receipt.logo = false;
    }
    if draft != saved {
        app.templates.draft = Some(draft);
    }
}
