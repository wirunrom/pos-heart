//! Device-local automatic-backup settings. Configuration loads once; work stays on the worker.
use crate::{
    app::App,
    auto_backup::Settings,
    ui::{Theme, space, type_scale, w},
    worker::Command,
};
use eframe::egui;
use std::path::Path;
#[derive(Default)]
pub struct State {
    pub settings: Settings,
    pub draft: Settings,
    pub error: Option<String>,
    pub last_result: Option<String>,
    pub latest: Option<std::time::SystemTime>,
}
impl State {
    pub fn load(data: &Path) -> Self {
        match Settings::load(data) {
            Ok(settings) => Self {
                draft: settings.clone(),
                settings,
                ..Default::default()
            },
            Err(e) => Self {
                error: Some(e.to_string()),
                ..Default::default()
            },
        }
    }
    pub fn saved(&mut self, settings: Settings) {
        self.settings = settings.clone();
        self.draft = settings;
    }
    pub fn dirty(&self) -> bool {
        self.settings != self.draft
    }
}
pub fn card(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    let t = Theme::of(ui);
    ui.add_space(space::S5);
    w::divider(ui);
    ui.add_space(space::S4);
    ui.label(t.text(
        lang.text("สำรองข้อมูลอัตโนมัติ", "Automatic backups"),
        type_scale::TITLE,
    ));
    w::help_text(ui,lang.text("เก็บใน automatic-backups ของเครื่องนี้ ทำงานขณะเปิดแอปและไม่ขัดจังหวะการขาย คัดลอกชุดสำรองไปไว้นอกเครื่องด้วยเพื่อป้องกันดิสก์เสีย", "Saved in this device’s automatic-backups folder while the app is open, without interrupting sales. Also copy backups off-device to protect against disk failure."));
    if let Some(error) = &app.auto_backup.error {
        w::inline_error(
            ui,
            &format!(
                "{}: {error}",
                lang.text(
                    "อ่านการตั้งค่าไม่ได้ จึงไม่เปิดการสำรองอัตโนมัติ กรุณาตรวจ auto-backup.json",
                    "Settings could not be read; automatic backups stay off. Check auto-backup.json"
                )
            ),
        );
        return;
    }
    let allowed =
        crate::staff_ui::access(app, pos_domain::Permission::Settings) != pos_domain::Access::Deny;
    ui.add_enabled_ui(allowed && !app.busy,|ui| {
        w::check_label(ui,&mut app.auto_backup.draft.enabled,lang.text("เปิดการสำรองอัตโนมัติ", "Enable automatic backups"));
        ui.add_enabled_ui(app.auto_backup.draft.enabled,|ui| {
            w::check_label(ui,&mut app.auto_backup.draft.daily,lang.text("วันละครั้ง ตามวันที่กรุงเทพฯ", "Once per Bangkok day"));
            w::check_label(ui,&mut app.auto_backup.draft.after_shift,lang.text("หลังปิดกะสำเร็จ", "After a shift closes"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(lang.text("เก็บชุดล่าสุด", "Keep latest backups"));
            for keep in [3,7,14,30] {
                if w::chip(ui,&keep.to_string(),app.auto_backup.draft.keep==keep,false).clicked() {app.auto_backup.draft.keep=keep;}
            }
        });
        w::help_text(ui,lang.text("หลังสำรองและตรวจสอบชุดใหม่สำเร็จเท่านั้น จึงลบเฉพาะชุดอัตโนมัติเก่าที่ระบบสร้างและยังสมบูรณ์ ไม่ลบชุดที่สำรองเองหรือชุดก่อนกู้คืน", "Only after a new backup passes verification, old valid automatic backups created by this device are removed. Manual and recovery safety backups are retained."));
        if let Err(error)=app.auto_backup.draft.validate() {w::inline_error(ui,&error.to_string());}
        ui.horizontal_wrapped(|ui| {
            if w::button(lang.text("บันทึกการสำรองอัตโนมัติ", "Save automatic backups")).enabled(app.auto_backup.dirty()&&app.auto_backup.draft.validate().is_ok()).show(ui).clicked() {
                app.send(Command::SaveAutoBackup(app.data_dir.clone(), app.auto_backup.draft.clone()));
            }
            if w::button(lang.text("ทิ้งการแก้ไข", "Discard changes")).ghost().enabled(app.auto_backup.dirty()).show(ui).clicked() {app.auto_backup.draft=app.auto_backup.settings.clone();}
            if w::button(lang.text("สำรองตอนนี้ / ลองใหม่", "Back up now / retry")).enabled(app.auto_backup.settings.enabled&&!app.auto_backup.dirty()).show(ui).clicked() {app.send(Command::RetryAutoBackup(app.data_dir.clone()));}
        });
    });
    if let Some(result) = &app.auto_backup.last_result {
        w::help_text(ui, result);
    }
}
