//! VAT settings card and the full tax invoice dialog.
//! Receipts of a VAT-registered shop are abbreviated tax invoices; a full
//! tax invoice is issued on request from Sales history.
use crate::{
    app::App,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align};
use pos_domain::{Buyer, SaleRecord, TaxSettings};
use uuid::Uuid;

#[derive(Default)]
pub struct TaxState {
    pub draft: Option<TaxSettings>,
    pub rate: String,
    pub invoice: Option<InvoiceEditor>,
}
pub struct InvoiceEditor {
    pub sale_id: Uuid,
    pub sale_number: u64,
    pub name: String,
    pub tax_id: String,
    pub head_office: bool,
    pub branch: String,
    pub address: String,
}
impl InvoiceEditor {
    fn buyer(&self) -> Buyer {
        Buyer {
            name: self.name.trim().to_owned(),
            tax_id: self.tax_id.trim().to_owned(),
            branch: if self.head_office {
                "00000".into()
            } else {
                format!("{:0>5}", self.branch.trim())
            },
            address: self.address.trim().to_owned(),
        }
    }
}
/// "สำนักงานใหญ่" / "สาขาที่ 00001".
pub fn branch_label(lang: crate::i18n::Language, branch: &str) -> String {
    if branch == "00000" {
        lang.text("สำนักงานใหญ่", "Head office").to_owned()
    } else {
        format!("{} {branch}", lang.text("สาขาที่", "Branch"))
    }
}
pub fn rate_label(rate_bp: u32) -> String {
    format::discount_rule(pos_domain::Discount::Percent(rate_bp.max(1)))
}

pub fn settings_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("ภาษีมูลค่าเพิ่ม (VAT)", "VAT"));
    w::help_text(
        ui,
        lang.text(
            "สำหรับร้านที่จดทะเบียน VAT เลือกได้ว่าราคาที่ตั้งรวม VAT แล้วหรือให้บวกเพิ่ม ใบเสร็จจะเป็นใบกำกับภาษีอย่างย่อ และออกใบกำกับภาษีเต็มรูปให้ลูกค้าได้จากประวัติขาย การใช้เครื่อง POS ออกใบกำกับภาษีอย่างย่อต้องขออนุมัติกรมสรรพากรก่อน (ภ.พ.06) แอปไม่ยื่นแบบภาษีให้",
            "For VAT-registered shops. Choose whether prices include VAT or have it added; receipts become abbreviated tax invoices, and full tax invoices can be issued from Sales history. Using a POS for abbreviated tax invoices needs Revenue Department approval first. The app does not file tax returns.",
        ),
    );
    let owner = crate::staff_ui::may(app, pos_domain::Permission::Settings);
    if !owner {
        w::help_text(
            ui,
            lang.text("บทบาทของคุณแก้ไขส่วนนี้ไม่ได้", "Your role cannot change this."),
        );
    }
    let saved = app.data.tax.clone();
    let mut draft = app.tax.draft.take().unwrap_or_else(|| saved.clone());
    if app.tax.rate.is_empty() {
        app.tax.rate = rate_label(draft.rate_bp).trim_end_matches('%').to_owned();
    }
    let label_w = 180.0;
    let row = |ui: &mut egui::Ui, text: &str| {
        ui.add_sized(
            egui::vec2(label_w, 24.0),
            egui::Label::new(t.text(text, type_scale::BODY)),
        );
    };
    ui.add_enabled_ui(owner && app.ready, |ui| {
        ui.horizontal(|ui| {
            row(ui, lang.text("จดทะเบียน VAT", "VAT registered"));
            w::switch(
                ui,
                &mut draft.enabled,
                lang.text("จดทะเบียน VAT", "VAT registered"),
            );
        });
        if draft.enabled {
            let field =
                |ui: &mut egui::Ui, value: &mut String, hint: &str, limit: usize, invalid: bool| {
                    w::text_input(
                        ui,
                        value,
                        w::FieldOpts {
                            hint,
                            limit,
                            invalid,
                            width: Some(ui.available_width().min(420.0)),
                            ..Default::default()
                        },
                    );
                };
            ui.horizontal(|ui| {
                row(ui, lang.text("ชื่อผู้ประกอบการ", "Registered name"));
                let empty = draft.seller_name.trim().is_empty();
                field(
                    ui,
                    &mut draft.seller_name,
                    lang.text("เช่น บริษัท หัวใจ จำกัด", "e.g. Heart Co., Ltd."),
                    160,
                    empty,
                );
            });
            ui.horizontal(|ui| {
                row(ui, lang.text("เลขประจำตัวผู้เสียภาษี", "Tax ID"));
                draft.tax_id.retain(|c| c.is_ascii_digit());
                let bad = !draft.tax_id.is_empty() && !pos_domain::valid_tax_id(&draft.tax_id);
                field(ui, &mut draft.tax_id, "0105555xxxxxx", 13, bad);
            });
            ui.horizontal(|ui| {
                row(ui, lang.text("สาขา", "Branch"));
                let mut head = draft.branch == "00000";
                if w::segmented(
                    ui,
                    &mut head,
                    &[
                        (true, lang.text("สำนักงานใหญ่", "Head office")),
                        (false, lang.text("สาขา", "Branch")),
                    ],
                ) {
                    draft.branch = if head { "00000".into() } else { "00001".into() };
                }
                if !head {
                    draft.branch.retain(|c| c.is_ascii_digit());
                    let bad = !pos_domain::valid_branch(&draft.branch);
                    w::text_input(
                        ui,
                        &mut draft.branch,
                        w::FieldOpts {
                            hint: "00001",
                            limit: 5,
                            invalid: bad,
                            width: Some(90.0),
                            ..Default::default()
                        },
                    );
                }
            });
            ui.horizontal(|ui| {
                row(ui, lang.text("ที่อยู่", "Address"));
                let empty = draft.address.trim().is_empty();
                field(
                    ui,
                    &mut draft.address,
                    lang.text("ที่อยู่ตามใบ ภ.พ.20", "Address as registered"),
                    300,
                    empty,
                );
            });
            ui.horizontal(|ui| {
                row(ui, lang.text("ราคาสินค้าที่ตั้งไว้", "Product prices"));
                w::segmented(
                    ui,
                    &mut draft.prices_include_vat,
                    &[
                        (true, lang.text("รวม VAT แล้ว", "Include VAT")),
                        (
                            false,
                            lang.text("ยังไม่รวม VAT (บวกเพิ่ม)", "Exclude VAT (added)"),
                        ),
                    ],
                );
            });
            ui.horizontal(|ui| {
                row(ui, lang.text("อัตรา VAT", "VAT rate"));
                w::text_input(
                    ui,
                    &mut app.tax.rate,
                    w::FieldOpts {
                        hint: "7",
                        limit: 6,
                        width: Some(90.0),
                        ..Default::default()
                    },
                );
                ui.label(t.text("%", type_scale::BODY));
                if let Some(bp) = format::parse_money(&app.tax.rate)
                    .ok()
                    .and_then(|m| u32::try_from(m.satang()).ok())
                {
                    draft.rate_bp = bp;
                }
            });
            ui.horizontal(|ui| {
                row(ui, lang.text("หมายเลขเครื่อง POS", "POS number"));
                field(
                    ui,
                    &mut draft.pos_id,
                    lang.text(
                        "ตามที่สรรพากรอนุมัติ (ถ้ามี)",
                        "From the Revenue Department, if any",
                    ),
                    40,
                    false,
                );
            });
        }
        let dirty = draft != saved;
        let valid = draft.validate().is_ok();
        if dirty && !valid && draft.enabled {
            w::inline_error(
                ui,
                &app.lang.error(&pos_core::Error::Domain(
                    pos_domain::DomainError::InvalidTaxSettings,
                )),
            );
        }
        ui.horizontal(|ui| {
            if w::button(lang.text("ทิ้งการแก้ไข", "Discard"))
                .ghost()
                .enabled(dirty)
                .show(ui)
                .clicked()
            {
                draft = saved.clone();
                app.tax.rate.clear();
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if w::button(lang.text("บันทึก", "Save"))
                    .primary()
                    .enabled(dirty && valid && !app.busy)
                    .show(ui)
                    .clicked()
                {
                    app.send(Command::SetTax(draft.clone()));
                }
            });
        });
    });
    if draft != saved {
        app.tax.draft = Some(draft);
    }
}

/// Button in receipt actions: issue (or print) the full tax invoice.
pub fn invoice_button(app: &mut App, ui: &mut egui::Ui, record: &SaleRecord) {
    let lang = app.lang;
    if record.sale.tax.is_none() {
        return;
    }
    if let Some(invoice) = &record.tax_invoice {
        if app.print.enabled()
            && w::button(lang.text("พิมพ์ใบกำกับภาษี", "Print tax invoice"))
                .icon(Icon::Printer)
                .loading(app.print.busy())
                .show(ui)
                .clicked()
        {
            app.print_tax_invoice(record, invoice, true);
        }
        return;
    }
    // Refunded sales need a credit note instead.
    if record.cancellation.is_some() || !record.refunds.is_empty() {
        return;
    }
    if w::button(lang.text("ออกใบกำกับภาษีเต็มรูป", "Issue tax invoice"))
        .icon(Icon::Pencil)
        .enabled(!app.busy)
        .show(ui)
        .clicked()
    {
        app.tax.invoice = Some(InvoiceEditor {
            sale_id: record.sale.id,
            sale_number: record.sale.number,
            name: String::new(),
            tax_id: String::new(),
            head_office: true,
            branch: String::new(),
            address: String::new(),
        });
        app.receipt = None;
    }
}

pub fn invoice_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut ed) = app.tax.invoice.take() else {
        return;
    };
    let lang = app.lang;
    let buyer = ed.buyer();
    let valid = buyer.validate().is_ok();
    let response = w::dialog(ctx, "tax-invoice", 520.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} {}",
                lang.text("ออกใบกำกับภาษีเต็มรูป · บิล", "Full tax invoice · receipt"),
                format::receipt_number(ed.sale_number)
            ),
            false,
        );
        w::help_text(
            ui,
            lang.text(
                "ใส่ข้อมูลผู้ซื้อให้ตรงกับเอกสารของลูกค้า ออกได้ครั้งเดียวต่อบิล และแก้ไขภายหลังไม่ได้",
                "Enter the buyer's details exactly as registered. One invoice per receipt; it cannot be edited later.",
            ),
        );
        w::field_label(ui, lang.text("ชื่อผู้ซื้อ", "Buyer name"), None);
        w::text_input(
            ui,
            &mut ed.name,
            w::FieldOpts {
                hint: lang.text("ชื่อบริษัทหรือชื่อ-นามสกุล", "Company or full name"),
                limit: 160,
                ..Default::default()
            },
        );
        w::field_label(ui, lang.text("เลขประจำตัวผู้เสียภาษี", "Tax ID"), None);
        ed.tax_id.retain(|c| c.is_ascii_digit());
        let bad_id = ed.tax_id.len() == 13 && !pos_domain::valid_tax_id(&ed.tax_id);
        w::text_input(
            ui,
            &mut ed.tax_id,
            w::FieldOpts {
                hint: lang.text("13 หลัก", "13 digits"),
                limit: 13,
                invalid: bad_id,
                width: Some(220.0),
                ..Default::default()
            },
        );
        if bad_id {
            w::inline_error(
                ui,
                lang.text(
                    "เลขประจำตัวผู้เสียภาษีไม่ถูกต้อง (ตรวจเลขหลักสุดท้าย)",
                    "Invalid tax ID (check digit)",
                ),
            );
        }
        ui.horizontal(|ui| {
            w::segmented(
                ui,
                &mut ed.head_office,
                &[
                    (true, lang.text("สำนักงานใหญ่", "Head office")),
                    (false, lang.text("สาขา", "Branch")),
                ],
            );
            if !ed.head_office {
                ed.branch.retain(|c| c.is_ascii_digit());
                w::text_input(
                    ui,
                    &mut ed.branch,
                    w::FieldOpts {
                        hint: "00001",
                        limit: 5,
                        width: Some(90.0),
                        ..Default::default()
                    },
                );
            }
        });
        w::field_label(ui, lang.text("ที่อยู่", "Address"), None);
        w::text_input(
            ui,
            &mut ed.address,
            w::FieldOpts {
                hint: lang.text(
                    "เลขที่ ถนน ตำบล อำเภอ จังหวัด รหัสไปรษณีย์",
                    "Street, district, province, postcode",
                ),
                limit: 300,
                ..Default::default()
            },
        );
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ออกใบกำกับภาษี", "Issue invoice"))
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
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        ui.add_space(space::S1);
        out
    });
    match response.inner {
        Some(true) => {
            let command = Command::IssueInvoice(ed.sale_id, buyer);
            app.tax.invoice = Some(ed);
            app.send(command);
        }
        Some(false) => app.error = None,
        None if !app.busy && response.should_close() => app.error = None,
        None => app.tax.invoice = Some(ed),
    }
}

/// Settings → Pricing: service charge and cash rounding.
pub fn pricing_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("การคิดราคา", "Pricing"));
    w::help_text(
        ui,
        lang.text(
            "ลำดับการคิด: ส่วนลด → service charge → VAT (ถ้าราคายังไม่รวม VAT) → ปัดเศษเงินสด ทุกยอดกระจายลงแต่ละรายการ คืนเงินบางรายการจึงได้ตามที่จ่ายจริง",
            "Order: discounts → service charge → VAT (when prices exclude it) → cash rounding. Each amount is spread over the lines, so partial refunds return what was paid.",
        ),
    );
    let owner = crate::staff_ui::may(app, pos_domain::Permission::Settings);
    let saved = app.data.pricing;
    let mut draft = app.pricing_draft.unwrap_or(saved);
    let label_w = 180.0;
    let row = |ui: &mut egui::Ui, text: &str| {
        ui.add_sized(
            egui::vec2(label_w, 24.0),
            egui::Label::new(t.text(text, type_scale::BODY)),
        );
    };
    ui.add_enabled_ui(owner && app.ready, |ui| {
        ui.horizontal(|ui| {
            row(ui, "Service charge");
            // The typed text is kept between frames so "12." can be edited.
            let id = egui::Id::new("service-rate");
            let mut stored: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| {
                if draft.service_bp == 0 {
                    String::from("0")
                } else {
                    rate_label(draft.service_bp).trim_end_matches('%').to_owned()
                }
            });
            w::text_input(
                ui,
                &mut stored,
                w::FieldOpts {
                    hint: "10",
                    limit: 5,
                    width: Some(90.0),
                    id: Some(id.with("field")),
                    ..Default::default()
                },
            );
            if let Some(bp) = format::parse_money(&stored)
                .ok()
                .and_then(|m| u32::try_from(m.satang()).ok())
            {
                draft.service_bp = bp;
            }
            ui.data_mut(|d| d.insert_temp(id, stored));
            ui.label(t.text("%", type_scale::BODY));
            ui.label(t.muted(
                lang.text("0 = ไม่คิด", "0 = off"),
                type_scale::CAPTION,
            ));
        });
        ui.horizontal(|ui| {
            row(ui, lang.text("ปัดเศษเงินสด", "Cash rounding"));
            w::segmented(
                ui,
                &mut draft.rounding.step,
                &[
                    (0, lang.text("ไม่ปัด", "Off")),
                    (25, "0.25"),
                    (50, "0.50"),
                    (100, lang.text("1 บาท", "1 baht")),
                ],
            );
        });
        if draft.rounding.enabled() {
            ui.horizontal(|ui| {
                row(ui, lang.text("วิธีปัด", "Direction"));
                w::segmented(
                    ui,
                    &mut draft.rounding.mode,
                    &[
                        (
                            pos_domain::RoundMode::Down,
                            lang.text("ปัดลง (ลูกค้าได้ประโยชน์)", "Down (customer's favour)"),
                        ),
                        (
                            pos_domain::RoundMode::Nearest,
                            lang.text("ปัดใกล้สุด", "Nearest"),
                        ),
                    ],
                );
            });
            w::help_text(
                ui,
                lang.text(
                    "ปัดเฉพาะการรับเงินสด โอนและบัตรรับเต็มจำนวนถึงสตางค์ ใบเสร็จแสดงบรรทัดปัดเศษ และ VAT คิดจากยอดที่รับจริง ควรให้นักบัญชียืนยันวิธีที่ร้านใช้",
                    "Cash only; transfers and cards are exact. Receipts show the rounding line and VAT is taken from the amount paid. Confirm the method with your accountant.",
                ),
            );
        }
        let dirty = draft != saved;
        let valid = draft.validate().is_ok();
        if dirty && !valid {
            w::inline_error(
                ui,
                &lang.error(&pos_core::Error::Domain(pos_domain::DomainError::InvalidPricing)),
            );
        }
        ui.horizontal(|ui| {
            if w::button(lang.text("ทิ้งการแก้ไข", "Discard"))
                .ghost()
                .enabled(dirty)
                .show(ui)
                .clicked()
            {
                draft = saved;
                ui.data_mut(|d| d.remove::<String>(egui::Id::new("service-rate")));
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if w::button(lang.text("บันทึก", "Save"))
                    .primary()
                    .enabled(dirty && valid && !app.busy)
                    .show(ui)
                    .clicked()
                {
                    app.send(Command::SetPricing(draft));
                }
            });
        });
    });
    app.pricing_draft = (draft != saved).then_some(draft);
}
