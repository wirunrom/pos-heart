//! Staff sign-in (lock screen), manager approval and the staff settings card.
//! Approval follows design/docs/04-patterns.md §6: a lock icon and "needs a
//! manager's approval", then a PIN in the same dialog; the record keeps who
//! asked and who approved.
use crate::{
    app::App,
    i18n::Language,
    ui::{Icon, Theme, icons, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, Align2, Key, Sense, Stroke, StrokeKind, vec2};
use pos_domain::{Access, OWNER, Permission, RoleDef, Staff, StaffInput, StaffRef};
use uuid::Uuid;

#[derive(Default)]
pub struct StaffState {
    /// Lock screen: chosen member and typed PIN.
    pub chosen: Option<Uuid>,
    pub pin: String,
    /// PIN errors shown inside the lock screen or approval dialog.
    pub pin_error: Option<String>,
    /// A restricted command waiting for a manager's PIN.
    pub approval: Option<Approval>,
    pub editor: Option<Editor>,
    pub role_editor: Option<RoleEditor>,
}
pub struct RoleEditor {
    def: RoleDef,
    limit: String,
}
pub struct Approval {
    pub permission: Permission,
    pub command: Command,
    pub approver: Option<Uuid>,
    pub pin: String,
}
pub struct Editor {
    id: Option<Uuid>,
    name: String,
    role: String,
    pin: String,
    confirm: String,
    active: bool,
}

/// Locked while staff exist and nobody is signed in.
pub fn locked(app: &App) -> bool {
    app.ready && app.data.staff_enabled && app.data.signed_in.is_none()
}
pub fn role_of<'a>(app: &'a App, id: &str) -> Option<&'a RoleDef> {
    app.data.roles.iter().find(|r| r.id == id)
}
/// Role name in the current language.
pub fn role_label(app: &App, id: &str) -> String {
    let stored = role_of(app, id).map_or(id, |r| r.name.as_str());
    app.lang.role_name(id, stored)
}
/// What the signed-in member's role says about `permission`; Allow without staff.
pub fn access(app: &App, permission: Permission) -> Access {
    if !app.data.staff_enabled {
        return Access::Allow;
    }
    app.data
        .signed_in
        .as_ref()
        .and_then(|s| role_of(app, &s.role))
        .map_or(Access::Deny, |r| r.access(permission))
}
/// May the signed-in member do this without approval? True without staff.
pub fn may(app: &App, permission: Permission) -> bool {
    access(app, permission) == Access::Allow
}
/// Discount share (basis points) the signed-in member may give alone.
pub fn discount_limit(app: &App) -> u32 {
    if may(app, Permission::Discount) {
        return 10_000;
    }
    app.data
        .signed_in
        .as_ref()
        .and_then(|s| role_of(app, &s.role))
        .map_or(0, |r| r.discount_limit_bp)
}
/// Members who may approve `permission`, other than the requester.
fn approvers(app: &App, permission: Permission) -> Vec<Staff> {
    let me = app.data.signed_in.as_ref().map(|s| s.id);
    app.data
        .staff
        .iter()
        .filter(|s| {
            s.active
                && Some(s.id) != me
                && role_of(app, &s.role).is_some_and(|r| r.approves(permission))
        })
        .cloned()
        .collect()
}
/// Called when the worker refuses a command for lack of permission.
pub fn request_approval(app: &mut App, permission: Permission, command: Option<Command>) {
    let Some(command) = command else {
        return;
    };
    // Unwrap an earlier failed approval so it is not nested.
    let command = match command {
        Command::Approved { inner, .. } => *inner,
        c => c,
    };
    let approver = approvers(app, permission).first().map(|s| s.id);
    app.staff.pin_error = None;
    app.staff.approval = Some(Approval {
        permission,
        command,
        approver,
        pin: String::new(),
    });
}

/// Digits typed on the keyboard or tapped on the pad. Returns true on Enter / OK.
fn pin_pad(
    ui: &mut egui::Ui,
    pin: &mut String,
    lang: Language,
    enabled: bool,
    large: bool,
) -> bool {
    let t = Theme::of(ui);
    let mut submit = false;
    if enabled {
        ui.input(|i| {
            for e in &i.events {
                if let egui::Event::Text(text) = e {
                    for c in text.chars().filter(char::is_ascii_digit) {
                        if pin.len() < 8 {
                            pin.push(c);
                        }
                    }
                }
            }
            if i.key_pressed(Key::Backspace) {
                pin.pop();
            }
            if i.key_pressed(Key::Enter) {
                submit = true;
            }
        });
    }
    // Dots for typed digits.
    let dot = 14.0;
    let (rect, _) = ui.allocate_exact_size(vec2(8.0 * (dot + 10.0), dot + 12.0), Sense::hover());
    let n = pin.len().max(4);
    let start = rect.center().x - (n as f32 * (dot + 10.0) - 10.0) / 2.0;
    for i in 0..n {
        let c = egui::pos2(start + i as f32 * (dot + 10.0) + dot / 2.0, rect.center().y);
        if i < pin.len() {
            ui.painter().circle_filled(c, dot / 2.0, t.p.fg_default);
        } else {
            ui.painter()
                .circle_stroke(c, dot / 2.0 - 1.0, Stroke::new(1.5, t.p.border_control));
        }
    }
    let key = if large {
        (t.m.control_lg + 8.0).max(52.0)
    } else {
        t.m.control_md.max(40.0)
    };
    let pad_w = key * 1.4 * 3.0 + space::S2 * 2.0;
    let keys = [
        ["1", "2", "3"],
        ["4", "5", "6"],
        ["7", "8", "9"],
        ["⌫", "0", "OK"],
    ];
    let offset = ((ui.available_width() - pad_w) / 2.0).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(offset);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = space::S2;
            ui.add_enabled_ui(enabled, |ui| {
                for row in keys {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = space::S2;
                        for k in row {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(key * 1.4, key), Sense::click());
                            let fill = if resp.hovered() {
                                t.p.bg_hover
                            } else if k == "OK" {
                                t.p.accent_soft
                            } else {
                                t.p.bg_surface
                            };
                            ui.painter().rect(
                                r,
                                t.control_radius(),
                                fill,
                                Stroke::new(1.0, t.p.border_default),
                                StrokeKind::Inside,
                            );
                            match k {
                                "⌫" => icons::paint_sized(
                                    ui.painter(),
                                    Icon::ChevronLeft,
                                    r.center(),
                                    20.0,
                                    t.p.fg_default,
                                ),
                                _ => {
                                    ui.painter().text(
                                        r.center(),
                                        Align2::CENTER_CENTER,
                                        if k == "OK" {
                                            lang.text("ตกลง", "OK")
                                        } else {
                                            k
                                        },
                                        t.strong_font(type_scale::TITLE),
                                        t.p.fg_default,
                                    );
                                }
                            }
                            let label = match k {
                                "⌫" => lang.text("ลบ", "Delete"),
                                "OK" => lang.text("ตกลง", "OK"),
                                d => d,
                            };
                            resp.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                            });
                            if resp.clicked() {
                                match k {
                                    "⌫" => {
                                        pin.pop();
                                    }
                                    "OK" => submit = true,
                                    d if pin.len() < 8 => pin.push_str(d),
                                    _ => {}
                                }
                            }
                        }
                    });
                }
            });
        });
    });
    submit
}
fn member_chip(ui: &mut egui::Ui, s: &Staff, selected: bool, role: &str) -> egui::Response {
    w::chip(ui, &format!("{} · {role}", s.name), selected, false)
}

/// Full-screen sign-in shown while locked.
pub fn lock_screen(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let members: Vec<Staff> = app
        .data
        .staff
        .iter()
        .filter(|s| s.active)
        .cloned()
        .collect();
    if app
        .staff
        .chosen
        .is_none_or(|id| !members.iter().any(|m| m.id == id))
    {
        app.staff.chosen = members.first().map(|m| m.id);
    }
    egui::ScrollArea::vertical().id_salt("lock").show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.set_max_width(520.0);
            ui.add_space(space::S8);
            icons::show(ui, Icon::Lock, 32.0, t.p.fg_muted);
            w::page_title(ui, &app.shop.name);
            ui.label(t.muted(
                lang.text(
                    "เลือกชื่อของคุณแล้วใส่ PIN",
                    "Choose your name and enter your PIN",
                ),
                type_scale::BODY,
            ));
            ui.add_space(space::S4);
            ui.horizontal_wrapped(|ui| {
                for m in &members {
                    let selected = app.staff.chosen == Some(m.id);
                    let role = role_label(app, &m.role);
                    if member_chip(ui, m, selected, &role).clicked() {
                        app.staff.chosen = Some(m.id);
                        app.staff.pin.clear();
                        app.staff.pin_error = None;
                    }
                }
            });
            ui.add_space(space::S4);
            let submit = pin_pad(ui, &mut app.staff.pin, lang, !app.busy, true);
            if let Some(e) = &app.staff.pin_error {
                w::inline_error(ui, e);
            }
            if submit
                && !app.busy
                && let Some(id) = app.staff.chosen
                && pos_domain::valid_pin(&app.staff.pin)
            {
                let pin = std::mem::take(&mut app.staff.pin);
                app.staff.pin_error = None;
                app.send(Command::SignIn(id, pin));
            }
            ui.add_space(space::S4);
            w::help_text(
                ui,
                lang.text(
                    "ลืม PIN ให้เจ้าของร้านตั้งใหม่ในตั้งค่า → พนักงานและสิทธิ์",
                    "Forgot your PIN? An owner can set a new one in Settings → Staff & permissions.",
                ),
            );
        });
    });
}

/// "Needs a manager's approval" with a PIN, in a dialog over the current work.
pub fn approval_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut approval) = app.staff.approval.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let approvers = approvers(app, approval.permission);
    // Digits go to the PIN, not to a field focused behind the dialog.
    ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    let response = w::dialog(ctx, "approval", 460.0, |ui| {
        ui.horizontal(|ui| {
            icons::show(ui, Icon::Lock, 22.0, t.p.warning_fg);
            ui.label(t.text(
                lang.text("ต้องให้ผู้มีสิทธิ์อนุมัติ", "Needs approval"),
                type_scale::TITLE,
            ));
        });
        ui.label(t.muted(
            format!(
                "{} · {} {}",
                lang.permission(approval.permission),
                lang.text("ขอโดย", "requested by"),
                app.data
                    .signed_in
                    .as_ref()
                    .map_or("—", |s: &StaffRef| s.name.as_str())
            ),
            type_scale::BODY,
        ));
        ui.add_space(space::S2);
        ui.horizontal_wrapped(|ui| {
            for m in &approvers {
                let role = role_label(app, &m.role);
                if member_chip(ui, m, approval.approver == Some(m.id), &role).clicked() {
                    approval.approver = Some(m.id);
                    approval.pin.clear();
                    app.staff.pin_error = None;
                }
            }
        });
        if approvers.is_empty() {
            w::inline_error(
                ui,
                lang.text(
                    "ยังไม่มีผู้จัดการหรือเจ้าของร้านที่อนุมัติได้",
                    "No active manager or owner can approve this.",
                ),
            );
        }
        ui.vertical_centered(|ui| {
            let submit = pin_pad(ui, &mut approval.pin, lang, !app.busy, false);
            if let Some(e) = &app.staff.pin_error {
                w::inline_error(ui, e);
            }
            let ready = approval.approver.is_some() && pos_domain::valid_pin(&approval.pin);
            let mut out = None;
            if submit && ready && !app.busy {
                out = Some(true);
            }
            w::dialog_actions(ui, |ui| {
                if w::button(lang.text("อนุมัติ", "Approve"))
                    .primary()
                    .enabled(ready)
                    .loading(app.busy)
                    .show(ui)
                    .clicked()
                {
                    out = Some(true);
                }
                if w::button(lang.text("ยกเลิก", "Cancel"))
                    .ghost()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
                {
                    out = Some(false);
                }
            });
            out
        })
        .inner
    });
    match response.inner {
        Some(false) => app.staff.pin_error = None,
        None if !app.busy && response.should_close() => app.staff.pin_error = None,
        Some(true) => {
            if let Some(approver) = approval.approver {
                let pin = std::mem::take(&mut approval.pin);
                app.staff.pin_error = None;
                let command = Command::Approved {
                    approver,
                    pin,
                    permission: approval.permission,
                    inner: Box::new(approval.command.clone()),
                };
                app.staff.approval = Some(approval);
                app.send(command);
            }
        }
        None => app.staff.approval = Some(approval),
    }
}

/// Top-bar chip with the signed-in name; opens "Lock" / "Switch user".
pub fn topbar_chip(app: &mut App, ui: &mut egui::Ui) {
    let Some(me) = app.data.signed_in.clone() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::of(ui);
    let role = role_label(app, &me.role);
    let r = w::status_pill(ui, &me.name, None, false).on_hover_text(format!(
        "{role} · {}",
        lang.text("ล็อกเครื่อง (Ctrl L)", "Lock (Ctrl L)")
    ));
    egui::Popup::menu(&r)
        .frame(
            t.raised(2)
                .inner_margin(egui::Margin::same(space::S1 as i8)),
        )
        .width(220.0)
        .show(|ui| {
            ui.label(t.muted(&role, type_scale::CAPTION));
            if crate::shell::menu_row(
                ui,
                Icon::Lock,
                lang.text("ล็อกเครื่อง / สลับผู้ใช้", "Lock / switch user"),
                "Ctrl L",
                false,
            )
            .clicked()
            {
                lock(app);
            }
        });
}
pub fn lock(app: &mut App) {
    if app.data.staff_enabled && !app.busy {
        app.staff.pin.clear();
        app.staff.pin_error = None;
        app.send(Command::SignOut);
    }
}

/// Settings → Staff. Only owners edit; everyone sees who is on the roster.
pub fn settings_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("พนักงานและสิทธิ์", "Staff & permissions"));
    if !app.data.staff_enabled {
        w::help_text(
            ui,
            lang.text(
                "ตอนนี้ทุกคนใช้เครื่องได้โดยไม่ต้องเข้าระบบ เพิ่มพนักงานเพื่อให้ทุกบิล กะ และการคืนเงินบันทึกชื่อผู้ทำ และให้ผู้จัดการอนุมัติการคืนเงิน ส่วนลดเกินกำหนด และเงินเข้า-ออก คนแรกต้องเป็นเจ้าของร้าน",
                "Anyone can use this register without signing in. Add staff to record who made each sale, shift and refund, and to have a manager approve refunds, large discounts and cash in/out. The first person must be the owner.",
            ),
        );
        if w::button(lang.text("เริ่มใช้ระบบพนักงาน", "Start using staff sign-in"))
            .icon(Icon::Lock)
            .enabled(app.ready && !app.busy)
            .show(ui)
            .clicked()
        {
            app.staff.editor = Some(Editor {
                id: None,
                name: String::new(),
                role: OWNER.into(),
                pin: String::new(),
                confirm: String::new(),
                active: true,
            });
        }
        return;
    }
    let owner = app.data.signed_in.as_ref().is_some_and(|s| s.role == OWNER);
    for s in app.data.staff.clone() {
        ui.horizontal(|ui| {
            ui.label(t.text(&s.name, type_scale::BODY_STRONG));
            w::badge(
                ui,
                &role_label(app, &s.role),
                if s.role == OWNER || s.role == pos_domain::MANAGER {
                    w::Tone::Info
                } else {
                    w::Tone::Neutral
                },
            );
            if !s.active {
                w::badge(ui, lang.text("ปิดใช้งาน", "Off"), w::Tone::Neutral);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if owner
                    && w::button(lang.text("แก้ไข", "Edit"))
                        .ghost()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                {
                    app.staff.editor = Some(Editor {
                        id: Some(s.id),
                        name: s.name.clone(),
                        role: s.role.clone(),
                        pin: String::new(),
                        confirm: String::new(),
                        active: s.active,
                    });
                }
            });
        });
    }
    if !owner {
        w::help_text(
            ui,
            lang.text(
                "เฉพาะเจ้าของร้านเพิ่มหรือแก้ไขพนักงานได้",
                "Only an owner can add or edit staff.",
            ),
        );
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if w::button(lang.text("เพิ่มพนักงาน", "Add staff"))
            .icon(Icon::Plus)
            .enabled(!app.busy)
            .show(ui)
            .clicked()
        {
            app.staff.editor = Some(Editor {
                id: None,
                name: String::new(),
                role: pos_domain::CASHIER.into(),
                pin: String::new(),
                confirm: String::new(),
                active: true,
            });
        }
    });
    ui.add_space(space::S3);
    w::field_label(ui, lang.text("บทบาทและสิทธิ์", "Roles & permissions"), None);
    w::help_text(
        ui,
        lang.text(
            "แต่ละสิทธิ์ตั้งได้ ทำได้เอง / ต้องให้คนอื่นอนุมัติ / ห้าม และเพดานส่วนลดที่ให้ได้เอง บทบาทเจ้าของร้านแก้ไม่ได้",
            "Each permission can be allowed, need someone's approval, or be denied, with a discount limit per role. The owner role is fixed.",
        ),
    );
    for role in app.data.roles.clone() {
        ui.horizontal(|ui| {
            ui.label(t.text(role_label(app, &role.id), type_scale::BODY_STRONG));
            let held = app.data.staff.iter().filter(|s| s.role == role.id).count();
            ui.label(t.muted(
                format!(
                    "{held} {}",
                    lang.noun(held as u64, "คน", "person", "people")
                ),
                type_scale::CAPTION,
            ));
            if role.can_approve || role.is_owner() {
                w::badge(ui, lang.text("อนุมัติได้", "Approves"), w::Tone::Success);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if !role.is_owner()
                    && w::button(lang.text("แก้สิทธิ์", "Edit"))
                        .ghost()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                {
                    app.staff.role_editor = Some(RoleEditor {
                        limit: percent_text(role.discount_limit_bp),
                        def: role.clone(),
                    });
                }
            });
        });
    }
    if w::button(lang.text("เพิ่มบทบาท", "Add role"))
        .icon(Icon::Plus)
        .enabled(!app.busy)
        .show(ui)
        .clicked()
    {
        let mut def = RoleDef::cashier(1_000);
        def.id = String::new();
        def.name = String::new();
        app.staff.role_editor = Some(RoleEditor {
            limit: percent_text(def.discount_limit_bp),
            def,
        });
    }
}
fn percent_text(bp: u32) -> String {
    if bp == 0 {
        return "0".into();
    }
    crate::ui::format::discount_rule(pos_domain::Discount::Percent(bp))
        .trim_end_matches('%')
        .to_owned()
}

/// Role editor: name, a three-way setting per permission, approval and limit.
pub fn role_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut ed) = app.staff.role_editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let builtin = [pos_domain::MANAGER, pos_domain::CASHIER].contains(&ed.def.id.as_str());
    let limit = crate::ui::format::parse_money(&ed.limit)
        .ok()
        .and_then(|m| u32::try_from(m.satang()).ok())
        .filter(|bp| *bp <= 10_000);
    let response = w::dialog(ctx, "role-editor", 620.0, |ui| {
        w::dialog_title(
            ui,
            if ed.def.id.is_empty() {
                lang.text("เพิ่มบทบาท", "Add role")
            } else {
                lang.text("แก้สิทธิ์ของบทบาท", "Edit role permissions")
            },
            false,
        );
        w::field_label(ui, lang.text("ชื่อบทบาท", "Role name"), None);
        if builtin {
            ui.label(t.text(role_label(app, &ed.def.id), type_scale::BODY_STRONG));
        } else {
            w::text_input(
                ui,
                &mut ed.def.name,
                w::FieldOpts {
                    hint: lang.text("เช่น หัวหน้ากะ", "e.g. Shift lead"),
                    limit: 40,
                    ..Default::default()
                },
            );
        }
        let reserve = if t.touch() { 500.0 } else { 400.0 };
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - reserve).clamp(120.0, 440.0))
            .show(ui, |ui| {
                for p in Permission::ALL {
                    if p == Permission::ManageStaff {
                        continue;
                    }
                    // Each row has its own ID scope: the segments share labels.
                    ui.push_id(p, |ui| {
                        ui.horizontal(|ui| {
                            let label_w = (ui.available_width() - 300.0).max(160.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(label_w, t.m.control_md),
                                egui::Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.set_min_width(label_w);
                                    ui.add(
                                        egui::Label::new(
                                            t.text(lang.permission(p), type_scale::BODY),
                                        )
                                        .wrap(),
                                    );
                                },
                            );
                            let mut a = ed.def.access(p);
                            if w::segmented(
                                ui,
                                &mut a,
                                &[
                                    (Access::Allow, lang.access(Access::Allow)),
                                    (Access::Approval, lang.access(Access::Approval)),
                                    (Access::Deny, lang.access(Access::Deny)),
                                ],
                            ) {
                                ed.def.access.insert(p, a);
                            }
                        });
                    });
                }
            });
        w::help_text(
            ui,
            lang.text(
                "จัดการพนักงานและบทบาทเป็นสิทธิ์ของเจ้าของร้านเท่านั้น · รายงานและการตั้งค่าที่ไม่ใช่ \"ทำได้เอง\" จะเปิดไม่ได้",
                "Managing staff and roles is owner-only · reports and settings open only when allowed.",
            ),
        );
        ui.horizontal(|ui| {
            ui.label(t.text(
                lang.text("อนุมัติให้คนอื่นได้", "May approve others"),
                type_scale::BODY,
            ));
            w::switch(
                ui,
                &mut ed.def.can_approve,
                lang.text("อนุมัติให้คนอื่นได้", "May approve others"),
            );
            ui.label(t.muted(
                lang.text("เฉพาะเรื่องที่บทบาทนี้ทำได้เอง", "only what this role is allowed"),
                type_scale::CAPTION,
            ));
        });
        ui.horizontal(|ui| {
            ui.label(t.text(
                lang.text("ส่วนลดที่ให้ได้เอง", "Discount without approval"),
                type_scale::BODY,
            ));
            w::text_input(
                ui,
                &mut ed.limit,
                w::FieldOpts {
                    hint: "10",
                    limit: 6,
                    invalid: limit.is_none(),
                    width: Some(90.0),
                    ..Default::default()
                },
            );
            ui.label(t.text(lang.text("% ของบิล", "% of the bill"), type_scale::BODY));
        });
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let valid = limit.is_some() && !ed.def.name.trim().is_empty();
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
                .enabled(valid && !app.busy)
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(1);
            }
            if w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .show(ui)
                .clicked()
            {
                out = Some(0);
            }
            if !builtin
                && !ed.def.id.is_empty()
                && w::button(lang.text("ลบบทบาท", "Delete role"))
                    .danger_quiet()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
            {
                out = Some(2);
            }
        });
        out
    });
    match response.inner {
        Some(1) => {
            ed.def.access.insert(Permission::ManageStaff, Access::Deny);
            if let Some(bp) = limit {
                ed.def.discount_limit_bp = bp;
            }
            let command = Command::SaveRole(ed.def.clone());
            app.staff.role_editor = Some(ed);
            app.send(command);
        }
        Some(2) => {
            let command = Command::DeleteRole(ed.def.id.clone());
            app.staff.role_editor = Some(ed);
            app.send(command);
        }
        Some(_) => app.error = None,
        None if response.should_close() => {}
        None => app.staff.role_editor = Some(ed),
    }
}

/// Add / edit staff dialog (owner only, or the first owner).
pub fn editor_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut ed) = app.staff.editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let first = !app.data.staff_enabled;
    let pin_ok = if ed.id.is_some() && ed.pin.is_empty() {
        true
    } else {
        pos_domain::valid_pin(&ed.pin) && ed.pin == ed.confirm
    };
    let valid = !ed.name.trim().is_empty() && ed.name.trim().chars().count() <= 60 && pin_ok;
    let response = w::dialog(ctx, "staff-editor", 460.0, |ui| {
        w::dialog_title(
            ui,
            if first {
                lang.text("ตั้งเจ้าของร้าน", "Set up the owner")
            } else if ed.id.is_some() {
                lang.text("แก้ไขพนักงาน", "Edit staff")
            } else {
                lang.text("เพิ่มพนักงาน", "Add staff")
            },
            false,
        );
        w::field_label(ui, lang.text("ชื่อ", "Name"), None);
        w::text_input(
            ui,
            &mut ed.name,
            w::FieldOpts {
                hint: lang.text("เช่น นิด", "e.g. Nid"),
                limit: 60,
                ..Default::default()
            },
        );
        if !first {
            w::field_label(ui, lang.text("บทบาท", "Role"), None);
            ui.horizontal_wrapped(|ui| {
                for role in app.data.roles.clone() {
                    let label = role_label(app, &role.id);
                    if w::chip(ui, &label, ed.role == role.id, false).clicked() {
                        ed.role = role.id.clone();
                    }
                }
            });
            if let Some(role) = role_of(app, &ed.role) {
                let list = |want: Access| -> String {
                    Permission::ALL
                        .iter()
                        .filter(|p| role.access(**p) == want)
                        .map(|p| lang.permission(*p))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                for want in [Access::Allow, Access::Approval, Access::Deny] {
                    let items = list(want);
                    if !items.is_empty() {
                        w::help_text(ui, &format!("{}: {items}", lang.access(want)));
                    }
                }
            }
        }
        w::field_label(
            ui,
            if ed.id.is_some() {
                lang.text("PIN ใหม่", "New PIN")
            } else {
                lang.text("PIN", "PIN")
            },
            Some(if ed.id.is_some() {
                lang.text("เว้นว่างเพื่อใช้ PIN เดิม", "leave empty to keep")
            } else {
                lang.text("ตัวเลข 4–8 หลัก", "4–8 digits")
            }),
        );
        for (value, hint) in [
            (&mut ed.pin, lang.text("PIN", "PIN")),
            (&mut ed.confirm, lang.text("ใส่ PIN อีกครั้ง", "Repeat PIN")),
        ] {
            ui.add(
                egui::TextEdit::singleline(value)
                    .password(true)
                    .hint_text(hint)
                    .char_limit(8)
                    .desired_width(160.0)
                    .font(t.font(type_scale::MONEY)),
            );
            value.retain(|c| c.is_ascii_digit());
        }
        if !ed.pin.is_empty() && !pin_ok {
            w::inline_error(
                ui,
                lang.text(
                    "PIN ต้องเป็นตัวเลข 4–8 หลัก และพิมพ์ให้ตรงกันทั้งสองช่อง",
                    "Use 4–8 digits and type the same PIN twice.",
                ),
            );
        }
        if ed.id.is_some() {
            ui.horizontal(|ui| {
                ui.label(t.text(lang.text("ใช้งานอยู่", "Active"), type_scale::BODY));
                w::switch(ui, &mut ed.active, lang.text("ใช้งานอยู่", "Active"));
            });
        }
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
                .enabled(valid && !app.busy)
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("ยกเลิก", "Cancel"))
                .ghost()
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out
    });
    match response.inner {
        Some(true) => {
            let input = StaffInput {
                id: ed.id,
                name: ed.name.trim().to_owned(),
                role: Some(if first { OWNER.into() } else { ed.role.clone() }),
                pin: (!ed.pin.is_empty()).then(|| ed.pin.clone()),
                active: ed.active,
            };
            app.staff.editor = Some(ed);
            app.send(Command::SaveStaff(input));
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.staff.editor = Some(ed),
    }
}

#[cfg(debug_assertions)]
pub fn preview_role_editor(app: &mut App) {
    if let Some(role) = app.data.roles.iter().find(|r| r.name == "หัวหน้ากะ").cloned()
    {
        app.staff.role_editor = Some(RoleEditor {
            limit: percent_text(role.discount_limit_bp),
            def: role,
        });
    }
}
