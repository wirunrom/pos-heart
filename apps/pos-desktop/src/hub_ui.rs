//! Settings → Devices: this computer as the shop hub (multi-device phase 1).
//! Off by default: nothing listens until an authorised member turns it on.
//! `hub.json`, `hub-cert.der` and `hub-key.der` are device files beside the
//! database (excluded from backups; recovery keeps this computer's own).
//! The hub also serves the iPad ordering web app: HTTPS on `port + 1`
//! (certificate from the shop CA in `web-ca.der` / `web-ca-key.der`) and
//! the setup page with the CA profile on plain HTTP `port + 2`.
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
/// The iPad app (HTTPS) and its setup page (HTTP) follow the hub port.
pub fn web_ports(port: u16) -> (u16, u16) {
    (port.saturating_add(1), port.saturating_add(2))
}

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
    /// Client mode: this computer is a paired device of a main register.
    pub device: Option<crate::worker::DeviceFile>,
    /// Client mode: currently connected to the main register.
    pub connected: bool,
    /// "Use this computer as a device" form.
    pub join: Join,
    /// Announces the hub on the LAN while it runs.
    pub advert: Option<pos_hub::discovery::Advertisement>,
    /// Why the iPad web app is not available (the hub itself still runs).
    pub web_error: Option<String>,
}
#[derive(Default)]
pub struct Join {
    /// The form is folded until asked for: most computers are the main register.
    pub open: bool,
    pub address: String,
    pub code: String,
    pub name: String,
    pub kitchen: bool,
    pub running: Option<std::sync::mpsc::Receiver<Result<crate::worker::DeviceFile, String>>>,
    pub done: Option<Result<(), String>>,
    /// Main registers found on the LAN (mDNS), or a search in progress.
    pub found: Vec<pos_hub::discovery::Found>,
    pub searching: Option<std::sync::mpsc::Receiver<Vec<pos_hub::discovery::Found>>>,
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
    if app.hub.hub.is_some() || !app.hub.settings.enabled || app.hub.device.is_some() {
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
        Ok(mut hub) => {
            app.hub.web_error = start_web(app, &mut hub).err().map(|e| e.to_string());
            app.worker.send(Command::SetHub(Some(hub.notifier())));
            // Devices find the hub by name; a failure only means typing the address.
            if !app.hub.settings.loopback {
                app.hub.advert = pos_hub::discovery::Advertisement::start(
                    &app.shop.name,
                    hub.address().port(),
                    &hub.fingerprint(),
                )
                .ok();
            }
            app.hub.hub = Some(hub);
            app.hub.error = None;
        }
        Err(e) => app.hub.error = Some(e.to_string()),
    }
}
/// The iPad web app on the next two ports; a failure leaves the hub running.
fn start_web(app: &App, hub: &mut pos_hub::Hub) -> std::io::Result<()> {
    let identity = pos_hub::webtls::WebIdentity::load_or_create(&app.data_dir)?;
    let host = if app.hub.settings.loopback {
        "127.0.0.1"
    } else {
        "0.0.0.0"
    };
    let (https, http) = web_ports(hub.address().port());
    let https = std::net::TcpListener::bind((host, https))?;
    let http = std::net::TcpListener::bind((host, http))?;
    let ips: Vec<_> = lan_address().into_iter().collect();
    hub.start_web(https, http, &identity, &ips, &app.shop.name)
}
fn stop(app: &mut App) {
    app.hub.advert = None;
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

/// Client mode: which main register this computer uses, and leaving it.
fn device_card(app: &mut App, ui: &mut egui::Ui, device: &crate::worker::DeviceFile) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(
        ui,
        lang.text("เครื่องนี้เป็นเครื่องลูก", "This computer is a shop device"),
    );
    w::help_text(
        ui,
        lang.text(
            "ขาย พักบิล และดูข้อมูลผ่านเครื่องหลัก การสำรองและกู้คืนข้อมูลทำที่เครื่องหลัก",
            "Selling, held bills and data go through the main register. Backups and recovery are done there.",
        ),
    );
    w::kv(
        ui,
        lang.text("เครื่องหลัก", "Main register"),
        &device.hub.to_string(),
        w::Value::Plain,
    );
    w::kv(
        ui,
        lang.text("ชื่อเครื่องนี้", "This device"),
        &device.name,
        w::Value::Plain,
    );
    w::kv(
        ui,
        lang.text("ประเภท", "Kind"),
        kind_label(lang, device.kind),
        w::Value::Plain,
    );
    w::kv(
        ui,
        lang.text("สถานะ", "Status"),
        if app.hub.connected {
            lang.text("เชื่อมต่ออยู่", "Connected")
        } else {
            lang.text("ไม่ได้เชื่อมต่อ กำลังลองใหม่…", "Not connected, retrying…")
        },
        if app.hub.connected {
            w::Value::Plain
        } else {
            w::Value::Danger
        },
    );
    ui.add_space(space::S2);
    if w::button(lang.text("เลิกเป็นเครื่องลูก", "Stop being a shop device"))
        .danger_quiet()
        .size(w::Size::Sm)
        .show(ui)
        .clicked()
    {
        match std::fs::remove_file(app.data_dir.join(crate::worker::DeviceFile::FILE)) {
            Ok(()) => {
                app.toast(lang.text(
                    "เลิกเป็นเครื่องลูกแล้ว ปิดแอปแล้วเปิดใหม่เพื่อใช้ข้อมูลในเครื่องนี้",
                    "Done. Restart the app to use this computer's own data.",
                ));
            }
            Err(e) => app.error = Some(e.to_string()),
        }
    }
    let _ = t;
}
/// "Use this computer as a shop device": address + code from the main
/// register; pairing runs in the background and writes device.json.
fn join_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.add_space(space::S4);
    w::divider(ui);
    ui.label(t.text(
        lang.text("ใช้เครื่องนี้เป็นเครื่องลูก", "Use this computer as a shop device"),
        type_scale::BODY_STRONG,
    ));
    w::help_text(
        ui,
        lang.text(
            "เช่น เครื่องขายที่สองหรือจอครัว ที่เครื่องหลักกด \"จับคู่เครื่องใหม่\" แล้วใส่ที่อยู่และรหัสที่เห็น",
            "For a second register or a kitchen PC. On the main register press \"Pair a device\", then enter the address and code shown there.",
        ),
    );
    if !app.hub.join.open && app.hub.join.running.is_none() && app.hub.join.done.is_none() {
        if w::button(lang.text("ตั้งค่าเป็นเครื่องลูก…", "Set up as a shop device…"))
            .show(ui)
            .clicked()
        {
            app.hub.join.open = true;
        }
        return;
    }
    if let Some(rx) = &app.hub.join.running
        && let Ok(result) = rx.try_recv()
    {
        app.hub.join.running = None;
        app.hub.join.done = Some(result.map(|_| ()));
    }
    let busy = app.hub.join.running.is_some();
    if let Some(rx) = &app.hub.join.searching
        && let Ok(found) = rx.try_recv()
    {
        app.hub.join.searching = None;
        app.hub.join.found = found;
    }
    let searching = app.hub.join.searching.is_some();
    if w::button(if searching {
        lang.text("กำลังค้นหาเครื่องหลัก…", "Looking for main registers…")
    } else {
        lang.text("ค้นหาเครื่องหลักในร้าน", "Find main registers")
    })
    .icon(Icon::Search)
    .enabled(!searching && !busy)
    .loading(searching)
    .show(ui)
    .clicked()
    {
        let (tx, rx) = std::sync::mpsc::channel();
        app.hub.join.searching = Some(rx);
        std::thread::spawn(move || {
            let found =
                pos_hub::discovery::browse(std::time::Duration::from_secs(3)).unwrap_or_default();
            let _ = tx.send(found);
        });
    }
    if searching {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }
    for found in app.hub.join.found.clone() {
        let pin = found
            .fingerprint
            .get(..8)
            .map(|h| h.to_uppercase())
            .unwrap_or_default();
        let label = format!("{} · {} · {pin}", found.name, found.address);
        if w::chip(
            ui,
            &label,
            app.hub.join.address == found.address.to_string(),
            false,
        )
        .clicked()
        {
            app.hub.join.address = found.address.to_string();
        }
    }
    w::field_label(ui, lang.text("ที่อยู่เครื่องหลัก", "Main register address"), None);
    w::text_input(
        ui,
        &mut app.hub.join.address,
        w::FieldOpts {
            hint: "192.168.1.10:47810",
            limit: 64,
            ..Default::default()
        },
    );
    w::field_label(ui, lang.text("รหัสจับคู่ 6 หลัก", "6-digit pairing code"), None);
    w::text_input(
        ui,
        &mut app.hub.join.code,
        w::FieldOpts {
            hint: "123 456",
            limit: 7,
            ..Default::default()
        },
    );
    w::field_label(ui, lang.text("ชื่อเครื่องนี้", "Name of this device"), None);
    w::text_input(
        ui,
        &mut app.hub.join.name,
        w::FieldOpts {
            hint: lang.text("เช่น เคาน์เตอร์ 2", "e.g. Counter 2"),
            limit: 80,
            ..Default::default()
        },
    );
    let mut kitchen = app.hub.join.kitchen;
    w::segmented(
        ui,
        &mut kitchen,
        &[
            (
                false,
                lang.text("เครื่องขาย (ลิ้นชักของตัวเอง)", "Register (own drawer)"),
            ),
            (true, lang.text("จอครัว", "Kitchen screen")),
        ],
    );
    app.hub.join.kitchen = kitchen;
    let code: String = app
        .hub
        .join
        .code
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let address = app.hub.join.address.trim().to_owned();
    let address = if address.contains(':') {
        address
    } else {
        format!("{address}:{DEFAULT_PORT}")
    };
    let parsed: Option<std::net::SocketAddr> = address.parse().ok();
    let ready =
        parsed.is_some() && code.len() == 6 && !app.hub.join.name.trim().is_empty() && !busy;
    if w::button(if busy {
        lang.text("กำลังจับคู่…", "Pairing…")
    } else {
        lang.text("จับคู่กับเครื่องหลัก", "Pair with the main register")
    })
    .primary()
    .enabled(ready)
    .loading(busy)
    .show(ui)
    .clicked()
        && let Some(hub) = parsed
    {
        let name = app.hub.join.name.trim().to_owned();
        let kind = if kitchen {
            DeviceKind::Kitchen
        } else {
            DeviceKind::Register
        };
        let dir = app.data_dir.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        app.hub.join.running = Some(rx);
        app.hub.join.done = None;
        std::thread::spawn(move || {
            let device = uuid::Uuid::new_v4();
            let result = pos_hub::Client::pair(hub, &code, device, &name, kind)
                .map_err(|e| e.to_string())
                .and_then(|p| {
                    let file = crate::worker::DeviceFile {
                        hub,
                        device,
                        token: p.token,
                        fingerprint: crate::worker::DeviceFile::hex(&p.fingerprint),
                        name,
                        kind,
                        register: p.register,
                    };
                    file.save(&dir).map_err(|e| e.to_string())?;
                    Ok(file)
                });
            let _ = tx.send(result);
        });
    }
    if busy {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }
    match &app.hub.join.done {
        Some(Ok(())) => w::banner(
            ui,
            w::Tone::Success,
            lang.text(
                "จับคู่สำเร็จ ปิดแอปแล้วเปิดใหม่เพื่อเริ่มใช้งานเป็นเครื่องลูก",
                "Paired. Restart the app to start working as a shop device.",
            ),
            None,
        ),
        Some(Err(e)) => {
            w::inline_error(
                ui,
                &format!("{} {e}", lang.text("จับคู่ไม่สำเร็จ:", "Pairing failed:")),
            );
            false
        }
        None => false,
    };
}

pub fn card(app: &mut App, ui: &mut egui::Ui) {
    if let Some(device) = app.hub.device.clone() {
        device_card(app, ui, &device);
        return;
    }
    hub_card(app, ui);
    if !app.hub.settings.enabled && app.data.devices.is_empty() {
        join_card(app, ui);
    }
}
fn hub_card(app: &mut App, ui: &mut egui::Ui) {
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
    if let Some(port) = app.hub.hub.as_ref().map(|h| h.address().port()) {
        ipad_section(app, ui, port);
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

/// How to add an ordering iPad: scan the setup page, install the shop
/// certificate once, then pair the web app with a code like any device.
fn ipad_section(app: &App, ui: &mut egui::Ui, port: u16) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.add_space(space::S3);
    ui.label(t.text(
        lang.text("iPad รับออเดอร์", "Ordering iPads"),
        type_scale::BODY_STRONG,
    ));
    if let Some(e) = &app.hub.web_error {
        w::inline_error(
            ui,
            &format!(
                "{} {e}",
                lang.text(
                    "เปิดแอปรับออเดอร์ไม่สำเร็จ:",
                    "Could not start the ordering app:"
                )
            ),
        );
        return;
    }
    let host = lan_address()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "127.0.0.1".into());
    let (https, http) = web_ports(port);
    let setup = format!("http://{host}:{http}/setup");
    ui.horizontal_top(|ui| {
        crate::ui::qr::show(ui, &setup, 132.0);
        ui.add_space(space::S3);
        ui.vertical(|ui| {
            w::help_text(
                ui,
                lang.text(
                    "สแกนด้วยกล้อง iPad (ต่อ Wi-Fi ร้านเดียวกัน) แล้วทำตามขั้นตอน: ติดตั้งใบรับรองของร้านครั้งเดียว เปิดแอปรับออเดอร์ แล้วกด \"จับคู่เครื่องใหม่\" ที่นี่เพื่อรับรหัส",
                    "Scan with the iPad camera (on the shop Wi-Fi) and follow the steps: install the shop certificate once, open the ordering app, then press \"Pair a device\" here for a code.",
                ),
            );
            w::kv(
                ui,
                lang.text("หน้าตั้งค่า", "Setup page"),
                &setup,
                w::Value::Plain,
            );
            w::kv(
                ui,
                lang.text("แอปรับออเดอร์", "Ordering app"),
                &format!("https://{host}:{https}/"),
                w::Value::Plain,
            );
        });
    });
}
