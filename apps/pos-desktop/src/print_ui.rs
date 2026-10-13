//! Receipt printer wiring: settings card, print actions, result handling.
//! Printing never blocks or undoes a sale; failures show a banner with
//! "Print again" (design/docs/04-patterns.md, device errors).
use crate::{
    app::App,
    print::{self, Connection, Job, Outcome, Paper, PrinterSettings, docs},
    ui::{Icon, Theme, space, type_scale, w},
};
use eframe::egui::{self, Align};
use pos_domain::{PaymentMethod, SaleRecord, Shift};

/// A document shown on screen before (or instead of) printing.
pub struct Preview {
    pub title: String,
    pub doc: print::render::Doc,
    /// Receipt to print from the dialog, if this is one.
    pub record: Option<SaleRecord>,
}
#[derive(Default)]
pub struct PrintState {
    pub preview: Option<Preview>,
    /// Rendered preview texture and the key it was rendered for.
    texture: Option<(u64, egui::TextureHandle)>,
    pub settings: PrinterSettings,
    pub draft: PrinterSettings,
    /// printer.json could not be read; it is not overwritten.
    pub readonly: bool,
    running: Option<std::sync::mpsc::Receiver<Outcome>>,
    /// Jobs waiting for the running one (kitchen tickets are never dropped).
    waiting: std::collections::VecDeque<(PrinterSettings, Job)>,
    /// The last job and its printer, kept so a failure can be retried.
    last: Option<(PrinterSettings, Job)>,
    pub failed: Option<String>,
    queues: Vec<String>,
}
impl PrintState {
    pub fn load(dir: &std::path::Path) -> (Self, Option<String>) {
        match PrinterSettings::load(dir) {
            Ok(settings) => (
                Self {
                    draft: settings.clone(),
                    settings,
                    ..Default::default()
                },
                None,
            ),
            Err(e) => (
                Self {
                    readonly: true,
                    ..Default::default()
                },
                Some(e.to_string()),
            ),
        }
    }
    pub fn enabled(&self) -> bool {
        self.settings.enabled()
    }
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }
}
impl Clone for Job {
    fn clone(&self) -> Self {
        Self {
            doc: self.doc.clone(),
            drawer: self.drawer,
            label: self.label.clone(),
        }
    }
}

impl App {
    fn start_print(&mut self, job: Job) {
        if !self.print.enabled() {
            return;
        }
        let device = self.print.settings.clone();
        self.queue_print(device, job);
    }
    /// Runs `job` on `device` now, or after the job in progress.
    fn queue_print(&mut self, device: PrinterSettings, job: Job) {
        if self.print.busy() {
            self.print.waiting.push_back((device, job));
            return;
        }
        self.print.failed = None;
        self.print.last = Some((device.clone(), job.clone()));
        self.print.running = Some(print::spawn(device, job, self.ctx.clone()));
    }
    /// Prints a kitchen ticket where the settings say, if anywhere.
    pub fn print_kitchen(&mut self, ticket: &pos_domain::KitchenTicket) {
        let Some(device) = self.print.settings.kitchen_device() else {
            return;
        };
        let doc = docs::kitchen_ticket(ticket, &self.shop.templates.kitchen, self.lang);
        self.queue_print(
            device,
            Job {
                doc: Some(doc),
                drawer: false,
                label: format!("{} #{}", self.lang.text("ใบครัว", "kitchen"), ticket.number),
            },
        );
    }
    /// A receipt as `template` lays it out. Shops that set a footer in the
    /// older printer settings keep it until the template has its own.
    pub fn receipt_doc(
        &self,
        template: &docs::ReceiptTemplate,
        record: &SaleRecord,
        copy: bool,
    ) -> print::render::Doc {
        let mut t = template.clone();
        if t.footer.trim().is_empty() {
            t.footer = self.print.settings.footer.clone();
        }
        let brand = docs::Brand {
            shop: &self.shop.name,
            logo: self.logo.as_ref(),
        };
        docs::receipt(record, &brand, &t, self.lang, copy)
    }
    /// Check bill for the open bill, priced as checkout will (non-cash total,
    /// with the rounded cash amount noted).
    pub fn check_bill_doc(
        &self,
        template: &docs::ReceiptTemplate,
        cart: &pos_domain::Cart,
    ) -> Option<print::render::Doc> {
        // Priced as a non-cash bill: the exact amount for the QR.
        let price = SnapshotPricing {
            app: self,
            method: pos_domain::PaymentMethod::Transfer,
        }
        .price(cart)?;
        let cash = SnapshotPricing {
            app: self,
            method: pos_domain::PaymentMethod::Cash,
        }
        .price(cart)
        .map(|p| p.total);
        let lines: Vec<docs::BillLine> = cart
            .lines
            .iter()
            .zip(&price.lines)
            .map(|(l, p)| {
                let product = self.known.get(&l.product_id);
                let options = product
                    .and_then(|pr| {
                        pos_domain::price_with_options(pr.price, &pr.options, &l.options).ok()
                    })
                    .map(|(_, o)| o.into_iter().map(|o| o.name).collect())
                    .unwrap_or_default();
                docs::BillLine {
                    name: product.map_or_else(String::new, |p| p.name.clone()),
                    quantity: l.quantity,
                    amount: pos_domain::Money::from_satang(p.gross.satang() - p.discount.satang())
                        .unwrap_or(p.gross),
                    options,
                }
            })
            .collect();
        let promptpay = self
            .shop
            .promptpay_id()
            .map(|id| id.payload(Some(price.total)));
        let brand = docs::Brand {
            shop: &self.shop.name,
            logo: self.logo.as_ref(),
        };
        Some(docs::check_bill(
            &lines,
            &price,
            cash,
            &cart.label,
            &brand,
            template,
            promptpay,
            self.lang,
        ))
    }
    /// Prints a receipt; `copy` marks reprints.
    pub fn print_receipt(&mut self, record: &SaleRecord, copy: bool) {
        let doc = self.receipt_doc(&self.shop.templates.receipt, record, copy);
        self.start_print(Job {
            doc: Some(doc),
            drawer: false,
            label: crate::ui::format::receipt_number(record.sale.number),
        });
    }
    pub fn print_tax_invoice(
        &mut self,
        record: &SaleRecord,
        invoice: &pos_domain::TaxInvoice,
        copy: bool,
    ) {
        let Some(doc) = docs::tax_invoice(record, invoice, self.lang, copy) else {
            return;
        };
        self.start_print(Job {
            doc: Some(doc),
            drawer: false,
            label: format!("TI-{:06}", invoice.number),
        });
    }
    /// After a payment: auto-print and/or open the drawer for cash.
    pub fn after_payment(&mut self, sale: &pos_domain::Sale) {
        if !self.print.enabled() {
            return;
        }
        let s = &self.print.settings;
        let drawer = s.open_drawer && sale.payment_method == PaymentMethod::Cash;
        let label = crate::ui::format::receipt_number(sale.number);
        if s.auto_print {
            let record = SaleRecord {
                sale: sale.clone(),
                cancellation: None,
                refunds: Vec::new(),
                tax_invoice: None,
            };
            let doc = self.receipt_doc(&self.shop.templates.receipt, &record, false);
            self.start_print(Job {
                doc: Some(doc),
                drawer,
                label,
            });
        } else if drawer {
            self.start_print(Job {
                doc: None,
                drawer: true,
                label,
            });
        }
    }
    pub fn print_shift(&mut self, shift: &Shift) {
        let number = self.data.shift_numbers.get(&shift.id).copied();
        let doc = docs::shift_summary(shift, number, &self.shop.name, self.lang);
        self.start_print(Job {
            doc: Some(doc),
            drawer: false,
            label: number.map_or_else(
                || self.lang.text("สรุปกะ", "shift summary").to_owned(),
                |n| format!("{} #{n}", self.lang.text("สรุปกะ", "shift summary")),
            ),
        });
    }
    /// Polls the running job; call once per frame.
    pub fn print_events(&mut self) {
        let Some(rx) = &self.print.running else {
            return;
        };
        let Ok(outcome) = rx.try_recv() else {
            return;
        };
        self.print.running = None;
        if let Some((device, job)) = self.print.waiting.pop_front() {
            self.queue_print(device, job);
        }
        match outcome {
            Outcome::Printed(label) => {
                self.toast(format!("{} {label}", self.lang.text("พิมพ์แล้ว", "Printed")));
            }
            Outcome::Failed { label, error } => {
                self.print.failed = Some(format!(
                    "{} {label} · {} ({error})",
                    self.lang.text("พิมพ์ไม่สำเร็จ", "Could not print"),
                    self.lang.text(
                        "ข้อมูลบันทึกแล้ว ตรวจกระดาษ สาย และการตั้งค่าเครื่องพิมพ์",
                        "your data is saved. Check paper, cable and printer settings",
                    ),
                ));
            }
        }
    }
    fn retry_print(&mut self) {
        if let Some((device, job)) = self.print.last.clone() {
            self.queue_print(device, job);
        }
    }
}

/// Failure banner with "Print again". Returns true when shown.
pub fn banner(app: &mut App, ui: &mut egui::Ui) -> bool {
    let Some(message) = app.print.failed.clone() else {
        return false;
    };
    let lang = app.lang;
    let t = Theme::of(ui);
    ui.horizontal_top(|ui| {
        let side = t.m.control_md;
        let width = ui.available_width() - side - space::S2;
        ui.allocate_ui_with_layout(
            egui::vec2(width, 0.0),
            egui::Layout::top_down(Align::Min),
            |ui| {
                if w::banner(
                    ui,
                    w::Tone::Warning,
                    &message,
                    Some(lang.text("พิมพ์ซ้ำ", "Print again")),
                ) {
                    app.retry_print();
                }
            },
        );
        if w::icon_button(ui, Icon::Close, lang.text("ปิดข้อความ", "Dismiss")).clicked()
        {
            app.print.failed = None;
        }
    });
    true
}

/// "Print receipt" / "Print copy" button, shown only with a printer set up.
pub fn receipt_button(app: &mut App, ui: &mut egui::Ui, record: &SaleRecord, copy: bool) {
    // The printed layout can always be seen, with or without a printer.
    if w::button(app.lang.text("ดูใบพิมพ์", "Print preview"))
        .ghost()
        .show(ui)
        .on_hover_text(
            app.lang
                .text("ดูหน้าตาใบเสร็จตามรูปแบบที่ตั้งไว้", "See the receipt as it prints"),
        )
        .clicked()
    {
        app.preview_receipt(record);
    }
    if !app.print.enabled() {
        return;
    }
    let label = if copy {
        app.lang.text("พิมพ์สำเนา", "Print copy")
    } else {
        app.lang.text("พิมพ์ใบเสร็จ", "Print receipt")
    };
    if w::button(label)
        .icon(Icon::Printer)
        .loading(app.print.busy())
        .show(ui)
        .clicked()
    {
        app.print_receipt(record, copy);
    }
}

/// Settings card. Changes save explicitly, like the shop profile.
pub fn settings_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("เครื่องพิมพ์ใบเสร็จ", "Receipt printer"));
    w::help_text(
        ui,
        lang.text(
            "เครื่องพิมพ์ความร้อน ESC/POS 58 หรือ 80 มม. ภาษาไทยพิมพ์เป็นภาพ จึงใช้ได้แม้เครื่องไม่มีฟอนต์ไทย ตั้งค่าเฉพาะเครื่องนี้",
            "ESC/POS thermal printers, 58 or 80 mm. Thai prints as an image, so printers without Thai fonts work. Saved for this device only.",
        ),
    );
    if app.print.readonly {
        w::banner(
            ui,
            w::Tone::Danger,
            lang.text(
                "อ่าน printer.json ไม่ได้ ไฟล์จะไม่ถูกเขียนทับ",
                "Cannot read printer.json. It will not be overwritten.",
            ),
            None,
        );
        return;
    }
    let label_w = 160.0;
    let row_label = |ui: &mut egui::Ui, text: &str| {
        ui.add_sized(
            egui::vec2(label_w, 24.0),
            egui::Label::new(t.text(text, type_scale::BODY)),
        );
    };
    let mut connection = app.print.draft.connection;
    ui.horizontal_wrapped(|ui| {
        row_label(ui, lang.text("การเชื่อมต่อ", "Connection"));
        let mut options = vec![
            (Connection::None, lang.text("ไม่ใช้", "None")),
            (
                Connection::Network,
                lang.text("เครือข่าย (IP)", "Network (IP)"),
            ),
        ];
        if print::system_supported() {
            options.push((
                Connection::System,
                lang.text("เครื่องพิมพ์ในระบบ", "System printer"),
            ));
        }
        if w::segmented(ui, &mut connection, &options) {
            app.print.draft.connection = connection;
            if connection == Connection::System && app.print.queues.is_empty() {
                app.print.queues = print::system_queues();
            }
        }
    });
    match app.print.draft.connection {
        Connection::None => {}
        Connection::Network => {
            let invalid = !app.print.draft.address.trim().is_empty()
                && print::parse_address(&app.print.draft.address).is_err();
            ui.horizontal(|ui| {
                row_label(ui, lang.text("ที่อยู่เครื่องพิมพ์", "Printer address"));
                w::text_input(
                    ui,
                    &mut app.print.draft.address,
                    w::FieldOpts {
                        hint: "192.168.1.100",
                        limit: 253,
                        invalid,
                        width: Some(ui.available_width().min(260.0)),
                        ..Default::default()
                    },
                );
            });
            w::help_text(
                ui,
                lang.text(
                    "IP ของเครื่องพิมพ์ในวงแลนเดียวกัน พอร์ตเริ่มต้น 9100 (ใส่ :พอร์ต ถ้าต่างจากนี้) ใช้เฉพาะในร้าน ไม่ส่งข้อมูลออกอินเทอร์เน็ต",
                    "The printer's IP on the shop network; port 9100 unless you add :port. Stays on the local network.",
                ),
            );
        }
        Connection::System => {
            ui.horizontal_wrapped(|ui| {
                row_label(ui, lang.text("เครื่องพิมพ์", "Printer"));
                let queues = app.print.queues.clone();
                if queues.is_empty() {
                    ui.label(t.muted(
                        lang.text("ไม่พบเครื่องพิมพ์ในระบบ", "No printers found"),
                        type_scale::BODY,
                    ));
                }
                for q in queues {
                    let selected = app.print.draft.queue == q;
                    if w::chip(ui, &q, selected, false).clicked() {
                        app.print.draft.queue = q;
                    }
                }
                if w::button(lang.text("ค้นหาอีกครั้ง", "Refresh"))
                    .ghost()
                    .size(w::Size::Sm)
                    .show(ui)
                    .clicked()
                {
                    app.print.queues = print::system_queues();
                }
            });
            w::help_text(
                ui,
                lang.text(
                    "เพิ่มเครื่องพิมพ์ใน System Settings ของเครื่องก่อน แอปส่งข้อมูลแบบ raw ให้เครื่องพิมพ์โดยตรง",
                    "Add the printer in the operating system first; the app sends raw ESC/POS to it.",
                ),
            );
        }
    }
    if app.print.draft.connection != Connection::None {
        ui.horizontal(|ui| {
            row_label(ui, lang.text("ขนาดกระดาษ", "Paper"));
            w::segmented(
                ui,
                &mut app.print.draft.paper,
                &[(Paper::Mm58, "58 mm"), (Paper::Mm80, "80 mm")],
            );
        });
        let switch = |ui: &mut egui::Ui, value: &mut bool, title: &str, help: &str| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - 60.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(t.text(title, type_scale::BODY_STRONG));
                    w::help_text(ui, help);
                });
                w::switch(ui, value, title);
            });
        };
        switch(
            ui,
            &mut app.print.draft.auto_print,
            lang.text("พิมพ์ใบเสร็จอัตโนมัติ", "Print receipts automatically"),
            lang.text(
                "พิมพ์ทันทีหลังรับเงินทุกบิล ถ้าปิดไว้ กดพิมพ์เองจากหน้ารับเงินหรือประวัติขาย",
                "Print right after each payment. Otherwise print from the paid screen or history.",
            ),
        );
        switch(
            ui,
            &mut app.print.draft.open_drawer,
            lang.text("เปิดลิ้นชักเมื่อรับเงินสด", "Open the drawer for cash"),
            lang.text(
                "สำหรับลิ้นชักที่ต่อสายเข้าเครื่องพิมพ์ (ช่อง RJ11)",
                "For a cash drawer plugged into the printer (RJ11 port).",
            ),
        );
        w::field_label(
            ui,
            lang.text("ข้อความท้ายใบเสร็จ", "Receipt footer"),
            Some(lang.text("ไม่บังคับ", "optional")),
        );
        w::text_input(
            ui,
            &mut app.print.draft.footer,
            w::FieldOpts {
                hint: lang.text("เช่น ขอบคุณที่อุดหนุน", "e.g. Thank you!"),
                limit: 200,
                ..Default::default()
            },
        );
    }
    if !app.data.tables.is_empty() {
        kitchen_section(app, ui, label_w);
    }
    let dirty = app.print.draft != app.print.settings;
    let valid = app.print.draft.validate().is_ok();
    ui.add_space(space::S1);
    ui.horizontal_wrapped(|ui| {
        let can_test = app.print.enabled() && !dirty;
        if w::button(lang.text("พิมพ์ทดสอบ", "Print test page"))
            .icon(Icon::Printer)
            .enabled(can_test && !app.print.busy())
            .loading(app.print.busy())
            .show(ui)
            .on_disabled_hover_text(lang.text(
                "บันทึกการตั้งค่าก่อนพิมพ์ทดสอบ",
                "Save the settings before printing a test page",
            ))
            .clicked()
        {
            let paper = match app.print.settings.paper {
                Paper::Mm58 => "58 mm",
                Paper::Mm80 => "80 mm",
            };
            let doc = docs::test_page(&app.shop.name, paper, lang);
            app.start_print(Job {
                doc: Some(doc),
                drawer: false,
                label: lang.text("หน้าทดสอบ", "test page").to_owned(),
            });
        }
        if w::button(lang.text("บันทึกตัวอย่างเป็นภาพ", "Save preview image"))
            .ghost()
            .enabled(app.print.draft.connection != Connection::None)
            .show(ui)
            .clicked()
        {
            save_preview(app);
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
                .enabled(dirty && valid)
                .show(ui)
                .clicked()
            {
                match app.print.draft.save(&app.data_dir) {
                    Ok(()) => {
                        app.print.settings = app.print.draft.clone();
                        app.toast(lang.text("บันทึกการตั้งค่าเครื่องพิมพ์แล้ว", "Printer settings saved."));
                    }
                    Err(e) => app.error = Some(e.to_string()),
                }
            }
            if w::button(lang.text("ทิ้งการแก้ไข", "Discard"))
                .ghost()
                .enabled(dirty)
                .show(ui)
                .clicked()
            {
                app.print.draft = app.print.settings.clone();
            }
        });
    });
}
/// Saves the latest receipt (or a test page) as PNG in the data folder.
fn save_preview(app: &mut App) {
    let lang = app.lang;
    let doc = match app.data.sales.first() {
        Some(record) => app.receipt_doc(&app.shop.templates.receipt, record, false),
        None => docs::test_page(&app.shop.name, "", lang),
    };
    let path = app.stamped("receipt-preview", "png");
    match print::save_preview(&app.print.draft, &doc, &path) {
        Ok(()) => app.toast(format!(
            "{} {}",
            lang.text("บันทึกภาพตัวอย่างแล้ว:", "Preview saved:"),
            path.display()
        )),
        Err(e) => app.error = Some(e.to_string()),
    }
}

/// Where kitchen tickets print: nowhere, the receipt printer, or their own.
fn kitchen_section(app: &mut App, ui: &mut egui::Ui, label_w: f32) {
    use crate::print::KitchenMode;
    let t = Theme::of(ui);
    let lang = app.lang;
    let row_label = |ui: &mut egui::Ui, text: &str| {
        ui.add_sized(
            egui::vec2(label_w, 24.0),
            egui::Label::new(t.text(text, type_scale::BODY)),
        );
    };
    ui.add_space(space::S2);
    w::field_label(ui, lang.text("ใบครัว", "Kitchen tickets"), None);
    ui.push_id("kitchen-printer", |ui| {
        ui.horizontal_wrapped(|ui| {
            row_label(ui, lang.text("พิมพ์ที่", "Print on"));
            w::segmented(
                ui,
                &mut app.print.draft.kitchen.mode,
                &[
                    (KitchenMode::Off, lang.text("ไม่พิมพ์", "Off")),
                    (KitchenMode::Receipt, lang.text("เครื่องพิมพ์ใบเสร็จ", "Receipt printer")),
                    (KitchenMode::Own, lang.text("เครื่องพิมพ์ครัว", "Kitchen printer")),
                ],
            );
        });
        if app.print.draft.kitchen.mode == KitchenMode::Own {
            ui.horizontal_wrapped(|ui| {
                row_label(ui, lang.text("การเชื่อมต่อ", "Connection"));
                let mut options = vec![(Connection::Network, lang.text("เครือข่าย (IP)", "Network (IP)"))];
                if print::system_supported() {
                    options.push((Connection::System, lang.text("เครื่องพิมพ์ในระบบ", "System printer")));
                }
                if app.print.draft.kitchen.connection == Connection::None {
                    app.print.draft.kitchen.connection = Connection::Network;
                }
                w::segmented(ui, &mut app.print.draft.kitchen.connection, &options);
            });
            ui.horizontal(|ui| {
                if app.print.draft.kitchen.connection == Connection::System {
                    row_label(ui, lang.text("ชื่อคิวเครื่องพิมพ์", "Queue name"));
                    w::text_input(
                        ui,
                        &mut app.print.draft.kitchen.queue,
                        w::FieldOpts {
                            hint: "Kitchen_Printer",
                            limit: 120,
                            width: Some(ui.available_width().min(260.0)),
                            ..Default::default()
                        },
                    );
                } else {
                    row_label(ui, lang.text("ที่อยู่เครื่องพิมพ์", "Printer address"));
                    let invalid = !app.print.draft.kitchen.address.trim().is_empty()
                        && print::parse_address(&app.print.draft.kitchen.address).is_err();
                    w::text_input(
                        ui,
                        &mut app.print.draft.kitchen.address,
                        w::FieldOpts {
                            hint: "192.168.1.101",
                            limit: 253,
                            invalid,
                            width: Some(ui.available_width().min(260.0)),
                            ..Default::default()
                        },
                    );
                }
            });
            ui.horizontal(|ui| {
                row_label(ui, lang.text("ขนาดกระดาษ", "Paper"));
                w::segmented(
                    ui,
                    &mut app.print.draft.kitchen.paper,
                    &[(Paper::Mm58, "58 mm"), (Paper::Mm80, "80 mm")],
                );
            });
        }
        w::help_text(
            ui,
            lang.text(
                "พิมพ์ทุกครั้งที่กดส่งครัว และใบยกเลิกเมื่อลดหรือลบรายการที่ส่งแล้ว จอครัวแสดงเหมือนกันเสมอ",
                "Prints on every send, plus cancellation slips when sent dishes are reduced or removed. The kitchen screen always shows them.",
            ),
        );
    });
}

/// Prices the open bill as if paid by `method` (rounding is cash-only).
struct SnapshotPricing<'a> {
    app: &'a App,
    method: pos_domain::PaymentMethod,
}
impl SnapshotPricing<'_> {
    fn price(&mut self, cart: &pos_domain::Cart) -> Option<pos_domain::BillPrice> {
        let lines = cart
            .lines
            .iter()
            .map(|line| {
                let product = self.app.known.get(&line.product_id)?;
                Some((
                    product
                        .unit_price(&line.options)
                        .ok()?
                        .checked_mul(line.quantity)
                        .ok()?,
                    line.discount,
                    product.service_charge,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let options = pos_domain::PriceOptions::for_bill(
            &self.app.data.pricing,
            &self.app.data.tax,
            cart.service_charge,
            self.method == pos_domain::PaymentMethod::Cash,
        );
        pos_domain::price_sale(&lines, cart.discount, &options).ok()
    }
}

/// Paper width shown in previews: the receipt printer's, else 80 mm.
pub fn preview_paper(app: &App) -> Paper {
    if app.print.settings.connection == Connection::None {
        app.print.draft.paper
    } else {
        app.print.settings.paper
    }
}
/// Draws `doc` as it will print, on a white paper card `width` points wide.
/// The texture is re-rendered only when `key` changes.
pub fn paper(app: &mut App, ui: &mut egui::Ui, doc: &print::render::Doc, key: u64, width: f32) {
    let t = Theme::of(ui);
    let dots = preview_paper(app).dots();
    let key = key ^ u64::from(dots).rotate_left(17);
    if app.print.texture.as_ref().is_none_or(|(k, _)| *k != key) {
        let Some(img) = print::render::render(doc, dots) else {
            w::inline_error(ui, "fonts");
            return;
        };
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_gray(size, img.as_raw());
        let tex = ui
            .ctx()
            .load_texture("print-preview", color, egui::TextureOptions::LINEAR);
        app.print.texture = Some((key, tex));
    }
    let Some((_, tex)) = &app.print.texture else {
        return;
    };
    let [w, h] = tex.size();
    // `width` is the outer width; the frame adds its margin and stroke.
    let width = (width - space::S3 * 2.0 - 2.0).max(1.0);
    let scale = width / w as f32;
    let size = egui::vec2(width, h as f32 * scale);
    egui::Frame::new()
        .fill(crate::ui::tokens::primitive::STONE_0)
        .stroke(egui::Stroke::new(1.0, t.p.border_default))
        .shadow(t.shadow(1))
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(ui, |ui| {
            ui.add(egui::Image::new((tex.id(), size)));
        });
}
/// Stable key for a preview: hashes the document's debug form.
pub fn doc_key(doc: &print::render::Doc) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{doc:?}").hash(&mut h);
    h.finish()
}
/// "Preview" dialog: the document as printed, with Print when a printer is set.
pub fn preview_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(preview) = app.print.preview.take() else {
        return;
    };
    let lang = app.lang;
    let key = doc_key(&preview.doc);
    let mut keep = true;
    let response = w::dialog(ctx, "print-preview", 440.0, |ui| {
        w::dialog_title(ui, &preview.title, false);
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 220.0).max(240.0))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| paper(app, ui, &preview.doc, key, 346.0));
            });
        w::dialog_actions(ui, |ui| {
            if app.print.enabled()
                && w::button(lang.text("พิมพ์", "Print"))
                    .icon(Icon::Printer)
                    .primary()
                    .loading(app.print.busy())
                    .show(ui)
                    .clicked()
            {
                match &preview.record {
                    Some(record) => app.print_receipt(record, true),
                    None => app.start_doc(preview.title.clone(), preview.doc.clone()),
                }
                keep = false;
            }
            if w::button(lang.text("ปิด", "Close"))
                .ghost()
                .show(ui)
                .clicked()
            {
                keep = false;
            }
        });
    });
    if keep && !response.should_close() {
        app.print.preview = Some(preview);
    }
}
impl App {
    /// Prints an already built document on the receipt printer.
    pub fn start_doc(&mut self, label: String, doc: print::render::Doc) {
        self.start_print(Job {
            doc: Some(doc),
            drawer: false,
            label,
        });
    }
    /// Opens the preview of the open bill's check bill.
    pub fn preview_check_bill(&mut self) {
        let template = self.shop.templates.receipt.clone();
        if let Some(doc) = self.check_bill_doc(&template, &self.cart) {
            self.print.preview = Some(Preview {
                title: self.lang.text("ใบแจ้งยอด", "Check bill").to_owned(),
                doc,
                record: None,
            });
        }
    }
    /// Opens the preview of a receipt as it would print.
    pub fn preview_receipt(&mut self, record: &SaleRecord) {
        let doc = self.receipt_doc(&self.shop.templates.receipt, record, false);
        self.print.preview = Some(Preview {
            title: format!(
                "{} {}",
                self.lang.text("ตัวอย่างใบเสร็จ", "Receipt preview"),
                crate::ui::format::receipt_number(record.sale.number)
            ),
            doc,
            record: Some(record.clone()),
        });
    }
}
