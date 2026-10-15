//! Settings → Devices: this computer as the shop hub (multi-device phase 1).
//! Off by default: nothing listens until an authorised member turns it on.
//! `hub.json`, `hub-cert.der` and `hub-key.der` are device files beside the
//! database (excluded from backups; recovery keeps this computer's own).
use crate::{
    app::App,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, RichText};
use pos_core::DeviceKind;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Instant};

const FILE: &str = "hub.json";
/// Default LAN port for paired devices.
pub const DEFAULT_PORT: u16 = 47_810;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub enabled: bool,
    pub port: u16,
    /// Debug captures listen on this computer only (no firewall prompt).
    #[serde(skip)]
    pub loopback: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            loopback: false,
        }
    }
}
impl Settings {
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::write(dir.join(FILE), serde_json::to_vec_pretty(self)?)
    }
}

#[derive(Default)]
pub struct State {
    pub settings: Settings,
    pub hub: Option<pos_hub::Hub>,
    /// The pairing code on screen and when it was opened.
    pub pairing: Option<(String, Instant)>,
    pub error: Option<String>,
}

/// This computer's address on the shop LAN, as devices type it. Finds the
/// outgoing interface without sending anything.
pub fn lan_address() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("10.255.255.255:1").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

/// Starts the hub when `hub.json` says so (app start and the switch).
pub fn start(app: &mut App) {
    if app.hub.hub.is_some() || !app.hub.settings.enabled {
        return;
    }
    let result = (|| -> std::io::Result<pos_hub::Hub> {
        let identity = pos_hub::tls::Identity::load_or_create(&app.data_dir)?;
        let host = if app.hub.settings.loopback {
            "127.0.0.1"
        } else {
            "0.0.0.0"
        };
        let listener = std::net::TcpListener::bind((host, app.hub.settings.port))?;
        pos_hub::Hub::start(listener, &identity, app.worker.hub_submit())
    })();
    match result {
        Ok(hub) => {
            app.worker.send(Command::SetHub(Some(hub.notifier())));
            app.hub.hub = Some(hub);
            app.hub.error = None;
        }
        Err(e) => app.hub.error = Some(e.to_string()),
    }
}
fn stop(app: &mut App) {
    app.worker.send(Command::SetHub(None));
    app.hub.hub = None;
    app.hub.pairing = None;
}
fn set_enabled(app: &mut App, enabled: bool) {
    app.hub.settings.enabled = enabled;
    if let Err(e) = app.hub.settings.save(&app.data_dir) {
        app.hub.error = Some(e.to_string());
        return;
    }
    if enabled {
        start(app);
    } else {
        stop(app);
    }
}

fn kind_label(lang: crate::i18n::Language, kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Register => lang.text("เครื่องขาย (ลิ้นชักของตัวเอง)", "Register (own drawer)"),
        DeviceKind::Kitchen => lang.text("จอครัว", "Kitchen screen"),
        DeviceKind::Ordering => lang.text("แท็บเล็ตรับออเดอร์", "Ordering tablet"),
    }
}

pub fn card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let may = crate::staff_ui::may(app, pos_domain::Permission::Settings) && !app.busy;
    w::title(ui, lang.text("อุปกรณ์ในร้าน", "Shop devices"));
    w::help_text(
        ui,
        lang.text(
            "ให้เครื่องขายเครื่องที่สอง จอครัว และ iPad รับออเดอร์ ใช้ข้อมูลร้านเดียวกันผ่าน Wi-Fi/LAN ของร้าน เครื่องนี้เป็นเครื่องหลักที่เก็บข้อมูล ขายได้แม้ไม่มีอินเทอร์เน็ต",
            "Let a second register, kitchen screens and ordering iPads use this shop's data over the shop Wi-Fi/LAN. This computer is the main register that keeps the data; selling works without internet.",
        ),
    );
    ui.horizontal(|ui| {
        let mut on = app.hub.settings.enabled;
        ui.add_enabled_ui(may, |ui| {
            if w::switch(
                ui,
                &mut on,
                lang.text("อนุญาตให้เครื่องอื่นเชื่อมต่อ", "Allow other devices"),
            )
            .changed()
            {
                set_enabled(app, on);
            }
        });
        ui.label(t.text(
            lang.text("อนุญาตให้เครื่องอื่นเชื่อมต่อ", "Allow other devices"),
            type_scale::BODY_STRONG,
        ));
    });
    if let Some(e) = &app.hub.error {
        w::inline_error(
            ui,
            &format!(
                "{} {e}",
                lang.text("เปิดการเชื่อมต่อไม่สำเร็จ:", "Could not start:")
            ),
        );
    }
    if let Some(hub) = &app.hub.hub {
        let address = lan_address()
            .map(|ip| format!("{ip}:{}", hub.address().port()))
            .unwrap_or_else(|| hub.address().to_string());
        w::kv(
            ui,
            lang.text("ที่อยู่เครื่องหลัก", "Main register address"),
            &address,
            w::Value::Plain,
        );
        w::kv(
            ui,
            lang.text("รหัสยืนยันเครื่องหลัก", "Main register check code"),
            &pos_hub::tls::short(&hub.fingerprint()),
            w::Value::Plain,
        );
        let online = hub.connected().len() as u64;
        w::kv(
            ui,
            lang.text("เชื่อมต่ออยู่", "Connected now"),
            &format!(
                "{} {}",
                format::count(online),
                lang.text("เครื่อง", "devices")
            ),
            w::Value::Plain,
        );
        ui.add_space(space::S2);
        let expired = app
            .hub
            .pairing
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > pos_hub::PAIRING_TTL);
        if expired {
            app.hub.pairing = None;
        }
        match app.hub.pairing.clone() {
            None => {
                if w::button(lang.text("จับคู่เครื่องใหม่", "Pair a device"))
                    .icon(Icon::Plus)
                    .enabled(may)
                    .show(ui)
                    .clicked()
                {
                    app.hub.pairing = Some((hub.open_pairing(), Instant::now()));
                }
            }
            Some((code, at)) => {
                let left = pos_hub::PAIRING_TTL.saturating_sub(at.elapsed()).as_secs();
                egui::Frame::new()
                    .fill(t.p.bg_sunken)
                    .corner_radius(t.radius(crate::ui::tokens::radius::MD))
                    .inner_margin(egui::Margin::same(space::S3 as i8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(t.muted(
                            lang.text("ใส่รหัสนี้ที่เครื่องที่จะจับคู่", "Enter this code on the new device"),
                            type_scale::LABEL,
                        ));
                        ui.label(
                            RichText::new(format!("{} {}", &code[..3], &code[3..]))
                                .font(t.strong_font(type_scale::MONEY_XL)),
                        );
                        ui.label(t.muted(
                            format!(
                                "{} {}:{:02} · {} {address} · {} {}",
                                lang.text("หมดอายุใน", "Expires in"),
                                left / 60,
                                left % 60,
                                lang.text("ที่อยู่", "address"),
                                lang.text("รหัสยืนยัน", "check code"),
                                pos_hub::tls::short(&hub.fingerprint())
                            ),
                            type_scale::CAPTION,
                        ));
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_secs(1));
                        if w::button(lang.text("ยกเลิกการจับคู่", "Cancel pairing"))
                            .ghost()
                            .size(w::Size::Sm)
                            .show(ui)
                            .clicked()
                        {
                            hub.close_pairing();
                            app.hub.pairing = None;
                        }
                    });
            }
        }
    }
    ui.add_space(space::S3);
    ui.label(t.text(
        lang.text("เครื่องที่จับคู่แล้ว", "Paired devices"),
        type_scale::BODY_STRONG,
    ));
    let devices = app.data.devices.clone();
    if devices.is_empty() {
        ui.label(t.muted(
            lang.text("ยังไม่มีเครื่องอื่น", "No other devices yet"),
            type_scale::BODY,
        ));
    }
    let online = app
        .hub
        .hub
        .as_ref()
        .map(|h| h.connected())
        .unwrap_or_default();
    for device in devices {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(t.text(&device.name, type_scale::BODY_STRONG));
                let state = if device.revoked_at.is_some() {
                    lang.text("เพิกถอนแล้ว", "Revoked").to_owned()
                } else if online.contains(&device.id) {
                    lang.text("เชื่อมต่ออยู่", "Connected").to_owned()
                } else {
                    match device.last_seen_at {
                        Some(at) => format!(
                            "{} {}",
                            lang.text("เห็นล่าสุด", "Last seen"),
                            format::date_time(lang, at)
                        ),
                        None => lang.text("ยังไม่เคยเชื่อมต่อ", "Never connected").to_owned(),
                    }
                };
                ui.label(t.muted(
                    format!("{} · {state}", kind_label(lang, device.kind)),
                    type_scale::CAPTION,
                ));
            });
            if device.revoked_at.is_none() {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if w::button(lang.text("เพิกถอน", "Revoke"))
                        .danger_quiet()
                        .size(w::Size::Sm)
                        .enabled(may)
                        .show(ui)
                        .clicked()
                    {
                        app.send(Command::RevokeDevice(device.id));
                    }
                });
            }
        });
        w::divider(ui);
    }
}
