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
    /// Backup password form (never kept after the key is derived).
    password: String,
    confirm: String,
    deriving: Option<mpsc::Receiver<std::io::Result<crate::cloud_backup::Key>>>,
    changing: bool,
    /// Debug captures: a sample sync status instead of a running sync.
    #[cfg(debug_assertions)]
    preview: Option<crate::cloud::Status>,
}

/// Debug captures: Settings → Devices & cloud, linked to a sample cloud
/// (nothing is contacted) or not linked.
#[cfg(debug_assertions)]
pub fn preview(app: &mut App, linked: bool) {
    app.cloud.started = true;
    if !linked {
        return;
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    app.cloud.file = Some(CloudFile {
        server: "https://cloud.posheart.example".into(),
        hub: uuid::Uuid::nil(),
        token: String::new(),
        shop: uuid::Uuid::nil(),
        shop_name: "ร้านหัวใจ".into(),
        backup_salt: Some("00".repeat(16)),
        backup_key: Some("00".repeat(32)),
        last_backup: Some(now - 3_600),
    });
    app.cloud.preview = Some(crate::cloud::Status {
        last_sync: Some(now - 40),
        pending: Some(0),
        applied: 2,
        last_backup: Some(now - 3_600),
        ..Default::default()
    });
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
    app.cloud.sync = Some(Syncer::start(
        file,
        app.worker.cloud_submit(),
        app.data_dir.clone(),
        move || {
            ctx.request_repaint();
        },
    ));
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
    let status = app.cloud.sync.as_ref().map(Syncer::status);
    #[cfg(debug_assertions)]
    let status = status.or_else(|| app.cloud.preview.clone());
    let status = status.unwrap_or_default();
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
    ui.add_space(space::S3);
    w::divider(ui);
    backups(app, ui, t, file, &status, may);
}

/// Encrypted daily backups to the cloud, sealed with the owner's password.
fn backups(
    app: &mut App,
    ui: &mut egui::Ui,
    t: &Theme,
    file: &CloudFile,
    status: &crate::cloud::Status,
    may: bool,
) {
    let lang = app.lang;
    ui.label(t.text(
        lang.text("ข้อมูลสำรองบนคลาวด์", "Cloud backups"),
        type_scale::BODY_STRONG,
    ));
    if let Some(rx) = &app.cloud.deriving
        && let Ok(result) = rx.try_recv()
    {
        app.cloud.deriving = None;
        match result {
            Ok(key) => {
                let mut saved = file.clone();
                let (salt, hex) = key.to_hex();
                saved.backup_salt = Some(salt);
                saved.backup_key = Some(hex);
                match saved.save(&app.data_dir) {
                    Ok(()) => {
                        app.cloud.file = Some(saved.clone());
                        app.cloud.changing = false;
                        begin(app, saved);
                        if let Some(sync) = &app.cloud.sync {
                            sync.backup_now();
                        }
                        app.toast(lang.text(
                            "ตั้งรหัสผ่านข้อมูลสำรองแล้ว กำลังสำรองครั้งแรก",
                            "Backup password set. Making the first backup.",
                        ));
                    }
                    Err(e) => app.cloud.error = Some(e.to_string()),
                }
            }
            Err(e) => app.cloud.error = Some(e.to_string()),
        }
    }
    let has_key = file.key().is_some();
    if has_key && !app.cloud.changing {
        let state = if status.backing_up {
            lang.text("กำลังสำรอง…", "Backing up…").to_owned()
        } else {
            match status.last_backup {
                Some(at) => format!(
                    "{} {}",
                    lang.text("สำรองล่าสุด", "Last backup"),
                    format::date_time(lang, at)
                ),
                None => lang.text("ยังไม่ได้สำรอง", "No backup yet").to_owned(),
            }
        };
        w::kv(ui, lang.text("สถานะ", "Status"), &state, w::Value::Plain);
        if let Some(e) = &status.backup_error {
            w::inline_error(
                ui,
                &format!("{} {e}", lang.text("สำรองไม่สำเร็จ:", "Backup failed:")),
            );
        }
        w::help_text(
            ui,
            lang.text(
                "สำรองทั้งร้านวันละครั้ง เข้ารหัสในเครื่องนี้ก่อนส่ง คลาวด์อ่านไม่ได้ ดาวน์โหลดได้จากหน้าเว็บเจ้าของ แล้วกู้คืนที่ ตั้งค่า → ข้อมูลและสำรอง ด้วยรหัสผ่านนี้",
                "The whole shop is backed up daily, encrypted on this computer before it leaves; the cloud cannot read it. Download from the owner's web page and restore in Settings → Data & backup with this password.",
            ),
        );
        ui.horizontal(|ui| {
            if w::button(lang.text("สำรองตอนนี้", "Back up now"))
                .ghost()
                .enabled(!status.backing_up)
                .show(ui)
                .clicked()
                && let Some(sync) = &app.cloud.sync
            {
                sync.backup_now();
            }
            if w::button(lang.text("เปลี่ยนรหัสผ่าน", "Change password"))
                .ghost()
                .enabled(may)
                .show(ui)
                .clicked()
            {
                app.cloud.changing = true;
            }
        });
        return;
    }
    w::help_text(
        ui,
        lang.text(
            "ตั้งรหัสผ่านสำหรับข้อมูลสำรอง (อย่างน้อย 10 ตัว) ข้อมูลจะถูกเข้ารหัสด้วยรหัสนี้ก่อนส่ง จดเก็บไว้ให้ดี ถ้าลืม ข้อมูลสำรองบนคลาวด์จะเปิดไม่ได้ และไม่มีใครกู้รหัสให้ได้",
            "Set a password for backups (at least 10 characters). Backups are encrypted with it before they leave. Keep it safe: if it is lost, cloud backups cannot be opened and nobody can recover it.",
        ),
    );
    if has_key {
        w::help_text(
            ui,
            lang.text(
                "ข้อมูลสำรองเก่ายังต้องใช้รหัสผ่านเดิม",
                "Older backups still need the old password.",
            ),
        );
    }
    let deriving = app.cloud.deriving.is_some();
    for (value, label) in [
        (
            &mut app.cloud.password,
            lang.text("รหัสผ่านข้อมูลสำรอง", "Backup password"),
        ),
        (&mut app.cloud.confirm, lang.text("ใส่อีกครั้ง", "Repeat it")),
    ] {
        w::field_label(ui, label, None);
        w::text_input(
            ui,
            value,
            w::FieldOpts {
                limit: 200,
                password: true,
                enabled: !deriving,
                ..Default::default()
            },
        );
    }
    let long = app.cloud.password.chars().count() >= crate::cloud_backup::MIN_PASSWORD;
    let same = app.cloud.password == app.cloud.confirm;
    if !app.cloud.confirm.is_empty() && !same {
        w::inline_error(
            ui,
            lang.text("รหัสผ่านสองช่องไม่ตรงกัน", "The passwords do not match."),
        );
    }
    ui.horizontal(|ui| {
        if w::button(if deriving {
            lang.text("กำลังตั้งรหัส…", "Setting…")
        } else {
            lang.text("ตั้งรหัสผ่านและเริ่มสำรอง", "Set password and start backups")
        })
        .enabled(may && long && same && !deriving)
        .loading(deriving)
        .show(ui)
        .clicked()
        {
            let password = std::mem::take(&mut app.cloud.password);
            app.cloud.confirm.clear();
            let (tx, rx) = mpsc::channel();
            app.cloud.deriving = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(crate::cloud_backup::Key::new(&password));
            });
        }
        if has_key
            && w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .show(ui)
                .clicked()
        {
            app.cloud.changing = false;
            app.cloud.password.clear();
            app.cloud.confirm.clear();
        }
    });
}
