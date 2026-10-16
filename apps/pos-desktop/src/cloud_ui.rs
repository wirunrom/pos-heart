//! Settings → Devices & cloud: link the main register to the shop's cloud
//! account and see sync status (multi-device phase 2, `cloud.rs`).
use crate::{
    app::App,
    cloud::{CloudFile, Syncer},
    ui::{Theme, format, space, type_scale, w},
};
use eframe::egui;
use std::sync::mpsc;

/// The development server; a hosted one is entered by the owner.
const DEFAULT_SERVER: &str = "http://127.0.0.1:8080";

#[derive(Default)]
pub struct State {
    pub file: Option<CloudFile>,
    pub sync: Option<Syncer>,
    pub server: String,
    pub code: String,
    linking: Option<mpsc::Receiver<Result<CloudFile, String>>>,
    pub error: Option<String>,
    started: bool,
}

/// Starts syncing once the database is open, when this computer is linked.
pub fn start(app: &mut App) {
    if app.cloud.started || app.shop_managed() {
        return;
    }
    app.cloud.started = true;
    app.cloud.file = CloudFile::load(&app.data_dir);
    if let Some(file) = app.cloud.file.clone() {
        begin(app, file);
    }
}
fn begin(app: &mut App, file: CloudFile) {
    let ctx = app.ctx.clone();
    app.cloud.sync = Some(Syncer::start(file, app.worker.cloud_submit(), move || {
        ctx.request_repaint();
    }));
}

pub fn card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("คลาวด์", "Cloud"));
    if app.shop_managed() {
        w::help_text(
            ui,
            lang.text(
                "เครื่องหลักเป็นผู้ซิงก์ข้อมูลร้านกับคลาวด์",
                "The main register syncs the shop with the cloud.",
            ),
        );
        return;
    }
    w::help_text(
        ui,
        lang.text(
            "ส่งยอดขาย กะ และสต็อกขึ้นคลาวด์ให้เจ้าของดูจากที่ไหนก็ได้ และรับการแก้เมนู/ราคาจากหน้าเว็บ การขายไม่ต้องรอเน็ต ถ้าเน็ตหลุดจะส่งทีหลัง",
            "Send sales, shifts and stock to the cloud so the owner can see them anywhere, and take menu/price edits from the web. Selling never waits for the internet; while offline, records wait and go later.",
        ),
    );
    let may = crate::staff_ui::may(app, pos_domain::Permission::Settings) && !app.busy;
    if let Some(rx) = &app.cloud.linking
        && let Ok(result) = rx.try_recv()
    {
        app.cloud.linking = None;
        match result.and_then(|file| {
            file.save(&app.data_dir)
                .map(|()| file)
                .map_err(|e| e.to_string())
        }) {
            Ok(file) => {
                // A relink (another shop or server) needs everything again.
                let submit = app.worker.cloud_submit();
                let (reply, _) = mpsc::channel();
                submit(crate::cloud::Job {
                    work: crate::cloud::Work::Requeue,
                    reply,
                });
                app.cloud.file = Some(file.clone());
                app.cloud.code.clear();
                app.cloud.error = None;
                begin(app, file);
                app.toast(lang.text("เชื่อมต่อคลาวด์แล้ว", "Linked to the cloud."));
            }
            Err(e) => app.cloud.error = Some(e),
        }
    }
    if let Some(file) = app.cloud.file.clone() {
        linked(app, ui, &t, &file, may);
        return;
    }
    let busy = app.cloud.linking.is_some();
    if app.cloud.server.is_empty() {
        app.cloud.server = DEFAULT_SERVER.into();
    }
    w::field_label(ui, lang.text("ที่อยู่คลาวด์", "Cloud address"), None);
    w::text_input(
        ui,
        &mut app.cloud.server,
        w::FieldOpts {
            hint: "https://…",
            limit: 200,
            ..Default::default()
        },
    );
    w::field_label(
        ui,
        lang.text("รหัสเชื่อมต่อจากหน้าเว็บ", "Link code from the web"),
        None,
    );
    w::text_input(
        ui,
        &mut app.cloud.code,
        w::FieldOpts {
            hint: "ABCD 2345",
            limit: 12,
            ..Default::default()
        },
    );
    if let Some(e) = &app.cloud.error {
        w::inline_error(ui, e);
    }
    let code: String = app
        .cloud
        .code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    if w::button(if busy {
        lang.text("กำลังเชื่อมต่อ…", "Linking…")
    } else {
        lang.text("เชื่อมต่อคลาวด์", "Link to the cloud")
    })
    .enabled(may && code.len() == 8 && !busy)
    .loading(busy)
    .show(ui)
    .clicked()
    {
        let server = app.cloud.server.clone();
        let name = app.shop.name.clone();
        let (tx, rx) = mpsc::channel();
        app.cloud.linking = Some(rx);
        app.cloud.error = None;
        std::thread::spawn(move || {
            let _ = tx.send(crate::cloud::link(&server, &code, &name));
        });
    }
}

fn linked(app: &mut App, ui: &mut egui::Ui, t: &Theme, file: &CloudFile, may: bool) {
    let lang = app.lang;
    let status = app
        .cloud
        .sync
        .as_ref()
        .map(Syncer::status)
        .unwrap_or_default();
    w::kv(
        ui,
        lang.text("ร้านบนคลาวด์", "Cloud shop"),
        &file.shop_name,
        w::Value::Plain,
    );
    w::kv(
        ui,
        lang.text("ที่อยู่", "Address"),
        &file.server,
        w::Value::Plain,
    );
    let state = if status.syncing {
        lang.text("กำลังซิงก์…", "Syncing…").to_owned()
    } else {
        match status.last_sync {
            Some(at) => format!(
                "{} {}",
                lang.text("ซิงก์ล่าสุด", "Last synced"),
                format::date_time(lang, at)
            ),
            None => lang.text("ยังไม่ได้ซิงก์", "Not synced yet").to_owned(),
        }
    };
    w::kv(ui, lang.text("สถานะ", "Status"), &state, w::Value::Plain);
    if let Some(n) = status.pending {
        w::kv(
            ui,
            lang.text("รอส่ง", "Waiting to send"),
            &format!("{} {}", format::count(n), lang.text("รายการ", "records")),
            w::Value::Plain,
        );
    }
    if status.applied > 0 {
        ui.label(t.muted(
            format!(
                "{} {}",
                lang.text("ใช้การแก้ไขจากเว็บแล้ว", "Web edits applied:"),
                format::count(status.applied)
            ),
            type_scale::CAPTION,
        ));
    }
    if let Some(e) = &status.error {
        w::inline_error(
            ui,
            &format!("{} {e}", lang.text("ซิงก์ไม่สำเร็จ:", "Sync failed:")),
        );
    }
    ui.add_space(space::S2);
    ui.horizontal(|ui| {
        if w::button(lang.text("ซิงก์ตอนนี้", "Sync now"))
            .ghost()
            .enabled(!status.syncing)
            .show(ui)
            .clicked()
            && let Some(sync) = &app.cloud.sync
        {
            sync.wake();
        }
        if w::button(lang.text("ยกเลิกการเชื่อมต่อ", "Unlink"))
            .danger_quiet()
            .enabled(may)
            .show(ui)
            .clicked()
        {
            app.cloud.sync = None;
            match CloudFile::remove(&app.data_dir) {
                Ok(()) => {
                    app.cloud.file = None;
                    app.toast(lang.text(
                        "ยกเลิกการเชื่อมต่อคลาวด์แล้ว ข้อมูลในเครื่องยังอยู่ครบ",
                        "Unlinked from the cloud. Everything on this computer stays.",
                    ));
                }
                Err(e) => app.cloud.error = Some(e.to_string()),
            }
        }
    });
}
