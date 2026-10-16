//! Recovery is reviewed explicitly and activated before SQLite opens on restart.
use crate::{
    app::App,
    backup::Review,
    ui::{space, type_scale, w},
    worker::Command,
};
use eframe::egui;
#[derive(Default)]
pub struct State {
    pub source: String,
    pub focus: bool,
    pub review: Option<Review>,
    pub confirm: bool,
    pub pending: Option<std::path::PathBuf>,
    /// Password of a cloud backup file (`.phcloud`); cleared after use.
    pub cloud_password: String,
    decrypting: Option<std::sync::mpsc::Receiver<std::io::Result<std::path::PathBuf>>>,
}
/// A downloaded cloud backup rather than a backup folder.
fn cloud_file(source: &str) -> bool {
    source
        .trim()
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", crate::cloud_backup::EXTENSION))
}
pub fn controls(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let allowed =
        crate::staff_ui::access(app, pos_domain::Permission::Settings) != pos_domain::Access::Deny;
    ui.add_enabled_ui(allowed && app.ready && !app.busy, |ui| {
        ui.horizontal_wrapped(|ui| {
            if w::button(lang.text("สำรองข้อมูลครบชุด", "Create recovery backup")).show(ui).clicked() { app.backup(); }
            if w::button(lang.text("ส่งออกสินค้า CSV", "Export products CSV")).show(ui).clicked() { app.export_products(); }
        });
        w::help_text(ui, lang.text("ชุดสำรองเป็นโฟลเดอร์ รวมฐานข้อมูล ข้อมูลร้าน บิลค้างและร่างนับเงิน ไม่รวมการตั้งค่าเครื่องพิมพ์และหน้าจอ คัดลอกทั้งโฟลเดอร์ไปเก็บนอกเครื่องได้", "A backup folder includes the database, shop profile, open bill and drawer drafts. Printer and display settings stay on this device. Copy the entire folder to external storage."));
        ui.add_space(space::S3);
        w::field_label(ui, lang.text("โฟลเดอร์ชุดสำรอง หรือไฟล์ .phcloud จากคลาวด์ ที่ต้องการกู้คืน", "Backup folder, or .phcloud file from the cloud, to restore"), None);
        if ui.add(egui::TextEdit::singleline(&mut app.recovery.source).desired_width(ui.available_width())).changed() { app.recovery.review = None; }
        if let Some(rx) = &app.recovery.decrypting && let Ok(result) = rx.try_recv() {
            app.recovery.decrypting = None;
            match result {
                Ok(folder) => {
                    app.recovery.source = folder.display().to_string();
                    app.send(Command::InspectBackup(folder));
                }
                Err(e) => app.error = Some(format!("{}: {e}", lang.text("เปิดข้อมูลสำรองจากคลาวด์ไม่ได้", "Cannot open the cloud backup"))),
            }
        }
        if cloud_file(&app.recovery.source) {
            let decrypting = app.recovery.decrypting.is_some();
            w::field_label(ui, lang.text("รหัสผ่านข้อมูลสำรอง", "Backup password"), None);
            w::text_input(ui, &mut app.recovery.cloud_password, w::FieldOpts { limit: 200, password: true, enabled: !decrypting, ..Default::default() });
            if w::button(if decrypting { lang.text("กำลังถอดรหัส…", "Decrypting…") } else { lang.text("ถอดรหัสและตรวจ", "Decrypt and inspect") })
                .enabled(!app.recovery.cloud_password.is_empty() && !decrypting).loading(decrypting).show(ui).clicked()
            {
                let source = std::path::PathBuf::from(app.recovery.source.trim());
                let password = std::mem::take(&mut app.recovery.cloud_password);
                let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
                // Beside the data folder, like other recovery folders.
                let dest = app.data_dir.parent().unwrap_or(&app.data_dir).join(format!("cloud-restore-{stamp}.posbackup"));
                let (tx, rx) = std::sync::mpsc::channel();
                app.recovery.decrypting = Some(rx);
                std::thread::spawn(move || {
                    let result = std::fs::read(&source)
                        .and_then(|bytes| crate::cloud_backup::open(&bytes, &password, &dest))
                        .map(|()| dest);
                    let _ = tx.send(result);
                });
            }
        } else if w::button(lang.text("ตรวจชุดสำรอง", "Inspect backup")).enabled(!app.recovery.source.trim().is_empty()).show(ui).clicked() {
            app.send(Command::InspectBackup(app.recovery.source.trim().into()));
        }
        if let Some(review) = app.recovery.review.clone() {
            let t = crate::ui::Theme::of(ui);
            ui.label(t.text(format!("{} · {} {} · {} {} · {} {}", review.shop, review.manifest.info.sales, lang.text("บิลขาย", "sales"), review.manifest.info.products, lang.text("สินค้า", "products"), review.manifest.info.held, lang.text("บิลค้าง", "held bills")), type_scale::BODY));
            let date = time::OffsetDateTime::from_unix_timestamp(review.manifest.created_at).ok().map(|d| d.to_offset(time::macros::offset!(+7)).to_string()).unwrap_or_else(|| "—".into());
            ui.label(t.muted(format!("{}: {date} · schema {}", lang.text("สำรองเมื่อ", "Created"), review.manifest.info.schema), type_scale::CAPTION));
            if w::button(lang.text("ตรวจและยืนยันการกู้คืน…", "Review recovery…")).show(ui).clicked() { app.recovery.confirm = true; }
        }
    });
    crate::auto_backup_ui::card(app, ui);
    if app.recovery.focus {
        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
        app.recovery.focus = false;
    }
}
pub fn dialog(app: &mut App, ctx: &egui::Context) {
    if !app.recovery.confirm {
        return;
    }
    let Some(review) = app.recovery.review.clone() else {
        app.recovery.confirm = false;
        return;
    };
    let lang = app.lang;
    let unsaved = app.shop_draft != app.shop
        || app.catalog.dirty()
        || app.purchasing.dirty()
        || app.print.draft != app.print.settings
        || app.pricing_draft.is_some()
        || app.tax.draft.is_some()
        || app.auto_backup.dirty()
        || !app.cart.lines.is_empty();
    let response = w::dialog(ctx, "restore-review", crate::ui::space::S6 * 20.0, |ui| {
        w::dialog_title(
            ui,
            lang.text("แทนข้อมูลด้วยชุดสำรองนี้?", "Replace data with this backup?"),
            true,
        );
        ui.add(
            egui::Label::new(format!(
                "{} · {} {}",
                review.shop,
                review.manifest.info.sales,
                lang.text("บิลขาย", "sales")
            ))
            .wrap(),
        );
        ui.add(egui::Label::new(lang.text("ข้อมูลร้านและรายการทั้งหมดจะกลับไป ณ เวลาสำรองเมื่อเปิดแอปครั้งถัดไป ข้อมูลปัจจุบันทั้งโฟลเดอร์จะเก็บแยกไว้ให้ย้อนกลับ การตั้งค่าเครื่องพิมพ์และหน้าจอของเครื่องนี้คงเดิม", "On next launch, all shop records return to the backup date. The entire current data folder is preserved separately for rollback. This device's printer and display settings are retained.")).wrap());
        if unsaved {
            w::help_text(
                ui,
                lang.text(
                    "บันทึกหรือยกเลิกงานแก้ไข และพักหรือล้างบิลปัจจุบันก่อนกู้คืน",
                    "Save or discard edits and hold or clear the current bill before recovery.",
                ),
            );
        }
        if let Some(error) = &app.error {
            w::help_text(ui, error);
        }
        let mut action = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("เตรียมกู้คืนและหยุดใช้งาน", "Prepare recovery and stop"))
                .danger()
                .enabled(!app.busy && !unsaved)
                .show(ui)
                .clicked()
            {
                action = Some(true);
            }
            if w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                action = Some(false);
            }
        });
        action
    });
    if let Some(commit) = response.inner {
        if commit {
            app.send(Command::PrepareRecovery(app.data_dir.clone(), review));
        } else {
            app.recovery.confirm = false;
        }
    }
    if response.should_close() && !app.busy {
        app.recovery.confirm = false;
    }
}
