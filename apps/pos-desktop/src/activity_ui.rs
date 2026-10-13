//! Read-only unified activity journal, using the standard list and filter controls.
use crate::{
    i18n::Language,
    ui::{Theme, format, space, w},
};
use eframe::egui::{self, RichText};
use pos_core::{ActivityEvent, ActivityKind, ActivityQuery};

#[derive(Default)]
pub struct ActivityState {
    pub query: ActivityQuery,
    pub events: Vec<ActivityEvent>,
}
fn kind_label(kind: ActivityKind, lang: Language) -> &'static str {
    match kind {
        ActivityKind::Product => lang.text("สินค้า / ราคา", "Product / price"),
        ActivityKind::Discount => lang.text("ส่วนลด", "Discount"),
        ActivityKind::Refund => lang.text("คืนบางรายการ", "Item refund"),
        ActivityKind::Void => lang.text("ยกเลิกบิล", "Void"),
        ActivityKind::Stock => lang.text("ปรับสต็อก", "Stock adjustment"),
        ActivityKind::Staff => lang.text("พนักงาน", "Staff"),
        ActivityKind::Role => lang.text("บทบาท / สิทธิ์", "Role / permissions"),
        ActivityKind::Kitchen => lang.text("ยกเลิกรายการที่ส่งครัวแล้ว", "Kitchen cancellation"),
    }
}
/// Returns true when a filter, refresh, or page change requests worker I/O.
pub fn show(ui: &mut egui::Ui, theme: &Theme, lang: Language, state: &mut ActivityState) -> bool {
    let mut refresh = false;
    w::page_title(ui, lang.text("ประวัติกิจกรรม", "Activity history"));
    w::help_text(
        ui,
        lang.text(
            "บันทึกตั้งแต่เปิดใช้รุ่นนี้ ข้อมูลเก่าไม่มีประวัติที่สร้างย้อนหลัง",
            "Recorded since this version. Historical events are not reconstructed.",
        ),
    );
    ui.add_space(space::S3);
    ui.horizontal_wrapped(|ui| {
        w::field_label(ui, lang.text("ประเภท", "Type"), None);
        egui::ComboBox::from_id_salt("activity-kind")
            .selected_text(
                state.query.kind.map_or(lang.text("ทั้งหมด", "All"), |k| {
                    kind_label(k, lang)
                }),
            )
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut state.query.kind, None, lang.text("ทั้งหมด", "All"))
                    .changed()
                {
                    refresh = true;
                }
                for kind in [
                    ActivityKind::Product,
                    ActivityKind::Discount,
                    ActivityKind::Refund,
                    ActivityKind::Void,
                    ActivityKind::Stock,
                    ActivityKind::Staff,
                    ActivityKind::Role,
                    ActivityKind::Kitchen,
                ] {
                    if ui
                        .selectable_value(&mut state.query.kind, Some(kind), kind_label(kind, lang))
                        .changed()
                    {
                        refresh = true;
                    }
                }
            });
        w::field_label(ui, lang.text("ค้นหา", "Search"), None);
        let response = ui.add(egui::TextEdit::singleline(&mut state.query.search));
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            refresh = true;
        }
        if w::button(lang.text("ค้นหา / โหลดใหม่", "Search / refresh"))
            .show(ui)
            .clicked()
        {
            refresh = true;
        }
        if w::button(lang.text("ล้างตัวกรอง", "Clear filters"))
            .ghost()
            .show(ui)
            .clicked()
        {
            state.query = ActivityQuery::default();
            refresh = true;
        }
    });
    if refresh {
        state.query.offset = 0;
    }
    ui.horizontal(|ui| {
        if w::button(lang.text("ก่อนหน้า", "Previous"))
            .enabled(state.query.offset > 0)
            .show(ui)
            .clicked()
        {
            state.query.offset = state.query.offset.saturating_sub(50);
            refresh = true;
        }
        ui.label(format!(
            "{} {}",
            lang.text("หน้า", "Page"),
            state.query.offset / 50 + 1
        ));
        if w::button(lang.text("ถัดไป", "Next"))
            .enabled(state.events.len() == 50)
            .show(ui)
            .clicked()
        {
            state.query.offset = state.query.offset.saturating_add(50);
            refresh = true;
        }
    });
    w::divider(ui);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if state.events.is_empty() {
                ui.label(lang.text("ไม่พบกิจกรรม", "No activity found"));
            }
            for event in &state.events {
                ui.push_id(event.number, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(kind_label(event.kind, lang)).strong());
                        ui.label(
                            RichText::new(format::date_time(lang, event.created_at))
                                .color(theme.p.fg_muted),
                        );
                        if let Some(number) = event.detail.get("receipt").and_then(|n| n.as_u64()) {
                            ui.label(format::receipt_number(number));
                        }
                    });
                    if let Some(name) = event.detail.get("name").and_then(|n| n.as_str()) {
                        ui.label(name);
                    }
                    let actor = event
                        .staff
                        .by
                        .as_ref()
                        .map_or(lang.text("ไม่ระบุผู้ทำ", "Unattributed"), |s| s.name.as_str());
                    ui.label(format!("{}: {}", lang.text("ผู้ทำ", "By"), actor));
                    if let Some(approver) = &event.staff.approved_by {
                        ui.label(format!(
                            "{}: {}",
                            lang.text("ผู้อนุมัติ", "Approved by"),
                            approver.name
                        ));
                    }
                    if let Some(amount) = event
                        .detail
                        .get("amount_satang")
                        .and_then(|n| n.as_i64())
                        .and_then(|n| pos_domain::Money::from_satang(n).ok())
                    {
                        w::kv(
                            ui,
                            lang.text("จำนวนเงิน", "Amount"),
                            &format::baht(amount),
                            w::Value::Plain,
                        );
                    }
                    if let Some(reason) = event
                        .detail
                        .get("reason")
                        .and_then(|n| n.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        ui.label(format!("{}: {}", lang.text("เหตุผล", "Reason"), reason));
                    }
                    egui::CollapsingHeader::new(
                        lang.text("รายละเอียดการเปลี่ยนแปลง", "Change details"),
                    )
                    .show(ui, |ui| {
                        ui.label(format!(
                            "{}: {}",
                            lang.text("รหัสรายการ", "Record ID"),
                            event.entity_id
                        ));
                        detail(ui, lang, "", &event.detail);
                    });
                    ui.add_space(space::S2);
                    w::divider(ui);
                });
            }
        });
    refresh
}
fn detail_label(key: &str, lang: Language) -> &str {
    match key {
        "before" => lang.text("ก่อนแก้ไข", "Before"),
        "after" => lang.text("หลังแก้ไข", "After"),
        "name" => lang.text("ชื่อ", "Name"),
        "price" | "price_satang" => lang.text("ราคา", "Price"),
        "cost" => lang.text("ต้นทุน", "Cost"),
        "sku" => lang.text("รหัสสินค้า", "SKU"),
        "barcode" => lang.text("บาร์โค้ด", "Barcode"),
        "category" => lang.text("หมวดหมู่", "Category"),
        "stock" => lang.text("สต็อก", "Stock"),
        "reason" => lang.text("เหตุผล", "Reason"),
        "active" => lang.text("ใช้งาน", "Active"),
        "on_sale" => lang.text("เปิดขาย", "On sale"),
        "role" => lang.text("บทบาท", "Role"),
        "pin_changed" => lang.text("เปลี่ยน PIN", "PIN changed"),
        "action" => lang.text("การกระทำ", "Action"),
        "permissions" => lang.text("สิทธิ์", "Permissions"),
        "discount_limit_bp" => lang.text("เพดานส่วนลด (1% = 100)", "Discount limit (1% = 100)"),
        "amount_satang" => lang.text("จำนวนเงิน", "Amount"),
        "discount_satang" => lang.text("ส่วนลดรายการ", "Line discount"),
        "bill_discount_satang" => lang.text("ส่วนลดบิล", "Bill discount"),
        "line_discounts" => lang.text("ส่วนลดแต่ละรายการ", "Line discounts"),
        "receipt" => lang.text("ใบเสร็จ", "Receipt"),
        "options" => lang.text("ตัวเลือกสินค้า", "Product options"),
        "service_charge" => lang.text("คิดค่าบริการ", "Service charge"),
        "reports" => lang.text("รายงาน", "Reports"),
        "settings" => lang.text("ตั้งค่า", "Settings"),
        "manage_staff" => lang.text("จัดการพนักงาน", "Manage staff"),
        "catalog" => lang.text("จัดการสินค้า", "Manage products"),
        "cash_movement" => lang.text("เงินเข้า / ออก", "Cash in / out"),
        "close_shift" => lang.text("ปิดกะ", "Close shift"),
        "refund" => lang.text("คืนบางรายการ", "Item refund"),
        "void" => lang.text("ยกเลิกบิล", "Void"),
        "discount" => lang.text("ส่วนลด", "Discount"),
        _ => key,
    }
}
fn detail(ui: &mut egui::Ui, lang: Language, key: &str, value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            if key.is_empty() {
                for (key, value) in fields {
                    detail(ui, lang, key, value);
                }
            } else {
                ui.push_id(key, |ui| {
                    egui::CollapsingHeader::new(detail_label(key, lang))
                        .default_open(true)
                        .show(ui, |ui| {
                            for (key, value) in fields {
                                detail(ui, lang, key, value);
                            }
                        });
                });
            }
        }
        serde_json::Value::Array(values) => {
            ui.push_id(key, |ui| {
                egui::CollapsingHeader::new(detail_label(key, lang)).show(ui, |ui| {
                    for (index, value) in values.iter().enumerate() {
                        ui.push_id(index, |ui| {
                            detail(ui, lang, "", value);
                        });
                        w::divider(ui);
                    }
                });
            });
        }
        _ => {
            let label = detail_label(key, lang);
            if (key.contains("satang") || ["price", "cost"].contains(&key))
                && let Some(amount) = value
                    .as_i64()
                    .and_then(|n| pos_domain::Money::from_satang(n).ok())
            {
                w::kv(ui, label, &format::baht(amount), w::Value::Plain);
            } else {
                let text = if value.is_null() {
                    "—".into()
                } else if let Some(on) = value.as_bool() {
                    if on {
                        lang.text("ใช่", "Yes")
                    } else {
                        lang.text("ไม่", "No")
                    }
                    .into()
                } else if let Some(text) = value.as_str() {
                    if key == "action"
                        || matches!(text, "allow" | "approval" | "deny")
                            && matches!(
                                key,
                                "refund"
                                    | "void"
                                    | "discount"
                                    | "cash_movement"
                                    | "close_shift"
                                    | "catalog"
                                    | "stock"
                                    | "reports"
                                    | "settings"
                                    | "manage_staff"
                            )
                    {
                        match text {
                            "save" => lang.text("บันทึก", "Save"),
                            "delete" => lang.text("ลบ", "Delete"),
                            "archive" => lang.text("เก็บสินค้าเข้าคลัง", "Archive product"),
                            "on_sale" => lang.text("เปลี่ยนการเปิดขาย", "Change on-sale status"),
                            "category" => lang.text("เปลี่ยนหมวดหมู่", "Change category"),
                            "initial" => lang.text("สต็อกเริ่มต้น", "Initial stock"),
                            "receive" => lang.text("รับเข้า", "Receive"),
                            "remove" => lang.text("นำออก", "Remove"),
                            "count" => lang.text("ตรวจนับ", "Count"),
                            "allow" => lang.text("อนุญาต", "Allowed"),
                            "approval" => lang.text("ต้องอนุมัติ", "Needs approval"),
                            "deny" => lang.text("ไม่อนุญาต", "Denied"),
                            _ => text,
                        }
                        .into()
                    } else {
                        text.to_owned()
                    }
                } else {
                    value.to_string()
                };
                ui.label(format!("{label}: {text}"));
            }
        }
    }
}
