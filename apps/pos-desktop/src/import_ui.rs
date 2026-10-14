//! CSV preview and explicit all-or-nothing catalog confirmation.
use crate::{
    app::App,
    ui::{
        Theme, format,
        tokens::{space, type_scale},
        widgets as w,
    },
    worker::Command,
};
use eframe::egui;

#[derive(Default)]
pub struct ImportState {
    pub path: String,
    pub preview: Option<pos_core::ImportPreview>,
}

pub fn dialog(app: &mut App, ctx: &egui::Context) {
    if app.staff.approval.is_some() {
        return;
    }
    let Some(mut state) = app.catalog.import.take() else {
        return;
    };
    let lang = app.lang;
    let r = w::dialog(ctx, "csv-import", space::S8 * 15.0, |ui| {
        let t = Theme::of(ui);
        w::dialog_title(
            ui,
            lang.text("นำเข้าสินค้าใหม่จาก CSV", "Import new products from CSV"),
            false,
        );
        let mut out = None;
        egui::ScrollArea::vertical().id_salt("import-body").max_height((ctx.content_rect().height() - t.m.control_lg * 5.0).max(t.m.control_lg * 3.0)).show(ui, |ui| {
        ui.label(lang.text("เพิ่มสินค้าใหม่ทั้งไฟล์พร้อมกัน ไม่แก้สินค้าหรือสต็อกเดิม ตัวเลือกสินค้าเพิ่มภายหลังได้", "Adds all new products together. Existing products and stock stay intact. Add options afterwards."));
        w::field_label(
            ui,
            lang.text("ตำแหน่งไฟล์ UTF-8 CSV", "UTF-8 CSV file path"),
            None,
        );
        let old = state.path.clone();
        ui.add_enabled_ui(!app.busy, |ui| { w::text_input(
            ui,
            &mut state.path,
            w::FieldOpts {
                limit: 4096,
                hint: lang.text("วางตำแหน่งไฟล์ .csv", "Paste the .csv file path"),
                ..Default::default()
            },
        );
        });
        if old != state.path {
            state.preview = None;
        }
        w::help_text(ui, lang.text("จำเป็น: sku,name,price · เพิ่มได้: category,stock,barcode,on_sale,service_charge,cost · stock ว่าง = ไม่ติดตาม · cost ว่าง = ไม่ทราบต้นทุน · สูงสุด 5,000 รายการ / 2 MiB", "Required: sku,name,price. Optional: category,stock,barcode,on_sale,service_charge,cost. Blank stock = untracked; blank cost = unknown. Max 5,000 rows / 2 MiB."));
        if w::button(lang.text("อ่านไฟล์และตรวจสอบ", "Read and preview"))
            .enabled(!app.busy && !state.path.trim().is_empty())
            .show(ui)
            .clicked()
        {
            out = Some(0);
        }
        if let Some(preview) = &state.preview {
            w::divider(ui);
            ui.label(t.text(
                format!(
                    "{} {} · {} {}",
                    preview.rows.len(),
                    lang.text("สินค้าที่อ่านได้", "parsed products"),
                    preview.issues.len(),
                    lang.text("ข้อผิดพลาด", "errors")
                ),
                type_scale::BODY_STRONG,
            ));
            egui::ScrollArea::vertical()
                .max_height(t.m.control_lg * 4.0)
                .show(ui, |ui| {
                    for issue in &preview.issues {
                        w::inline_error(
                            ui,
                            &format!(
                                "{} {}: {}",
                                lang.text("แถว", "Row"),
                                issue.row,
                                issue.message
                            ),
                        );
                    }
                    for row in preview.rows.iter().take(50) {
                        ui.label(&row.product.name);
                        ui.horizontal(|ui| {
                            ui.label(t.muted(
                                format!("{} · {}", row.row, row.product.sku),
                                type_scale::LABEL,
                            ));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        t.text(format::money(row.product.price), type_scale::MONEY),
                                    );
                                },
                            );
                        });
                    }
                });
            if preview.rows.len() > 50 {
                w::help_text(
                    ui,
                    lang.text(
                        "แสดงตัวอย่าง 50 รายการแรก ตรวจสอบทุกแถวก่อนนำเข้าแล้ว",
                        "Showing the first 50 products; every row has been validated.",
                    ),
                );
            }
            w::help_text(
                ui,
                lang.text(
                    "ยืนยันแล้วจะเพิ่มสินค้าทุกแถวที่ตรวจผ่าน หากมีข้อผิดพลาดจะไม่นำเข้าเลย",
                    "Confirm adds all validated products. Any error refuses the entire import.",
                ),
            );
        }
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        });
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ยืนยันนำเข้าทั้งหมด", "Confirm import all"))
                .enabled(!app.busy && state.preview.as_ref().is_some_and(|p| p.can_import()))
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(1);
            }
            if w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(2);
            }
        });
        out
    });
    match r.inner {
        Some(0) => {
            state.preview = None;
            app.send(Command::PreviewImport(state.path.trim().into()));
        }
        Some(1) => {
            if let Some(preview) = &state.preview {
                app.send(Command::ImportProducts(preview.source.clone()));
            }
        }
        Some(2) => return,
        None if !app.busy && r.should_close() => return,
        _ => {}
    }
    app.catalog.import = Some(state);
}
