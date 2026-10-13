//! Device-local idle lock in Settings → Staff & permissions.
use crate::{
    app::App,
    auto_lock::TIMEOUTS,
    ui::{space, w},
};
use eframe::egui;

pub fn card(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    w::title(ui, lang.text("ล็อกเมื่อไม่ได้ใช้งาน", "Lock when idle"));
    w::help_text(ui, lang.text(
        "ตั้งค่าเฉพาะเครื่องนี้ ใช้เมื่อเปิดระบบพนักงานเท่านั้น ต้องใส่ PIN เพื่อกลับมาใช้งาน บิลและร่างแก้ไขยังอยู่",
        "For this device, with staff sign-in enabled. Enter a PIN to return; bills and unsaved edits are preserved.",
    ));
    if app.auto_lock.readonly {
        w::banner(ui, w::Tone::Danger, lang.text(
            "อ่าน auto-lock.json ไม่ได้ ปิดแอปแล้วตรวจไฟล์ก่อนเปลี่ยนการตั้งค่า ไฟล์จะไม่ถูกเขียนทับ",
            "Cannot read auto-lock.json. Close the app and check the file before changing this setting. It will not be overwritten.",
        ), None);
    } else if let Some(error) = &app.auto_lock.error {
        w::inline_error(
            ui,
            &format!(
                "{}: {error}",
                lang.text("บันทึกไม่ได้ กรุณาลองอีกครั้ง", "Could not save; try again")
            ),
        );
    }
    ui.add_space(space::S2);
    w::field_label(
        ui,
        lang.text("ล็อกหลังไม่มีการใช้งาน", "Lock after inactivity"),
        None,
    );
    let mut selected = app.auto_lock.settings.timeout_minutes;
    let allowed =
        crate::staff_ui::may(app, pos_domain::Permission::Settings) && !app.auto_lock.readonly;
    ui.add_enabled_ui(allowed, |ui| {
        ui.horizontal_wrapped(|ui| {
            for minutes in TIMEOUTS {
                let text = if minutes == 0 {
                    lang.text("ปิด", "Off").to_owned()
                } else {
                    format!("{minutes} {}", lang.text("นาที", "min"))
                };
                if w::chip(ui, &text, selected == minutes, false).clicked() {
                    selected = minutes;
                }
            }
        });
    });
    if selected != app.auto_lock.settings.timeout_minutes
        && let Err(error) = app.auto_lock.set_timeout(&app.data_dir, selected)
    {
        app.auto_lock.error = Some(error.to_string());
    }
    if !app.data.staff_enabled {
        w::help_text(
            ui,
            lang.text(
                "ยังไม่เริ่มจับเวลา เปิดระบบพนักงานด้านล่างก่อน",
                "The timer stays inactive until staff sign-in is enabled below.",
            ),
        );
    }
    w::help_text(ui, lang.text(
        "การเลื่อนเมาส์ กดปุ่ม พิมพ์ หรือแตะหน้าจอเริ่มนับใหม่ การทำงานเบื้องหลังไม่ยืดเวลา",
        "Mouse movement, keys, typing and touch restart the timer. Background work does not extend it.",
    ));
}
