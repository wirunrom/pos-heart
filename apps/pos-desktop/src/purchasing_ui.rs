//! Supplier orders and explicit partial receiving. Drafts are kept across navigation/locking.
use crate::{
    app::App,
    ui::{Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align};
use pos_domain::{
    CancelPurchaseRequest, Money, Permission, Product, PurchaseItemInput, PurchaseLine,
    PurchaseOrder, PurchaseOrderRequest, PurchaseReceipt, PurchaseReceiveItem, PurchaseStatus,
    PurchaseTarget, ReceivePurchaseRequest, Supplier, purchase_cost, purchase_receive_cost,
};
use uuid::Uuid;

#[derive(Default, Clone, Copy, PartialEq)]
enum Tab {
    #[default]
    Orders,
    Suppliers,
}
#[derive(Default)]
pub struct PurchasingState {
    pub open: bool,
    pub loaded: bool,
    pub suppliers: Vec<Supplier>,
    pub orders: Vec<PurchaseOrder>,
    pub products: Vec<Product>,
    pub product_query: String,
    pub offset: u32,
    pub selected: Option<Detail>,
    pub editor: Option<OrderEditor>,
    pub supplier_editor: Option<Supplier>,
    pub receive: Option<ReceiveEditor>,
    tab: Tab,
    cancel: Option<CancelEditor>,
    discard: bool,
}
pub struct Detail {
    pub order: PurchaseOrder,
    pub receipts: Vec<PurchaseReceipt>,
    pub receipt_offset: u32,
}
pub struct OrderEditor {
    id: Uuid,
    supplier: Option<Uuid>,
    lines: Vec<LineDraft>,
    note: String,
    search: String,
    confirm: bool,
}
struct LineDraft {
    target: PurchaseTarget,
    name: String,
    unit: String,
    quantity: String,
    cost: String,
}
pub struct ReceiveEditor {
    id: Uuid,
    order: PurchaseOrder,
    quantities: Vec<String>,
    note: String,
    confirm: bool,
}
struct CancelEditor {
    id: Uuid,
    order: PurchaseOrder,
    reason: String,
    confirm: bool,
}
impl Default for OrderEditor {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            supplier: None,
            lines: Vec::new(),
            note: String::new(),
            search: String::new(),
            confirm: false,
        }
    }
}
impl PurchasingState {
    pub fn dirty(&self) -> bool {
        self.editor.is_some()
            || self.supplier_editor.is_some()
            || self.receive.is_some()
            || self.cancel.is_some()
    }
    pub fn cancel_confirmations(&mut self) {
        if let Some(e) = &mut self.editor {
            e.confirm = false
        }
        if let Some(e) = &mut self.receive {
            e.confirm = false
        }
        if let Some(e) = &mut self.cancel {
            e.confirm = false
        }
        self.discard = false;
    }
    pub fn set_detail(
        &mut self,
        order: PurchaseOrder,
        receipts: Vec<PurchaseReceipt>,
        receipt_offset: u32,
    ) {
        if let Some(e) = &mut self.receive
            && e.order.id == order.id
            && e.order.revision != order.revision
        {
            e.order = order.clone();
            e.id = Uuid::new_v4();
            e.confirm = false;
            e.quantities.resize(order.items.len(), String::new());
        }
        if let Some(e) = &mut self.cancel
            && e.order.id == order.id
            && e.order.revision != order.revision
        {
            e.order = order.clone();
            e.id = Uuid::new_v4();
            e.confirm = false;
        }
        self.selected = Some(Detail {
            order,
            receipts,
            receipt_offset,
        });
    }
    pub fn on_supplier_saved(&mut self, supplier: Supplier) {
        self.suppliers.retain(|s| s.id != supplier.id);
        self.suppliers.push(supplier);
        self.supplier_editor = None;
        self.discard = false;
    }
    pub fn on_order_created(&mut self, order: PurchaseOrder) {
        self.editor = None;
        self.on_order_updated(order);
    }
    pub fn on_order_updated(&mut self, order: PurchaseOrder) {
        self.orders.retain(|o| o.id != order.id);
        self.orders.insert(0, order.clone());
        self.orders.truncate(50);
        self.selected = Some(Detail {
            order,
            receipts: Vec::new(),
            receipt_offset: 0,
        });
        self.receive = None;
        self.cancel = None;
        self.discard = false;
    }
}
fn allowed(app: &App, permission: Permission) -> bool {
    crate::staff_ui::access(app, permission) != pos_domain::Access::Deny
}
fn status(lang: crate::i18n::Language, s: PurchaseStatus) -> &'static str {
    match s {
        PurchaseStatus::Open => lang.text("รอรับของ", "Open"),
        PurchaseStatus::Partial => lang.text("รับบางส่วน", "Part received"),
        PurchaseStatus::Received => lang.text("รับครบแล้ว", "Received"),
        PurchaseStatus::Cancelled => lang.text("ยกเลิกส่วนที่เหลือ", "Remainder cancelled"),
    }
}
fn total_text(value: Result<Money, pos_domain::DomainError>) -> String {
    value.map(format::baht).unwrap_or_else(|_| "—".into())
}
fn scroll_height(ctx: &egui::Context, t: &Theme) -> f32 {
    (ctx.content_rect().height() - t.m.control_lg * 6.0 - space::S6 * 4.0).max(t.m.control_lg * 3.0)
}
fn input(ui: &mut egui::Ui, label: &str, value: &mut String, limit: usize) {
    w::field_label(ui, label, None);
    w::text_input(
        ui,
        value,
        w::FieldOpts {
            limit,
            ..Default::default()
        },
    );
}
fn money(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    id: impl std::hash::Hash + std::fmt::Debug,
) {
    w::field_label(ui, label, None);
    w::money_input(
        ui,
        value,
        w::MoneyOpts {
            id: egui::Id::new(id),
            primary: false,
            width: None,
            large: false,
            invalid: !value.trim().is_empty() && format::parse_money(value).is_err(),
            enabled: true,
        },
    );
}
fn safety(app: &App, ui: &mut egui::Ui) {
    w::help_text(ui,app.lang.text("รับของเพิ่มสต็อกเท่านั้น ไม่บันทึกจ่ายเงิน ภาษี เจ้าหนี้ หรือเปลี่ยนต้นทุนมาตรฐาน","Receiving adds stock only; it does not record payment, VAT, payables or change standard costs."));
}
fn error(app: &App, ui: &mut egui::Ui) {
    if let Some(e) = &app.error {
        w::inline_error(ui, e)
    }
}

pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    if !allowed(app, Permission::Catalog) && !allowed(app, Permission::Stock) {
        w::state_view(
            ui,
            crate::ui::Icon::Products,
            app.lang
                .text("ไม่มีสิทธิ์ดูการจัดซื้อ", "Purchasing access required"),
            app.lang.text(
                "ต้องมีสิทธิ์จัดการสินค้าหรือสต็อก",
                "Catalog or Stock access is required.",
            ),
        );
        return;
    }
    let lang = app.lang;
    let t = Theme::of(ui);
    if !app.purchasing.loaded && !app.busy {
        app.purchasing.loaded = true;
        app.send(Command::LoadPurchases(app.purchasing.offset));
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(t.text(
            lang.text("จัดซื้อและรับของ", "Purchasing & receiving"),
            type_scale::TITLE,
        ));
        if w::button(lang.text("โหลดใหม่", "Refresh"))
            .enabled(!app.busy)
            .show(ui)
            .clicked()
        {
            app.send(Command::LoadPurchases(app.purchasing.offset));
        }
    });
    w::segmented(
        ui,
        &mut app.purchasing.tab,
        &[
            (Tab::Orders, lang.text("ใบสั่งซื้อ", "Orders")),
            (Tab::Suppliers, lang.text("ผู้ขาย", "Suppliers")),
        ],
    );
    let may = allowed(app, Permission::Catalog);
    match app.purchasing.tab {
        Tab::Suppliers => {
            if may
                && w::button(lang.text("เพิ่มผู้ขาย", "Add supplier"))
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
            {
                app.error = None;
                app.purchasing.supplier_editor = Some(Supplier {
                    id: Uuid::new_v4(),
                    name: String::new(),
                    contact: String::new(),
                    note: String::new(),
                    active: true,
                });
            }
            let suppliers = app.purchasing.suppliers.clone();
            if suppliers.is_empty() {
                w::state_view(
                    ui,
                    crate::ui::Icon::Products,
                    lang.text("ยังไม่มีผู้ขาย", "No suppliers yet"),
                    lang.text(
                        "เพิ่มชื่อผู้ขายก่อนสร้างใบสั่งซื้อ",
                        "Add a supplier before creating an order.",
                    ),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("suppliers")
                .show(ui, |ui| {
                    for s in suppliers {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                w::name_label(ui, &s.name, type_scale::BODY_STRONG, t.p.fg_default);
                                if !s.contact.is_empty() {
                                    ui.label(t.muted(&s.contact, type_scale::CAPTION));
                                }
                                if !s.note.is_empty() {
                                    ui.label(t.muted(&s.note, type_scale::CAPTION));
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                if may
                                    && w::button(lang.text("แก้ไข", "Edit"))
                                        .ghost()
                                        .show(ui)
                                        .clicked()
                                {
                                    app.error = None;
                                    app.purchasing.supplier_editor = Some(s.clone());
                                }
                                if !s.active {
                                    w::badge(ui, lang.text("ปิดใช้", "Inactive"), w::Tone::Neutral);
                                }
                            });
                        });
                        w::divider(ui);
                    }
                });
        }
        Tab::Orders => {
            ui.horizontal_wrapped(|ui| {
                if may
                    && w::button(lang.text("สร้างใบสั่งซื้อ", "Create order"))
                        .enabled(!app.busy)
                        .show(ui)
                        .clicked()
                {
                    app.error = None;
                    app.purchasing.editor = Some(OrderEditor::default());
                    app.purchasing.product_query.clear();
                    app.send(Command::PurchaseProducts(String::new()));
                }
                w::help_text(
                    ui,
                    lang.text(
                        "สร้างใบสั่งซื้อยังไม่เพิ่มสต็อก รับของจริงทีละส่วนได้",
                        "Orders do not change stock. Receive actual deliveries in parts.",
                    ),
                );
            });
            let orders = app.purchasing.orders.clone();
            if orders.is_empty() {
                w::state_view(
                    ui,
                    crate::ui::Icon::Products,
                    lang.text("ยังไม่มีใบสั่งซื้อ", "No orders yet"),
                    lang.text(
                        "เพิ่มผู้ขาย แล้วเลือกสินค้าหรือวัตถุดิบที่ต้องการสั่ง",
                        "Add a supplier, then choose products or ingredients to order.",
                    ),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("purchase-orders")
                .show(ui, |ui| {
                    for o in orders {
                        ui.horizontal(|ui| {
                            let name_w = (ui.available_width() * 0.45).max(t.m.control_lg * 3.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(name_w, 0.0),
                                egui::Layout::top_down(Align::Min),
                                |ui| {
                                    w::name_label(
                                        ui,
                                        &o.supplier.name,
                                        type_scale::BODY_STRONG,
                                        t.p.fg_default,
                                    );
                                    ui.label(t.muted(
                                        format::date_time(lang, o.created_at),
                                        type_scale::CAPTION,
                                    ));
                                    w::badge(ui, status(lang, o.status()), w::Tone::Neutral);
                                    ui.label(t.muted(o.id.to_string(), type_scale::CAPTION));
                                },
                            );
                            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                if w::button(lang.text("รายละเอียด", "Details"))
                                    .enabled(!app.busy)
                                    .show(ui)
                                    .clicked()
                                {
                                    app.error = None;
                                    app.send(Command::PurchaseDetail(o.id, 0));
                                }
                                ui.add(
                                    egui::Label::new(
                                        t.text(total_text(o.total()), type_scale::MONEY),
                                    )
                                    .extend(),
                                );
                            });
                        });
                        w::divider(ui);
                    }
                });
            pager(app, ui, false);
        }
    }
}
fn pager(app: &mut App, ui: &mut egui::Ui, receipt: bool) {
    let lang = app.lang;
    let (offset, len, id) = if receipt {
        match &app.purchasing.selected {
            Some(d) => (d.receipt_offset, d.receipts.len(), Some(d.order.id)),
            None => return,
        }
    } else {
        (app.purchasing.offset, app.purchasing.orders.len(), None)
    };
    ui.horizontal(|ui| {
        let mut next = None;
        if w::button(lang.text("ใหม่กว่า", "Newer"))
            .ghost()
            .enabled(offset > 0 && !app.busy)
            .show(ui)
            .clicked()
        {
            next = Some(offset.saturating_sub(50));
        }
        ui.label(format!(
            "{}–{}",
            offset + u32::from(len > 0),
            offset + len as u32
        ));
        if w::button(lang.text("เก่ากว่า", "Older"))
            .ghost()
            .enabled(len == 50 && !app.busy)
            .show(ui)
            .clicked()
        {
            next = Some(offset.saturating_add(50));
        }
        if let Some(n) = next {
            match id {
                Some(id) => app.send(Command::PurchaseDetail(id, n)),
                None => {
                    app.purchasing.offset = n;
                    app.send(Command::LoadPurchases(n));
                }
            }
        }
    });
}
fn line_summary(ui: &mut egui::Ui, lang: crate::i18n::Language, l: &PurchaseLine) {
    let t = Theme::of(ui);
    w::name_label(ui, &l.name, type_scale::BODY_STRONG, t.p.fg_default);
    ui.horizontal_wrapped(|ui| {
        ui.label(t.muted(
            format!(
                "{} {} · {} {} · {} {} {}",
                lang.text("สั่ง", "Ordered"),
                format::qty(l.quantity),
                lang.text("รับแล้ว", "Received"),
                format::qty(l.received),
                lang.text("เหลือ", "Remaining"),
                format::qty(l.quantity - l.received),
                l.unit
            ),
            type_scale::CAPTION,
        ));
    });
    w::kv(
        ui,
        &format!("{} / {}", lang.text("ราคาต่อหน่วย", "Unit cost"), l.unit),
        &format::baht(l.unit_cost),
        w::Value::Plain,
    );
    w::divider(ui);
}
pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    if !allowed(app, Permission::Catalog) && !allowed(app, Permission::Stock) {
        return;
    }
    if app.purchasing.supplier_editor.is_some() && allowed(app, Permission::Catalog) {
        supplier_dialog(app, ctx);
        return;
    }
    if app.purchasing.editor.is_some() && allowed(app, Permission::Catalog) {
        order_dialog(app, ctx);
        return;
    }
    if app.purchasing.receive.is_some() && allowed(app, Permission::Stock) {
        receive_dialog(app, ctx);
        return;
    }
    if app.purchasing.cancel.is_some() && allowed(app, Permission::Catalog) {
        cancel_dialog(app, ctx);
        return;
    }
    detail_dialog(app, ctx);
}
fn detail_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(d) = app.purchasing.selected.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let o = &d.order;
    let mut intent = 0;
    let response = w::dialog(ctx, "purchase-detail", t.m.control_lg * 15.0, |ui| {
        w::dialog_title(ui, lang.text("ใบสั่งซื้อ", "Purchase order"), false);
        ui.label(t.text(&o.supplier.name, type_scale::BODY_STRONG));
        ui.label(t.muted(format::date_time(lang, o.created_at), type_scale::CAPTION));
        w::badge(ui, status(lang, o.status()), w::Tone::Neutral);
        egui::ScrollArea::vertical()
            .id_salt("purchase-detail-body")
            .max_height(scroll_height(ctx, &t))
            .show(ui, |ui| {
                if !o.note.is_empty() {
                    ui.label(&o.note);
                }
                for l in &o.items {
                    line_summary(ui, lang, l)
                }
                w::kv(
                    ui,
                    lang.text("ยอดสั่งซื้อ", "Order total"),
                    &total_text(o.total()),
                    w::Value::Plain,
                );

                w::kv(
                    ui,
                    lang.text("มูลค่ารับสะสม", "Total received value"),
                    &total_text(o.received_total()),
                    w::Value::Plain,
                );
                if let Some(cancel) = &o.cancellation {
                    w::help_text(
                        ui,
                        &format!(
                            "{} · {}",
                            format::date_time(lang, cancel.created_at),
                            cancel.request.reason
                        ),
                    );
                }
                w::title(ui, lang.text("ประวัติรับของ", "Receiving history"));
                if d.receipts.is_empty() {
                    w::help_text(
                        ui,
                        lang.text("ยังไม่มีประวัติในหน้านี้", "No receipts on this page."),
                    );
                }
                for r in &d.receipts {
                    ui.label(t.text(
                        format::date_time(lang, r.created_at),
                        type_scale::BODY_STRONG,
                    ));
                    for l in &r.lines {
                        w::name_label(
                            ui,
                            &format!("{} · {} {}", l.name, format::qty(l.quantity), l.unit),
                            type_scale::BODY,
                            t.p.fg_default,
                        );
                        w::kv(
                            ui,
                            lang.text("มูลค่ารับ", "Received value"),
                            &format::baht(l.total),
                            w::Value::Plain,
                        );
                    }
                    w::kv(
                        ui,
                        lang.text("รวมครั้งนี้", "Delivery total"),
                        &format::baht(r.total),
                        w::Value::Plain,
                    );
                    if !r.request.note.is_empty() {
                        ui.label(&r.request.note);
                    }
                    w::divider(ui);
                }
            });
        let mut new_offset = None;
        ui.horizontal(|ui| {
            if w::button(lang.text("ใหม่กว่า", "Newer"))
                .ghost()
                .enabled(d.receipt_offset > 0 && !app.busy)
                .show(ui)
                .clicked()
            {
                new_offset = Some(d.receipt_offset.saturating_sub(50));
            }
            if w::button(lang.text("เก่ากว่า", "Older"))
                .ghost()
                .enabled(d.receipts.len() == 50 && !app.busy)
                .show(ui)
                .clicked()
            {
                new_offset = Some(d.receipt_offset.saturating_add(50));
            }
        });
        if let Some(offset) = new_offset {
            app.send(Command::PurchaseDetail(o.id, offset));
        }
        w::dialog_actions(ui, |ui| {
            let open = matches!(o.status(), PurchaseStatus::Open | PurchaseStatus::Partial);
            if open
                && allowed(app, Permission::Stock)
                && w::button(lang.text("รับของ", "Receive delivery"))
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
            {
                intent = 1;
            }
            if open
                && allowed(app, Permission::Catalog)
                && w::button(lang.text("ยกเลิกส่วนที่เหลือ", "Cancel remainder"))
                    .danger()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
            {
                intent = 2;
            }
            if w::button(lang.text("ปิด", "Close"))
                .ghost()
                .show(ui)
                .clicked()
            {
                intent = 3;
            }
        });
    });
    match intent {
        1 => {
            app.purchasing.receive = Some(ReceiveEditor {
                id: Uuid::new_v4(),
                order: o.clone(),
                quantities: vec![String::new(); o.items.len()],
                note: String::new(),
                confirm: false,
            })
        }
        2 => {
            app.purchasing.cancel = Some(CancelEditor {
                id: Uuid::new_v4(),
                order: o.clone(),
                reason: String::new(),
                confirm: false,
            })
        }
        _ => {}
    }
    if intent != 3 && !response.should_close() {
        app.purchasing.selected = Some(d);
    }
}
fn discard_actions(app: &mut App, ui: &mut egui::Ui) -> bool {
    let lang = app.lang;
    if app.purchasing.discard {
        w::inline_error(ui, lang.text("ทิ้งร่างที่กรอกไว้?", "Discard this draft?"));
    }
    let clicked = w::button(if app.purchasing.discard {
        lang.text("ยืนยันทิ้งร่าง", "Discard draft")
    } else {
        lang.text("ยกเลิก", "Cancel")
    })
    .ghost()
    .enabled(!app.busy)
    .show(ui)
    .clicked();
    if clicked {
        if app.purchasing.discard {
            app.purchasing.discard = false;
            return true;
        }
        app.purchasing.discard = true;
    }
    false
}
fn supplier_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut s) = app.purchasing.supplier_editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let mut close = false;
    w::dialog(ctx, "supplier-editor", t.m.control_lg * 11.0, |ui| {
        w::dialog_title(ui, lang.text("ข้อมูลผู้ขาย", "Supplier"), false);
        egui::ScrollArea::vertical()
            .max_height(scroll_height(ctx, &t))
            .show(ui, |ui| {
                input(ui, lang.text("ชื่อผู้ขาย", "Supplier name"), &mut s.name, 160);
                input(
                    ui,
                    lang.text("ข้อมูลติดต่อ (ไม่บังคับ)", "Contact (optional)"),
                    &mut s.contact,
                    240,
                );
                input(
                    ui,
                    lang.text("หมายเหตุ (ไม่บังคับ)", "Note (optional)"),
                    &mut s.note,
                    240,
                );
                ui.horizontal(|ui| {
                    w::switch(ui, &mut s.active, lang.text("ใช้งานอยู่", "Active"));
                    ui.label(lang.text("ใช้งานอยู่", "Active"));
                });
            });
        error(app, ui);
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึกผู้ขาย", "Save supplier"))
                .enabled(s.validate().is_ok() && !app.busy)
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                app.send(Command::SaveSupplier(s.clone()));
            }
            close = discard_actions(app, ui);
        });
    });
    if !close {
        app.purchasing.supplier_editor = Some(s)
    }
}
fn build_order(e: &OrderEditor) -> Option<PurchaseOrderRequest> {
    let items = e
        .lines
        .iter()
        .map(|l| {
            Some(PurchaseItemInput {
                target: l.target,
                quantity: format::parse_qty(&l.quantity)?,
                unit_cost: format::parse_money(&l.cost).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let r = PurchaseOrderRequest {
        id: e.id,
        supplier_id: e.supplier?,
        items,
        note: e.note.clone(),
    };
    r.validate().ok()?;
    Some(r)
}
fn draft_total(e: &OrderEditor) -> Option<Money> {
    e.lines.iter().try_fold(Money::ZERO, |a, l| {
        a.checked_add(
            purchase_cost(
                format::parse_money(&l.cost).ok()?,
                format::parse_qty(&l.quantity)?,
            )
            .ok()?,
        )
        .ok()
    })
}
fn order_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut e) = app.purchasing.editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let mut close = false;
    w::dialog(ctx, "purchase-create", t.m.control_lg * 16.0, |ui| {
        w::dialog_title(
            ui,
            if e.confirm {
                lang.text("ยืนยันสร้างใบสั่งซื้อ?", "Create this purchase order?")
            } else {
                lang.text("สร้างใบสั่งซื้อ", "Create purchase order")
            },
            false,
        );
        egui::ScrollArea::vertical().id_salt("purchase-create-body").max_height(scroll_height(ctx,&t)).show(ui,|ui| {
            if !e.confirm {
                w::field_label(ui,lang.text("ผู้ขาย","Supplier"),None);egui::ComboBox::from_id_salt("po-supplier").width(ui.available_width()).selected_text(app.purchasing.suppliers.iter().find(|s|Some(s.id)==e.supplier).map_or(lang.text("เลือกผู้ขาย","Choose supplier"),|s|s.name.as_str())).show_ui(ui,|ui|{for s in app.purchasing.suppliers.iter().filter(|s|s.active){ui.selectable_value(&mut e.supplier,Some(s.id),&s.name);}});
                input(ui,lang.text("ค้นหาสินค้าที่ติดตามสต็อก","Search tracked products"),&mut e.search,80);
                if e.search!=app.purchasing.product_query && !app.busy {app.purchasing.product_query=e.search.clone();app.send(Command::PurchaseProducts(e.search.clone()));}
                let candidates:Vec<_>=app.purchasing.products.iter().filter(|p|p.active&&p.stock.is_some()).map(|p|(PurchaseTarget::Product(p.id),p.name.clone(),lang.text("ชิ้น","pcs").to_owned(),p.cost)).chain(app.data.ingredients.iter().filter(|i|i.active&&(e.search.is_empty()||i.name.to_lowercase().contains(&e.search.to_lowercase()))).take(50).map(|i|(PurchaseTarget::Ingredient(i.id),i.name.clone(),i.unit.clone(),i.cost))).collect();
                ui.horizontal_wrapped(|ui| {for (target,name,unit,cost) in candidates {if !e.lines.iter().any(|l|l.target==target) && e.lines.len()<100 && w::chip(ui,&name,false,false).clicked(){e.lines.push(LineDraft{target,name,unit,quantity:String::new(),cost:cost.map(format::money).unwrap_or_default()});}}});
                w::help_text(ui,lang.text("ค้นหาสินค้าได้ครั้งละ 50 รายการ · สั่งได้สูงสุด 100 รายการ","Search returns up to 50 products · at most 100 lines per order."));
            }else {ui.label(t.text(app.purchasing.suppliers.iter().find(|s|Some(s.id)==e.supplier).map_or("—",|s|s.name.as_str()),type_scale::BODY_STRONG));}
            let mut remove=None;
            for (n,l) in e.lines.iter_mut().enumerate() {ui.push_id(n,|ui| {w::divider(ui);w::name_label(ui,&l.name,type_scale::BODY_STRONG,t.p.fg_default);if e.confirm {w::kv(ui,lang.text("จำนวน","Quantity"),&format!("{} {}",l.quantity,l.unit),w::Value::Plain);w::kv(ui,lang.text("ราคาต่อหน่วย","Unit cost"),&format::parse_money(&l.cost).map(format::baht).unwrap_or_else(|_| "—".into()),w::Value::Plain);}else{ui.columns(2,|cols|{input(&mut cols[0],&format!("{} ({})",lang.text("จำนวน","Quantity"),l.unit),&mut l.quantity,20);money(&mut cols[1],lang.text("ราคาต่อหน่วย (บาท)","Unit cost (฿)"),&mut l.cost,("po-cost",n));});if w::button(lang.text("เอารายการออก","Remove line")).ghost().show(ui).clicked(){remove=Some(n);}}
                if let (Some(q),Ok(c))=(format::parse_qty(&l.quantity),format::parse_money(&l.cost)){w::kv(ui,lang.text("รวมรายการ","Line total"),&total_text(purchase_cost(c,q)),w::Value::Plain);}
            });}if let Some(n)=remove{e.lines.remove(n);}
            if !e.confirm{input(ui,lang.text("หมายเหตุ (ไม่บังคับ)","Note (optional)"),&mut e.note,240);w::help_text(ui,lang.text("สินค้ากรอกจำนวนเต็ม วัตถุดิบทศนิยมได้ 3 ตำแหน่ง ต้องระบุราคาต่อหน่วย (0 ได้)","Products need whole units; ingredients allow three decimals. Enter a unit cost (0 is valid)."));}else if !e.note.is_empty(){ui.label(&e.note);}
        });
        if let Some(total) = draft_total(&e) {
            w::kv(
                ui,
                lang.text("ยอดสั่งซื้อ", "Order total"),
                &format::baht(total),
                w::Value::Plain,
            );
        }
        error(app, ui);
        let built = build_order(&e);
        w::dialog_actions(ui, |ui| {
            if w::button(if e.confirm {
                lang.text("ยืนยันสร้างใบสั่งซื้อ", "Confirm order")
            } else {
                lang.text("ตรวจใบสั่งซื้อ", "Review order")
            })
            .enabled(built.is_some() && !app.busy)
            .loading(app.busy)
            .show(ui)
            .clicked()
            {
                if e.confirm {
                    if let Some(r) = built {
                        app.send(Command::CreatePurchase(r));
                    }
                } else {
                    e.confirm = true;
                    app.purchasing.discard = false;
                }
            }
            if e.confirm {
                if w::button(lang.text("กลับไปแก้ไข", "Back to editing"))
                    .ghost()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
                {
                    e.confirm = false;
                }
            } else {
                close = discard_actions(app, ui);
            }
        });
    });
    if !close {
        app.purchasing.editor = Some(e)
    }
}
fn receive_request(e: &ReceiveEditor) -> Option<(ReceivePurchaseRequest, Money)> {
    let mut items = Vec::new();
    let mut total = Money::ZERO;
    for (n, (s, l)) in e.quantities.iter().zip(&e.order.items).enumerate() {
        if s.trim().is_empty() {
            continue;
        }
        let q = format::parse_qty(s)?;
        if q == 0 {
            continue;
        }
        l.target.validate_quantity(q).ok()?;
        if q > l.quantity - l.received {
            return None;
        }
        items.push(PurchaseReceiveItem {
            line: n,
            quantity: q,
        });
        total = total
            .checked_add(purchase_receive_cost(l.unit_cost, l.received, q).ok()?)
            .ok()?;
    }
    let r = ReceivePurchaseRequest {
        id: e.id,
        order_id: e.order.id,
        revision: e.order.revision,
        items,
        note: e.note.clone(),
    };
    r.validate().ok()?;
    Some((r, total))
}
fn receive_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut e) = app.purchasing.receive.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let mut close = false;
    w::dialog(ctx, "purchase-receive", t.m.control_lg * 15.0, |ui| {
        w::dialog_title(
            ui,
            if e.confirm {
                lang.text("ยืนยันรับของและเพิ่มสต็อก?", "Receive delivery and add stock?")
            } else {
                lang.text("รับของตามใบสั่งซื้อ", "Receive delivery")
            },
            false,
        );
        ui.label(t.text(&e.order.supplier.name, type_scale::BODY_STRONG));
        egui::ScrollArea::vertical()
            .id_salt("purchase-receive-body")
            .max_height(scroll_height(ctx, &t))
            .show(ui, |ui| {
                for (n, l) in e.order.items.iter().enumerate() {
                    if l.received == l.quantity {
                        continue;
                    }
                    ui.push_id(n, |ui| {
                        line_summary(ui, lang, l);
                        if e.confirm {
                            let q = format::parse_qty(&e.quantities[n]).unwrap_or(0);
                            if q > 0 {
                                w::kv(
                                    ui,
                                    lang.text("รับครั้งนี้", "Receive now"),
                                    &format!("{} {}", format::qty(q), l.unit),
                                    w::Value::Strong,
                                );
                                w::kv(
                                    ui,
                                    lang.text("มูลค่าครั้งนี้", "Value now"),
                                    &total_text(purchase_receive_cost(l.unit_cost, l.received, q)),
                                    w::Value::Plain,
                                );
                            }
                        } else {
                            input(
                                ui,
                                &format!(
                                    "{} ({})",
                                    lang.text("จำนวนรับจริงครั้งนี้", "Actual quantity this delivery"),
                                    l.unit
                                ),
                                &mut e.quantities[n],
                                20,
                            );
                        }
                    });
                }
                if !e.confirm {
                    input(
                        ui,
                        lang.text("หมายเหตุรับของ (ไม่บังคับ)", "Delivery note (optional)"),
                        &mut e.note,
                        240,
                    );
                    w::help_text(
                        ui,
                        lang.text(
                            "เว้นว่าง = ไม่รับรายการนั้น จำนวนต้องไม่เกินส่วนที่เหลือ",
                            "Leave blank to skip a line. Quantities cannot exceed the remainder.",
                        ),
                    );
                } else if !e.note.is_empty() {
                    ui.label(&e.note);
                }
                safety(app, ui);
            });
        let built = receive_request(&e);
        if let Some((_, total)) = &built {
            w::kv(
                ui,
                lang.text("มูลค่ารับครั้งนี้", "Delivery value"),
                &format::baht(*total),
                w::Value::Plain,
            );
        } else {
            w::help_text(ui,lang.text("ระบุจำนวนรับอย่างน้อย 1 รายการ ไม่เกินที่เหลือ สินค้าต้องเป็นจำนวนเต็ม","Enter at least one delivery quantity, within the remainder; products need whole units."));
        }
        error(app, ui);
        if app.error.is_some()
            && w::button(lang.text(
                "โหลดใบสั่งซื้อใหม่ (เก็บจำนวนที่กรอก)",
                "Refresh order (keep entered quantities)",
            ))
            .enabled(!app.busy)
            .show(ui)
            .clicked()
        {
            app.error = None;
            app.send(Command::PurchaseDetail(e.order.id, 0));
        }
        w::dialog_actions(ui, |ui| {
            if w::button(if e.confirm {
                lang.text("ยืนยันรับของ", "Confirm receiving")
            } else {
                lang.text("ตรวจรายการรับ", "Review delivery")
            })
            .enabled(built.is_some() && !app.busy)
            .loading(app.busy)
            .show(ui)
            .clicked()
            {
                if e.confirm {
                    if let Some((r, _)) = built {
                        app.send(Command::ReceivePurchase(r));
                    }
                } else {
                    e.confirm = true;
                    app.purchasing.discard = false;
                }
            }
            if e.confirm {
                if w::button(lang.text("กลับไปแก้ไข", "Back to editing"))
                    .ghost()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
                {
                    e.confirm = false;
                }
            } else {
                close = discard_actions(app, ui);
            }
        });
    });
    if !close {
        app.purchasing.receive = Some(e)
    }
}
fn cancel_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut e) = app.purchasing.cancel.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let mut close = false;
    w::dialog(ctx, "purchase-cancel", t.m.control_lg * 12.0, |ui| {
        w::dialog_title(
            ui,
            lang.text("ยกเลิกส่วนที่ยังไม่รับ?", "Cancel the unreceived remainder?"),
            true,
        );
        egui::ScrollArea::vertical()
            .max_height(scroll_height(ctx, &t))
            .show(ui, |ui| {
                ui.label(&e.order.supplier.name);
                for l in &e.order.items {
                    if l.quantity > l.received {
                        w::name_label(
                            ui,
                            &format!(
                                "{} · {} {}",
                                l.name,
                                format::qty(l.quantity - l.received),
                                l.unit
                            ),
                            type_scale::BODY,
                            t.p.fg_default,
                        );
                    }
                }
                w::help_text(
                    ui,
                    lang.text(
                        "ของที่รับแล้วและประวัติยังอยู่ สต็อกไม่เปลี่ยน",
                        "Received stock and history remain unchanged.",
                    ),
                );
                if !e.confirm {
                    input(
                        ui,
                        lang.text("เหตุผล (ต้องระบุ)", "Reason (required)"),
                        &mut e.reason,
                        240,
                    );
                } else {
                    ui.label(&e.reason);
                }
            });
        error(app, ui);
        if app.error.is_some()
            && w::button(lang.text("โหลดใบสั่งซื้อใหม่ (เก็บเหตุผล)", "Refresh order (keep reason)"))
                .enabled(!app.busy)
                .show(ui)
                .clicked()
        {
            app.error = None;
            app.send(Command::PurchaseDetail(e.order.id, 0));
        }
        w::dialog_actions(ui, |ui| {
            if w::button(if e.confirm {
                lang.text("ยืนยันยกเลิกส่วนที่เหลือ", "Confirm cancellation")
            } else {
                lang.text("ตรวจการยกเลิก", "Review cancellation")
            })
            .danger()
            .enabled(!e.reason.trim().is_empty() && !app.busy)
            .loading(app.busy)
            .show(ui)
            .clicked()
            {
                if e.confirm {
                    app.send(Command::CancelPurchase(CancelPurchaseRequest {
                        id: e.id,
                        order_id: e.order.id,
                        revision: e.order.revision,
                        reason: e.reason.clone(),
                    }));
                } else {
                    e.confirm = true;
                }
            }
            if e.confirm {
                if w::button(lang.text("กลับไปแก้ไข", "Back to editing"))
                    .ghost()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
                {
                    e.confirm = false;
                }
            } else {
                close = discard_actions(app, ui);
            }
        });
    });
    if !close {
        app.purchasing.cancel = Some(e)
    }
}

/// Layout-only presets use orders loaded from the isolated seed database.
#[cfg(debug_assertions)]
pub fn preview(app: &mut App, name: &str) {
    app.purchasing.open = true;
    match name {
        "suppliers" => app.purchasing.tab = Tab::Suppliers,
        "supplier-editor" => {
            app.purchasing.supplier_editor = Some(
                app.purchasing
                    .suppliers
                    .first()
                    .cloned()
                    .unwrap_or(Supplier {
                        id: Uuid::new_v4(),
                        name: "ผู้ขายตัวอย่าง / Sample supplier".into(),
                        contact: "08x-xxx-xxxx".into(),
                        note: "ข้อมูลจำลอง / Synthetic data".into(),
                        active: true,
                    }),
            )
        }
        "purchase-cancel" => {
            if let Some(order) = app.purchasing.orders.first().cloned() {
                app.purchasing.cancel = Some(CancelEditor {
                    id: Uuid::new_v4(),
                    order,
                    reason: "ผู้ขายส่งไม่ครบ / Supplier cannot deliver the remainder".into(),
                    confirm: true,
                });
            }
        }
        "purchase-detail" => {
            if app.purchasing.selected.is_none()
                && let Some(order) = app.purchasing.orders.first().cloned()
            {
                app.purchasing.selected = Some(Detail {
                    order,
                    receipts: Vec::new(),
                    receipt_offset: 0,
                });
            }
        }
        "purchase-create" | "purchase-create-confirm" => {
            let mut e = OrderEditor {
                supplier: app.purchasing.suppliers.first().map(|s| s.id),
                ..Default::default()
            };
            if let Some(i) = app.data.ingredients.iter().find(|i| i.active) {
                e.lines.push(LineDraft {
                    target: PurchaseTarget::Ingredient(i.id),
                    name: i.name.clone(),
                    unit: i.unit.clone(),
                    quantity: "12.5".into(),
                    cost: "125.50".into(),
                });
            }
            if name == "purchase-create-confirm" {
                e.confirm = true;
                if let Some(line) = e.lines.first_mut() {
                    line.cost = "125".into();
                }
            }
            app.purchasing.editor = Some(e);
        }
        "purchase-receive" | "purchase-receive-confirm" => {
            if let Some(o) = app
                .purchasing
                .orders
                .iter()
                .find(|o| matches!(o.status(), PurchaseStatus::Open | PurchaseStatus::Partial))
                .cloned()
            {
                let mut quantities = vec![String::new(); o.items.len()];
                if let Some((n, l)) = o
                    .items
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.quantity > l.received)
                {
                    quantities[n] = format::qty(l.quantity - l.received);
                }
                app.purchasing.receive = Some(ReceiveEditor {
                    id: Uuid::new_v4(),
                    order: o,
                    quantities,
                    note: String::new(),
                    confirm: name == "purchase-receive-confirm",
                });
            }
        }
        _ => {}
    }
}
