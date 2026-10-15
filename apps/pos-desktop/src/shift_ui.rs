//! Shift & drawer workspace (design/docs/components/ScreenShiftClose.md and
//! .impeccable/surfaces/shifts.md). Shift sales and drawer cash are separate
//! values; only cash affects the expected drawer.
use crate::{
    app::{App, Page},
    i18n::Language,
    ui::{Icon, Theme, format, icons, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, Id, RichText, Sense, Stroke, vec2};
use pos_domain::{
    CashCount, CashKind, CashMovement, Money, PaymentMethod, Shift, ShiftCount, SignedMoney,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    Close,
    History,
}
/// Thai notes and coins counted at closing (satang).
const DENOMINATIONS: [(i64, bool); 9] = [
    (100_000, true),
    (50_000, true),
    (10_000, true),
    (5_000, true),
    (2_000, true),
    (1_000, false),
    (500, false),
    (200, false),
    (100, false),
];
pub struct Editor {
    pub tab: Tab,
    pub opening: String,
    pub amount: String,
    pub reason: String,
    pub kind: CashKind,
    pub counts: Vec<String>,
    pub other: String,
    pub close_reason: String,
    pub reviewing: bool,
    draft_for: Option<Uuid>,
    draft_saved: Option<i64>,
    draft_dirty: bool,
    pub open_id: Uuid,
    pub move_id: Uuid,
    pub pending: Option<Pending>,
    pub selected: Option<Shift>,
    pub history_offset: u32,
    pub movement_offset: u32,
}
pub enum Pending {
    Move(CashMovement),
    Close(Box<Shift>, ShiftCount),
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            tab: Tab::Overview,
            opening: String::new(),
            amount: String::new(),
            reason: String::new(),
            kind: CashKind::CashIn,
            counts: vec![String::new(); DENOMINATIONS.len()],
            other: String::new(),
            close_reason: String::new(),
            reviewing: false,
            draft_for: None,
            draft_saved: None,
            draft_dirty: false,
            open_id: Uuid::new_v4(),
            move_id: Uuid::new_v4(),
            pending: None,
            selected: None,
            history_offset: 0,
            movement_offset: 0,
        }
    }
}
/// Local autosaved count so a closing can be resumed after a restart.
#[derive(Serialize, Deserialize)]
struct Draft {
    version: u32,
    shift_id: Uuid,
    counts: Vec<String>,
    other: String,
    reason: String,
    saved_at: i64,
}
fn draft_path(app: &App, id: Uuid) -> std::path::PathBuf {
    app.data_dir.join(format!("drawer-count-{id}.json"))
}
impl Editor {
    fn denominations(&self) -> Option<Vec<CashCount>> {
        let mut out = Vec::new();
        for ((value, _), text) in DENOMINATIONS.iter().zip(&self.counts) {
            let text = text.trim().replace(',', "");
            let count = if text.is_empty() {
                0
            } else {
                text.parse::<u32>().ok()?
            };
            if count > 0 {
                out.push(CashCount {
                    value: Money::from_satang(*value).ok()?,
                    count,
                });
            }
        }
        let other = if self.other.trim().is_empty() {
            Money::ZERO
        } else {
            format::parse_money(&self.other).ok()?
        };
        if other != Money::ZERO {
            out.push(CashCount {
                value: other,
                count: 1,
            });
        }
        Some(out)
    }
    fn counted(&self) -> Option<Money> {
        ShiftCount::total(&self.denominations()?).ok()
    }
    fn touched(&self) -> bool {
        self.counts.iter().any(|c| !c.trim().is_empty()) || !self.other.trim().is_empty()
    }
    fn reset_count(&mut self) {
        self.counts = vec![String::new(); DENOMINATIONS.len()];
        self.other.clear();
        self.close_reason.clear();
        self.reviewing = false;
        self.draft_saved = None;
        self.draft_dirty = false;
    }
}
pub fn load_draft(app: &mut App, id: Uuid) {
    if app.drawer.draft_for == Some(id) {
        return;
    }
    app.drawer.reset_count();
    app.drawer.draft_for = Some(id);
    if let Ok(bytes) = std::fs::read(draft_path(app, id))
        && let Ok(d) = serde_json::from_slice::<Draft>(&bytes)
        && d.version == 1
        && d.shift_id == id
        && d.counts.len() == DENOMINATIONS.len()
    {
        app.drawer.counts = d.counts;
        app.drawer.other = d.other;
        app.drawer.close_reason = d.reason;
        app.drawer.draft_saved = Some(d.saved_at);
    }
}
fn save_draft(app: &mut App, id: Uuid) {
    if !app.drawer.draft_dirty {
        return;
    }
    app.drawer.draft_dirty = false;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let draft = Draft {
        version: 1,
        shift_id: id,
        counts: app.drawer.counts.clone(),
        other: app.drawer.other.clone(),
        reason: app.drawer.close_reason.clone(),
        saved_at: now,
    };
    let path = draft_path(app, id);
    let temp = path.with_extension("tmp");
    let written = serde_json::to_vec(&draft)
        .map_err(std::io::Error::other)
        .and_then(|bytes| std::fs::write(&temp, bytes))
        .and_then(|()| std::fs::rename(&temp, &path));
    if written.is_ok() {
        app.drawer.draft_saved = Some(now);
    }
}
pub fn shift_closed(app: &mut App, shift: &Shift) {
    let _ = std::fs::remove_file(draft_path(app, shift.id));
    app.drawer.reset_count();
    app.drawer.draft_for = None;
    app.drawer.pending = None;
    app.drawer.selected = Some(shift.clone());
    app.drawer.tab = Tab::History;
    app.drawer.history_offset = 0;
    let lang = app.lang;
    app.toast(lang.text(
        "ปิดกะแล้ว บันทึกยอดนับถาวร",
        "Shift closed. The count is saved permanently.",
    ));
    // "Close and print summary" (ScreenShiftClose.md) when a printer is set up.
    app.print_shift(shift);
}

fn kind_label(lang: Language, k: CashKind, method: PaymentMethod) -> String {
    let base = match k {
        CashKind::Sale => lang.text("ขาย", "Sale"),
        CashKind::Refund => lang.text("คืนเงิน", "Refund"),
        CashKind::CashIn => lang.text("เงินเข้า", "Cash in"),
        CashKind::CashOut => lang.text("เงินออก", "Cash out"),
    };
    if matches!(k, CashKind::Sale | CashKind::Refund) {
        format!("{base} · {}", lang.payment_method(method))
    } else {
        base.to_owned()
    }
}
fn number(app: &App, shift: &Shift) -> String {
    app.data
        .shift_numbers
        .get(&shift.id)
        .map_or_else(String::new, |n| format!("#{n}"))
}
fn difference_label(lang: Language, amount: SignedMoney) -> (&'static str, w::Tone) {
    match amount.satang().cmp(&0) {
        std::cmp::Ordering::Less => (lang.text("ขาด", "Short"), w::Tone::Danger),
        std::cmp::Ordering::Greater => (lang.text("เกิน", "Over"), w::Tone::Warning),
        std::cmp::Ordering::Equal => (lang.text("ตรง", "Balanced"), w::Tone::Success),
    }
}

/// Shift toolbar: shift name, Overview / Count & close / History and the
/// close steps on the right.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let shift = app.data.shift.clone();
    match &shift {
        Some(s) => {
            ui.label(t.chrome(
                format!("{} {}", lang.text("กะ", "Shift"), number(app, s)),
                type_scale::BODY_STRONG,
            ));
            ui.label(t.chrome_muted(
                format!(
                    "{} {} · {}",
                    lang.text("เปิด", "opened"),
                    format::clock(s.opened_at),
                    format::duration(lang, now - s.opened_at)
                ),
                type_scale::CAPTION,
            ));
        }
        None => {
            ui.label(t.chrome(
                lang.text("ยังไม่เปิดกะ", "No open shift"),
                type_scale::BODY_STRONG,
            ));
        }
    }
    crate::chrome::separator(ui, &t);
    let mut tab = app.drawer.tab;
    let options: Vec<(Tab, String)> = if shift.is_some() {
        vec![
            (Tab::Overview, lang.text("ภาพรวมกะ", "Overview").to_owned()),
            (
                Tab::Close,
                lang.text("นับเงินและปิดกะ", "Count & close").to_owned(),
            ),
            (
                Tab::History,
                lang.text("ประวัติกะ", "Shift history").to_owned(),
            ),
        ]
    } else {
        vec![
            (Tab::Overview, lang.text("เปิดกะ", "Open shift").to_owned()),
            (
                Tab::History,
                lang.text("ประวัติกะ", "Shift history").to_owned(),
            ),
        ]
    };
    if shift.is_none() && tab == Tab::Close {
        tab = Tab::Overview;
    }
    if crate::chrome::segmented(ui, &mut tab, &options) {
        app.drawer.selected = None;
        app.drawer.movement_offset = 0;
        app.refresh();
    }
    app.drawer.tab = tab;
    if tab == Tab::Close && shift.is_some() {
        let step = if app.drawer.pending.is_some() {
            2
        } else if app.drawer.reviewing {
            1
        } else {
            0
        };
        let labels = [
            lang.text("นับเงิน", "Count"),
            lang.text("ตรวจยอด", "Reconcile"),
            lang.text("ยืนยันปิดกะ", "Confirm"),
        ];
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            for (i, label) in labels.iter().enumerate().rev() {
                let (fg, strong) = if i == step {
                    (t.p.fg_chrome, true)
                } else {
                    (t.p.fg_chrome_muted, false)
                };
                let mark = if i < step {
                    "✓".to_owned()
                } else {
                    (i + 1).to_string()
                };
                ui.label(
                    RichText::new(format!("{mark} {label}"))
                        .font(if strong {
                            t.strong_font(type_scale::LABEL)
                        } else {
                            t.font(type_scale::LABEL)
                        })
                        .color(fg),
                );
                if i > 0 {
                    ui.label(RichText::new("—").color(t.p.border_chrome));
                }
            }
        });
    }
}
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    // Title, tabs and close steps live in the toolbar (design v0.2).
    let shift = app.data.shift.clone();
    if shift.is_none() && app.drawer.tab == Tab::Close {
        app.drawer.tab = Tab::Overview;
    }
    let tab = app.drawer.tab;
    let _ = (&t, lang, now);
    if let (Tab::Close, Some(s)) = (tab, &shift) {
        close_view(app, ui, s);
        return;
    }
    // The other tabs read as statements: centred at a readable width.
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let width = full.width().min(crate::app::READING_WIDTH);
    let rect = egui::Rect::from_min_size(
        egui::pos2(full.center().x - width / 2.0, full.top()),
        vec2(width, full.height()),
    );
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(Align::Min)),
    );
    let ui = &mut ui;
    match (tab, shift) {
        (Tab::History, _) => history(app, ui),
        (_, None) => open_view(app, ui),
        (Tab::Overview, Some(s)) => overview(app, ui, &s),
        (Tab::Close, Some(s)) => close_view(app, ui, &s),
    }
}

fn open_view(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::columns(ui, 2, |cols| {
        t.surface().show(&mut cols[0], |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = space::S3;
            w::title(ui, lang.text("เปิดกะใหม่", "Open a new shift"));
            ui.add(egui::Label::new(t.muted(
                lang.text(
                    "นับเงินทอนที่มีในลิ้นชักแล้วใส่เป็นเงินตั้งต้น ใส่ 0 ได้ถ้าลิ้นชักว่าง ต้องเปิดกะก่อนรับเงินหรือคืนเงินทุกช่องทาง",
                    "Count the change already in the drawer and enter it as the opening float (0 is fine). A shift must be open to take or refund any payment.",
                ),
                type_scale::BODY,
            )).wrap());
            w::field_label(ui, lang.text("เงินตั้งต้น (บาท)", "Opening float (THB)"), None);
            let opening = format::parse_money(&app.drawer.opening).ok();
            let invalid = !app.drawer.opening.trim().is_empty() && opening.is_none();
            ui.add_enabled_ui(app.ready && !app.busy, |ui| {
                w::money_input(
                    ui,
                    &mut app.drawer.opening,
                    w::MoneyOpts {
                        id: Id::new("opening-float"),
                        primary: true,
                        width: Some(ui.available_width().min(280.0)),
                        large: true,
                        invalid,
                        enabled: true,
                    },
                );
                if w::button(lang.text("เปิดกะ", "Open shift"))
                    .primary()
                    .size(w::Size::Lg)
                    .icon(Icon::Shift)
                    .enabled(opening.is_some())
                    .loading(app.busy)
                    .show(ui)
                    .clicked()
                    && let Some(opening) = opening
                {
                    app.send(Command::OpenShift(app.drawer.open_id, opening));
                }
            });
        });
        t.surface().show(&mut cols[1], |ui| {
            ui.set_width(ui.available_width());
            w::title(ui, lang.text("กะล่าสุด", "Latest shift"));
            match app.data.shifts.first().cloned() {
                Some(last) => closed_summary(app, ui, &last),
                None => w::state_view(
                    ui,
                    Icon::Shift,
                    lang.text("ยังไม่มีกะที่ปิด", "No closed shifts yet"),
                    lang.text("เมื่อปิดกะ ยอดนับจะแสดงที่นี่", "Closed counts appear here."),
                ),
            }
        });
    });
}
fn closed_summary(app: &App, ui: &mut egui::Ui, s: &Shift) {
    let lang = app.lang;
    let t = Theme::of(ui);
    let Some(c) = &s.closing else {
        return;
    };
    ui.label(t.muted(
        format!(
            "{} {} · {} → {}",
            lang.text("กะ", "Shift"),
            number(app, s),
            format::date_time(lang, s.opened_at),
            format::clock(c.closed_at)
        ),
        type_scale::LABEL,
    ));
    let names: Vec<String> = [
        s.opened_by
            .as_ref()
            .map(|b| format!("{} {}", lang.text("เปิดโดย", "opened by"), b.name)),
        c.closed_by
            .as_ref()
            .map(|b| format!("{} {}", lang.text("ปิดโดย", "closed by"), b.name)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !names.is_empty() {
        ui.label(t.muted(names.join(" · "), type_scale::LABEL));
    }
    w::kv(
        ui,
        lang.text("ควรมีในลิ้นชัก", "Expected"),
        &format::signed(c.expected),
        w::Value::Plain,
    );
    w::kv(
        ui,
        lang.text("นับได้", "Counted"),
        &format::money(c.counted),
        w::Value::Plain,
    );
    let (label, tone) = difference_label(lang, c.difference);
    ui.horizontal(|ui| {
        ui.label(t.muted(lang.text("ส่วนต่าง", "Difference"), type_scale::BODY));
        w::badge(ui, label, tone);
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format::delta(c.difference))
                    .font(t.strong_font(type_scale::MONEY))
                    .color(if c.difference.satang() < 0 {
                        t.p.danger_fg
                    } else {
                        t.p.fg_default
                    }),
            );
        });
    });
    if !c.reason.is_empty() {
        w::help_text(
            ui,
            &format!("{}: {}", lang.text("เหตุผล", "Reason"), c.reason),
        );
    }
}

/// Shift sales by every method — explicitly not drawer cash.
fn shift_sales(ui: &mut egui::Ui, lang: Language, s: &Shift) {
    let t = Theme::of(ui);
    ui.horizontal(|ui| {
        icons::show(ui, Icon::History, 16.0, t.p.fg_muted);
        ui.label(t.text(
            lang.text(
                "ยอดขายของกะ — ไม่ใช่เงินในลิ้นชัก",
                "Shift sales — not drawer cash",
            ),
            type_scale::BODY_STRONG,
        ));
    });
    let gross = [s.sales, s.transfer_sales, s.card_sales]
        .into_iter()
        .try_fold(Money::ZERO, Money::checked_add)
        .unwrap_or_default();
    let refunds = [s.refunds, s.transfer_refunds, s.card_refunds]
        .into_iter()
        .try_fold(Money::ZERO, Money::checked_add)
        .unwrap_or_default();
    w::kv(
        ui,
        lang.text("ยอดขายก่อนคืน", "Gross sales"),
        &format::money(gross),
        w::Value::Plain,
    );
    w::kv(
        ui,
        &format!(
            "{} · {} {}",
            lang.text("คืนเงิน", "Refunds"),
            s.refund_count,
            lang.noun(s.refund_count, "บิล", "bill", "bills")
        ),
        &format::signed(format::negative(refunds)),
        if refunds == Money::ZERO {
            w::Value::Plain
        } else {
            w::Value::Danger
        },
    );
    w::dashed(ui);
    w::kv(
        ui,
        lang.text("ยอดขายสุทธิ", "Net sales"),
        &format::signed(SignedMoney::from_satang(gross.satang() - refunds.satang())),
        w::Value::Strong,
    );
    w::help_text(
        ui,
        &format!(
            "{} {} · {} {} · {} {}",
            lang.text("เงินสด", "Cash"),
            format::money(s.sales),
            lang.text("โอน", "Transfer"),
            format::money(s.transfer_sales),
            lang.text("บัตร", "Card"),
            format::money(s.card_sales)
        ),
    );
}
/// Drawer cash statement: only cash in and out of the drawer.
fn drawer_statement(ui: &mut egui::Ui, lang: Language, s: &Shift, counted: Option<Money>) {
    let t = Theme::of(ui);
    ui.horizontal(|ui| {
        icons::show(ui, Icon::Shift, 16.0, t.p.fg_muted);
        ui.label(t.text(
            lang.text("เงินสดในลิ้นชัก", "Drawer cash"),
            type_scale::BODY_STRONG,
        ));
    });
    t.sunken().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = space::S1;
        w::kv(
            ui,
            lang.text("เงินตั้งต้น", "Opening float"),
            &format::money(s.opening),
            w::Value::Plain,
        );
        w::kv(
            ui,
            lang.text("รับเงินสดจากการขาย", "Cash sales"),
            &format::delta(format::positive(s.sales)),
            w::Value::Plain,
        );
        w::kv(
            ui,
            lang.text("คืนเงินสด", "Cash refunds"),
            &format::delta(format::negative(s.refunds)),
            w::Value::Plain,
        );
        w::kv(
            ui,
            lang.text("เงินเข้า", "Cash in"),
            &format::delta(format::positive(s.cash_in)),
            w::Value::Plain,
        );
        w::kv(
            ui,
            lang.text("เงินออก", "Cash out"),
            &format::delta(format::negative(s.cash_out)),
            w::Value::Plain,
        );
        w::dashed(ui);
        if let Ok(expected) = s.expected() {
            w::kv(
                ui,
                lang.text("ควรมีในลิ้นชัก", "Expected"),
                &format::signed(expected),
                w::Value::Strong,
            );
        }
        if let Some(counted) = counted {
            w::kv(
                ui,
                lang.text("นับได้", "Counted"),
                &format::money(counted),
                w::Value::Strong,
            );
        }
    });
    w::help_text(
        ui,
        lang.text(
            "เงินสดหลังทอนแล้ว · โอนและบัตรไม่อยู่ในลิ้นชัก · เงินคืนนับในกะที่คืนจริง",
            "Cash after change · transfers and cards are not in the drawer · refunds count in the shift that paid them",
        ),
    );
}

fn overview(app: &mut App, ui: &mut egui::Ui, s: &Shift) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let height = ui.available_height();
    w::columns(ui, 2, |cols| {
        t.surface().show(&mut cols[0], |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height - 34.0);
            ui.spacing_mut().item_spacing.y = space::S2;
            drawer_statement(ui, lang, s, None);
            ui.add_space(space::S3);
            shift_sales(ui, lang, s);
            ui.add_space(space::S3);
            if w::button(lang.text("นับเงินและปิดกะ", "Count & close shift"))
                .icon(Icon::Banknote)
                .show(ui)
                .clicked()
            {
                app.drawer.tab = Tab::Close;
            }
        });
        t.surface().show(&mut cols[1], |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height - 34.0);
            ui.spacing_mut().item_spacing.y = space::S2;
            w::title(
                ui,
                lang.text("เงินเข้า / ออกจากลิ้นชัก", "Cash in / out of drawer"),
            );
            ui.add_enabled_ui(!app.busy, |ui| {
                w::segmented(
                    ui,
                    &mut app.drawer.kind,
                    &[
                        (CashKind::CashIn, lang.text("เงินเข้า", "Cash in")),
                        (CashKind::CashOut, lang.text("เงินออก", "Cash out")),
                    ],
                );
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(150.0);
                        w::field_label(ui, lang.text("จำนวนเงิน", "Amount"), None);
                        w::money_input(
                            ui,
                            &mut app.drawer.amount,
                            w::MoneyOpts {
                                id: Id::new("cash-move-amount"),
                                primary: false,
                                width: Some(150.0),
                                large: false,
                                invalid: false,
                                enabled: true,
                            },
                        );
                    });
                    ui.vertical(|ui| {
                        w::field_label(ui, lang.text("เหตุผล", "Reason"), None);
                        w::text_input(
                            ui,
                            &mut app.drawer.reason,
                            w::FieldOpts {
                                hint: lang
                                    .text("เช่น แลกเหรียญ / ซื้อน้ำแข็ง", "e.g. change float / ice"),
                                limit: 240,
                                ..Default::default()
                            },
                        );
                    });
                });
                let amount = format::parse_money(&app.drawer.amount)
                    .ok()
                    .filter(|m| *m != Money::ZERO);
                let valid = amount.is_some() && !app.drawer.reason.trim().is_empty();
                let label = if app.drawer.kind == CashKind::CashIn {
                    lang.text("บันทึกเงินเข้า", "Record cash in")
                } else {
                    lang.text("บันทึกเงินออก", "Record cash out")
                };
                let mut button = w::button(label)
                    .primary()
                    .money(amount.map_or(String::new(), format::baht))
                    .enabled(valid);
                if !crate::staff_ui::may(app, pos_domain::Permission::CashMovement) {
                    button = button
                        .icon(Icon::Lock)
                        .tooltip(lang.text("ต้องให้ผู้จัดการอนุมัติ", "Needs a manager's approval"));
                }
                if button.show(ui).clicked()
                    && let Some(amount) = amount
                {
                    app.drawer.pending = Some(Pending::Move(CashMovement {
                        id: app.drawer.move_id,
                        shift_id: s.id,
                        created_at: 0,
                        payment_method: PaymentMethod::Cash,
                        kind: app.drawer.kind,
                        amount,
                        reason: app.drawer.reason.trim().into(),
                        staff: Default::default(),
                    }));
                }
            });
            ui.add_space(space::S3);
            w::divider(ui);
            ledger(app, ui, s.id);
        });
    });
}
fn ledger(app: &mut App, ui: &mut egui::Ui, shift_id: Uuid) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.label(t.text(
        lang.text("รายการในกะ", "Shift records"),
        type_scale::BODY_STRONG,
    ));
    let records: Vec<CashMovement> = app
        .data
        .movements
        .iter()
        .filter(|m| m.shift_id == shift_id)
        .cloned()
        .collect();
    egui::ScrollArea::vertical()
        .id_salt(("ledger", shift_id))
        .max_height((ui.available_height() - t.m.control_md - space::S3).max(120.0))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if records.is_empty() {
                w::help_text(
                    ui,
                    lang.text(
                        "ยังไม่มีรายการ เงินตั้งต้นอยู่ในสรุป",
                        "No records yet. The opening float is in the statement.",
                    ),
                );
            }
            for m in records {
                let outflow = matches!(m.kind, CashKind::Refund | CashKind::CashOut);
                let cash = m.payment_method == PaymentMethod::Cash;
                ui.horizontal(|ui| {
                    ui.label(t.muted(format::clock(m.created_at), type_scale::MONEY_SM));
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 120.0).max(80.0));
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            t.text(kind_label(lang, m.kind, m.payment_method), type_scale::BODY),
                        );
                        if !m.reason.is_empty() {
                            ui.add(
                                egui::Label::new(t.muted(&m.reason, type_scale::CAPTION))
                                    .truncate(),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let amount = if outflow {
                            format::negative(m.amount)
                        } else {
                            format::positive(m.amount)
                        };
                        ui.label(
                            RichText::new(format::delta(amount))
                                .font(t.font(type_scale::MONEY))
                                .color(if cash { t.p.fg_default } else { t.p.fg_muted }),
                        )
                        .on_hover_text(if cash {
                            lang.text("เงินสดในลิ้นชัก", "Drawer cash")
                        } else {
                            lang.text("บันทึกการชำระ ไม่นับในลิ้นชัก", "Payment record, not drawer cash")
                        });
                    });
                });
                w::divider(ui);
            }
        });
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!app.busy, |ui| {
            if w::button(lang.text("ใหม่กว่า", "Newer"))
                .size(w::Size::Sm)
                .enabled(app.drawer.movement_offset > 0)
                .show(ui)
                .clicked()
            {
                app.drawer.movement_offset = app.drawer.movement_offset.saturating_sub(50);
                app.refresh();
            }
            if w::button(lang.text("เก่ากว่า", "Older"))
                .size(w::Size::Sm)
                .enabled(app.data.movements.len() == 50)
                .show(ui)
                .clicked()
            {
                app.drawer.movement_offset = app.drawer.movement_offset.saturating_add(50);
                app.refresh();
            }
        });
    });
}

/// Count & close (design v0.2): steps | denomination grid | reconcile.
fn close_view(app: &mut App, ui: &mut egui::Ui, s: &Shift) {
    let t = Theme::of(ui);
    let lang = app.lang;
    load_draft(app, s.id);
    let counted = app.drawer.counted();
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let gap = t.pane_gap();
    let narrow = full.width() < 1180.0;
    let steps_w = if narrow { 0.0 } else { 230.0 };
    let right_w = if narrow { 340.0 } else { 360.0 };
    let steps = egui::Rect::from_min_size(full.min, vec2(steps_w, full.height()));
    let mid = egui::Rect::from_min_max(
        egui::pos2(
            if narrow {
                full.left()
            } else {
                steps.right() + gap
            },
            full.top(),
        ),
        egui::pos2(full.right() - right_w - gap, full.bottom()),
    );
    let right = egui::Rect::from_min_max(egui::pos2(mid.right() + gap, full.top()), full.max);
    if !narrow {
        let (mut head, mut body) = w::pane_uis(ui, steps);
        w::pane_title(
            &mut head,
            &format!("{} {}", lang.text("ปิดกะ", "Close shift"), number(app, s)),
        );
        egui::Frame::new()
            .inner_margin(egui::Margin::same(6))
            .show(&mut body, |ui| close_steps(app, ui, s));
    }
    let (mut head, mut body) = w::pane_uis(ui, mid);
    w::pane_title(&mut head, lang.text("นับเงินในลิ้นชัก", "Count the drawer"));
    if let Some(at) = app.drawer.draft_saved {
        w::pane_count(
            &mut head,
            &format!(
                "{} {}",
                lang.text("บันทึกร่างอัตโนมัติ", "Draft autosaved"),
                format::clock(at)
            ),
        );
    }
    count_table(app, &mut body, counted);
    let (mut head, mut body) = w::pane_uis(ui, right);
    w::pane_title(&mut head, lang.text("ตรวจยอด", "Reconcile"));
    egui::Frame::new()
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(&mut body, |ui| {
            ui.set_min_size(ui.available_size());
            reconcile(app, ui, s, counted);
        });
    if app.drawer.draft_dirty {
        save_draft(app, s.id);
    }
}
/// The close steps: opened, count, reconcile, close + summary.
fn close_steps(app: &App, ui: &mut egui::Ui, s: &Shift) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let step = if app.drawer.pending.is_some() {
        3
    } else if app.drawer.reviewing {
        2
    } else {
        1
    };
    let items = [
        (
            lang.text("เปิดกะ", "Shift opened"),
            format!(
                "{} · {} {}",
                format::clock(s.opened_at),
                lang.text("ตั้งต้น", "float"),
                format::money(s.opening)
            ),
        ),
        (
            lang.text("นับเงินในลิ้นชัก", "Count the drawer"),
            lang.text(
                "นับทีละชนิด ยอดรวมคำนวณให้",
                "Count each note; totals are added up",
            )
            .to_owned(),
        ),
        (
            lang.text("ตรวจส่วนต่าง", "Check the difference"),
            lang.text("ใส่เหตุผลถ้าไม่ตรง", "Give a reason if it is off")
                .to_owned(),
        ),
        (
            lang.text("ปิดกะและพิมพ์สรุป", "Close and print summary"),
            String::new(),
        ),
    ];
    ui.spacing_mut().item_spacing.y = 4.0;
    for (i, (title, detail)) in items.iter().enumerate() {
        let done = i < step;
        let current = i == step;
        let frame = egui::Frame::new()
            .fill(if current {
                t.p.bg_selected
            } else if done {
                t.p.bg_raised
            } else {
                egui::Color32::TRANSPARENT
            })
            .corner_radius(t.radius(crate::ui::tokens::radius::MD))
            .inner_margin(egui::Margin::symmetric(10, 8));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                if done {
                    ui.painter().circle_filled(r.center(), 11.0, t.p.bg_inverse);
                    crate::ui::icons::paint_sized(
                        ui.painter(),
                        Icon::Check,
                        r.center(),
                        14.0,
                        t.p.fg_inverse,
                    );
                } else {
                    ui.painter().circle_stroke(
                        r.center(),
                        10.0,
                        Stroke::new(
                            1.5,
                            if current {
                                t.p.border_selected
                            } else {
                                t.p.border_control
                            },
                        ),
                    );
                    ui.painter().text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        (i + 1).to_string(),
                        t.strong_font(type_scale::CAPTION),
                        if current {
                            t.p.border_selected
                        } else {
                            t.p.fg_muted
                        },
                    );
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(if current || done {
                        t.text(*title, type_scale::BODY_STRONG)
                    } else {
                        t.muted(*title, type_scale::BODY)
                    });
                    if !detail.is_empty() {
                        ui.add(egui::Label::new(t.muted(detail, type_scale::CAPTION)).wrap());
                    }
                });
            });
        });
    }
    let held = app.held_bills().len();
    if held > 0 {
        ui.add_space(space::S3);
        w::banner(
            ui,
            w::Tone::Warning,
            &format!(
                "{} {held} {}",
                lang.text("มีบิลพักไว้", "Held bills:"),
                lang.text("บิล ชำระหรือเก็บไว้ก่อนปิดกะ", "pay or keep them before closing")
            ),
            None,
        );
    }
}
/// Denomination cards (−/+ and typing), then the counted total pinned below.
fn count_table(app: &mut App, ui: &mut egui::Ui, counted: Option<Money>) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let busy = app.busy;
    egui::Panel::bottom("count-total")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(14, 10)))
        .show(ui, |ui| {
            let top = ui.max_rect().top() - 10.0;
            ui.painter().hline(
                ui.max_rect().x_range().expand(14.0),
                top + 0.5,
                Stroke::new(1.0, t.p.border_subtle),
            );
            ui.horizontal(|ui| {
                ui.label(t.text(lang.text("นับได้", "Counted"), type_scale::BODY_STRONG));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(counted.map_or("—".into(), format::baht))
                            .font(t.strong_font(type_scale::MONEY_XL)),
                    );
                });
            });
        });
    egui::CentralPanel::no_frame().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("drawer-count")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(space::S3 as i8))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!busy, |ui| denomination_grid(app, ui));
                    });
            });
    });
}
fn denomination_grid(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let gap = space::S2;
    let width = ui.available_width();
    let min_w = if t.touch() { 210.0 } else { 180.0 };
    let cols = (((width + gap) / (min_w + gap)).floor() as usize).clamp(1, 3);
    let card_w = ((width - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    let card_h = if t.touch() { 134.0 } else { 112.0 };
    let n = DENOMINATIONS.len();
    for row in 0..n.div_ceil(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in (row * cols)..((row + 1) * cols).min(n) {
                ui.allocate_ui_with_layout(
                    vec2(card_w, card_h),
                    egui::Layout::top_down(Align::Min),
                    |ui| denomination_card(app, ui, i, card_w, card_h),
                );
            }
        });
        ui.add_space(gap - ui.spacing().item_spacing.y);
    }
    // Loose satang and anything else, typed as an amount.
    egui::Frame::new()
        .fill(t.p.bg_raised)
        .stroke(Stroke::new(1.0, t.p.border_subtle))
        .corner_radius(t.radius(crate::ui::tokens::radius::LG))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(t.text(
                    lang.text("เศษสตางค์ / อื่น ๆ (บาท)", "Satang / other (THB)"),
                    type_scale::BODY,
                ));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let invalid = !app.drawer.other.trim().is_empty()
                        && format::parse_money(&app.drawer.other).is_err();
                    let r = w::money_input(
                        ui,
                        &mut app.drawer.other,
                        w::MoneyOpts {
                            id: Id::new("count-other"),
                            primary: false,
                            width: Some(130.0),
                            large: false,
                            invalid,
                            enabled: true,
                        },
                    );
                    if r.changed() {
                        app.drawer.draft_dirty = true;
                        app.drawer.reviewing = false;
                    }
                });
            });
        });
}
fn denomination_card(app: &mut App, ui: &mut egui::Ui, i: usize, width: f32, height: f32) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let (value, note) = DENOMINATIONS[i];
    let face = Money::from_satang(value).unwrap_or_default();
    let n = app.drawer.counts[i]
        .trim()
        .replace(',', "")
        .parse::<u32>()
        .unwrap_or(0);
    egui::Frame::new()
        .fill(t.p.bg_raised)
        .stroke(Stroke::new(
            1.0,
            if n > 0 {
                t.p.border_default
            } else {
                t.p.border_subtle
            },
        ))
        .corner_radius(t.radius(crate::ui::tokens::radius::LG))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(width - 20.0);
            ui.set_min_height(height - 20.0);
            ui.spacing_mut().item_spacing.y = space::S2;
            ui.horizontal(|ui| {
                let label = if value >= 100 {
                    format::count(u64::try_from(value / 100).unwrap_or_default())
                } else {
                    format::money(face)
                };
                ui.label(RichText::new(label).font(t.strong_font(type_scale::MONEY_LG)));
                ui.label(t.muted(
                    if note {
                        lang.text("ธนบัตร", "note")
                    } else {
                        lang.text("เหรียญ", "coin")
                    },
                    type_scale::CAPTION,
                ));
            });
            let step_h = if t.touch() {
                44.0
            } else {
                t.m.control_md + 4.0
            };
            let side = step_h;
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if w::icon_button_sized(ui, Icon::Minus, lang.text("ลด", "Fewer"), Some(side), true)
                    .clicked()
                    && n > 0
                {
                    app.drawer.counts[i] = (n - 1).to_string();
                    changed = true;
                }
                let field_w = (ui.available_width() - side).max(40.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.drawer.counts[i])
                        .id(Id::new(("denomination", i)))
                        .font(t.strong_font(type_scale::MONEY))
                        .horizontal_align(Align::Center)
                        .vertical_align(Align::Center)
                        .hint_text("0")
                        .min_size(vec2(field_w, step_h))
                        .desired_width(field_w)
                        .char_limit(6),
                );
                if resp.changed() {
                    app.drawer.counts[i].retain(|c| c.is_ascii_digit());
                    changed = true;
                }
                if w::icon_button_sized(ui, Icon::Plus, lang.text("เพิ่ม", "More"), Some(side), true)
                    .clicked()
                {
                    app.drawer.counts[i] = n.saturating_add(1).min(999_999).to_string();
                    changed = true;
                }
            });
            if changed {
                app.drawer.draft_dirty = true;
                app.drawer.reviewing = false;
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(face.checked_mul(n).map_or("—".into(), format::money))
                        .font(t.font(type_scale::MONEY))
                        .color(if n > 0 { t.p.fg_default } else { t.p.fg_subtle }),
                );
            });
        });
}
fn reconcile(app: &mut App, ui: &mut egui::Ui, s: &Shift, counted: Option<Money>) {
    let lang = app.lang;
    // Difference, reason and the commit stay pinned (01 Foundations: never hide
    // money); only the sales and drawer breakdown scroll.
    egui::Panel::bottom("reconcile-footer")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: space::S2 as i8,
            bottom: 0,
        }))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::S2;
            reconcile_footer(app, ui, s, counted);
        });
    egui::CentralPanel::no_frame().show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("reconcile")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = space::S2;
                shift_sales(ui, lang, s);
                ui.add_space(space::S2);
                drawer_statement(ui, lang, s, counted.filter(|_| app.drawer.touched()));
            });
    });
}
fn reconcile_footer(app: &mut App, ui: &mut egui::Ui, s: &Shift, counted: Option<Money>) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::divider(ui);
    let difference = counted
        .filter(|_| app.drawer.touched())
        .and_then(|c| s.difference(c).ok());
    if let Some(diff) = difference {
        let (label, tone) = difference_label(lang, diff);
        let (bg, fg) = tone.colors(&t);
        egui::Frame::new()
            .fill(bg)
            .corner_radius(t.radius(8))
            .inner_margin(egui::Margin::same(space::S3 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(t.text(lang.text("ส่วนต่าง", "Difference"), type_scale::BODY_STRONG));
                    ui.label(
                        RichText::new(label)
                            .font(t.strong_font(type_scale::LABEL))
                            .color(fg),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format::delta(diff))
                                .font(t.strong_font(type_scale::MONEY_LG))
                                .color(if diff.satang() < 0 { t.p.danger_fg } else { fg }),
                        );
                    });
                });
            });
        let off = diff.satang() != 0;
        w::field_label(
            ui,
            lang.text("เหตุผลของส่วนต่าง", "Reason for difference"),
            Some(if off {
                lang.text("(จำเป็น)", "(required)")
            } else {
                lang.text("(ไม่บังคับ เมื่อยอดตรง)", "(optional when balanced)")
            }),
        );
        let invalid = off && app.drawer.reviewing && app.drawer.close_reason.trim().is_empty();
        let r = w::text_input(
            ui,
            &mut app.drawer.close_reason,
            w::FieldOpts {
                hint: lang.text("เช่น ทอนเกินบิล #000177", "e.g. over-changed on #000177"),
                limit: 240,
                invalid,
                enabled: !app.busy,
                ..Default::default()
            },
        );
        if r.changed() {
            app.drawer.draft_dirty = true;
        }
    } else {
        w::banner(
            ui,
            w::Tone::Info,
            lang.text(
                "นับเงินทางซ้ายเพื่อดูส่วนต่าง",
                "Count the drawer on the left to see the difference.",
            ),
            None,
        );
    }
    if !app.cart.lines.is_empty() {
        w::banner(
            ui,
            w::Tone::Warning,
            lang.text(
                "มีบิลที่ยังไม่ชำระในหน้าขาย พักหรือล้างบิลก่อนปิดกะ",
                "An unpaid bill is open in Sell. Hold or clear it before closing.",
            ),
            None,
        );
    }
    let count = app
        .drawer
        .denominations()
        .zip(counted)
        .map(|(d, c)| ShiftCount {
            counted: c,
            reason: app.drawer.close_reason.trim().to_owned(),
            denominations: d,
        });
    let diff = count.as_ref().and_then(|c| s.difference(c.counted).ok());
    let needs_reason =
        diff.is_some_and(|d| d.satang() != 0) && app.drawer.close_reason.trim().is_empty();
    if app.drawer.reviewing && needs_reason {
        w::help_text(
            ui,
            lang.text(
                "ใส่เหตุผลของส่วนต่างก่อนปิดกะ",
                "Enter a reason for the difference to close the shift.",
            ),
        );
    }
    ui.horizontal(|ui| {
        if app.drawer.reviewing
            && w::button(lang.text("กลับไปนับ", "Back to count"))
                .icon(Icon::ChevronLeft)
                .ghost()
                .show(ui)
                .clicked()
        {
            app.drawer.reviewing = false;
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            let ready =
                count.is_some() && app.drawer.touched() && app.cart.lines.is_empty() && !app.busy;
            if !app.drawer.reviewing {
                if w::button(lang.text("ตรวจยอด", "Reconcile"))
                    .primary()
                    .size(w::Size::Lg)
                    .icon(Icon::ChevronRight)
                    .enabled(ready)
                    .show(ui)
                    .clicked()
                {
                    app.drawer.reviewing = true;
                }
            } else if w::button(lang.text("ปิดกะ", "Close shift"))
                .primary()
                .size(w::Size::Lg)
                .enabled(ready && !needs_reason)
                .show(ui)
                .clicked()
                && let Some(count) = count
            {
                app.drawer.pending = Some(Pending::Close(Box::new(s.clone()), count));
            }
        });
    });
}

fn history(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let height = ui.available_height();
    let shifts = app.data.shifts.clone();
    let selected = app
        .drawer
        .selected
        .clone()
        .or_else(|| shifts.first().cloned());
    w::columns(ui, 2, |cols| {
        t.surface().show(&mut cols[0], |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height - 34.0);
            ui.label(t.text(lang.text("กะที่ปิดแล้ว", "Closed shifts"), type_scale::TITLE));
            egui::ScrollArea::vertical()
                .id_salt("shift-history")
                .max_height(height - 120.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if shifts.is_empty() {
                        w::state_view(
                            ui,
                            Icon::Shift,
                            lang.text("ยังไม่มีกะที่ปิด", "No closed shifts yet"),
                            "",
                        );
                    }
                    for s in &shifts {
                        let Some(c) = &s.closing else { continue };
                        let active = selected.as_ref().is_some_and(|x| x.id == s.id);
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(ui.available_width(), t.m.row_height + 14.0),
                            Sense::click(),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Button,
                                true,
                                active,
                                number(app, s),
                            )
                        });
                        ui.painter().rect_filled(
                            rect,
                            t.radius(6),
                            if active {
                                t.p.bg_selected
                            } else if response.hovered() {
                                t.p.bg_hover
                            } else {
                                egui::Color32::TRANSPARENT
                            },
                        );
                        let left = rect.left() + space::S3;
                        ui.painter().text(
                            egui::pos2(left, rect.center().y - 9.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{} {}", lang.text("กะ", "Shift"), number(app, s)),
                            t.strong_font(type_scale::BODY),
                            t.p.fg_default,
                        );
                        ui.painter().text(
                            egui::pos2(left, rect.center().y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            format!(
                                "{} → {}",
                                format::date_time(lang, s.opened_at),
                                format::clock(c.closed_at)
                            ),
                            t.font(type_scale::CAPTION),
                            t.p.fg_muted,
                        );
                        let (label, tone) = difference_label(lang, c.difference);
                        let (_, fg) = tone.colors(&t);
                        ui.painter().text(
                            egui::pos2(rect.right() - space::S3, rect.center().y - 9.0),
                            egui::Align2::RIGHT_CENTER,
                            format::delta(c.difference),
                            t.strong_font(type_scale::MONEY),
                            if c.difference.satang() < 0 {
                                t.p.danger_fg
                            } else {
                                t.p.fg_default
                            },
                        );
                        ui.painter().text(
                            egui::pos2(rect.right() - space::S3, rect.center().y + 10.0),
                            egui::Align2::RIGHT_CENTER,
                            label,
                            t.strong_font(type_scale::CAPTION),
                            fg,
                        );
                        if response.clicked() {
                            app.drawer.selected = Some(s.clone());
                            app.drawer.movement_offset = 0;
                            app.refresh();
                        }
                        ui.painter().line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            Stroke::new(1.0, t.p.border_subtle),
                        );
                    }
                });
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!app.busy, |ui| {
                    if w::button(lang.text("ใหม่กว่า", "Newer"))
                        .size(w::Size::Sm)
                        .enabled(app.drawer.history_offset > 0)
                        .show(ui)
                        .clicked()
                    {
                        app.drawer.history_offset = app.drawer.history_offset.saturating_sub(50);
                        app.refresh();
                    }
                    if w::button(lang.text("เก่ากว่า", "Older"))
                        .size(w::Size::Sm)
                        .enabled(shifts.len() == 50)
                        .show(ui)
                        .clicked()
                    {
                        app.drawer.history_offset = app.drawer.history_offset.saturating_add(50);
                        app.refresh();
                    }
                });
            });
        });
        t.surface().show(&mut cols[1], |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height - 34.0);
            let Some(s) = selected else {
                return;
            };
            ui.spacing_mut().item_spacing.y = space::S2;
            ui.horizontal(|ui| {
                w::title(
                    ui,
                    &format!("{} {}", lang.text("กะ", "Shift"), number(app, &s)),
                );
                w::badge(
                    ui,
                    lang.text("ปิดแล้ว · แก้ไขไม่ได้", "Closed · read-only"),
                    w::Tone::Neutral,
                );
                if app.print.enabled() {
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if w::button(lang.text("พิมพ์สรุปกะ", "Print summary"))
                            .icon(Icon::Printer)
                            .size(w::Size::Sm)
                            .loading(app.print.busy())
                            .show(ui)
                            .clicked()
                        {
                            app.print_shift(&s);
                        }
                    });
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("closed-detail")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    closed_summary(app, ui, &s);
                    if let Some(c) = &s.closing
                        && !c.denominations.is_empty()
                    {
                        let parts: Vec<String> = c
                            .denominations
                            .iter()
                            .map(|d| format!("{} × {}", format::money(d.value), d.count))
                            .collect();
                        w::help_text(
                            ui,
                            &format!("{}: {}", lang.text("นับแยก", "Breakdown"), parts.join(" · ")),
                        );
                    }
                    ui.add_space(space::S2);
                    drawer_statement(ui, lang, &s, s.closing.as_ref().map(|c| c.counted));
                    ui.add_space(space::S2);
                    shift_sales(ui, lang, &s);
                    ui.add_space(space::S2);
                    ledger(app, ui, s.id);
                });
        });
    });
}

pub fn confirm(app: &mut App, ctx: &egui::Context) {
    let Some(pending) = app.drawer.pending.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let response = w::dialog(ctx, "shift-confirm", 480.0, |ui| {
        match &pending {
            Pending::Move(m) => {
                let title = if m.kind == CashKind::CashIn {
                    lang.text("บันทึกเงินเข้าลิ้นชัก?", "Record cash into the drawer?")
                } else {
                    lang.text("บันทึกเงินออกจากลิ้นชัก?", "Record cash out of the drawer?")
                };
                w::dialog_title(ui, title, false);
                ui.label(
                    RichText::new(format::baht(m.amount)).font(t.strong_font(type_scale::MONEY_XL)),
                );
                ui.label(t.text(&m.reason, type_scale::BODY));
                w::help_text(
                    ui,
                    lang.text(
                        "ยืนยันหลังใส่หรือนำเงินออกจริง รายการนี้แก้ไขย้อนหลังไม่ได้",
                        "Confirm after physically moving the cash. This record cannot be edited later.",
                    ),
                );
            }
            Pending::Close(s, count) => {
                w::dialog_title(
                    ui,
                    &format!("{} {}?", lang.text("ปิดกะ", "Close shift"), number(app, s)),
                    false,
                );
                if let Ok(expected) = s.expected() {
                    w::kv(
                        ui,
                        lang.text("ควรมีในลิ้นชัก", "Expected"),
                        &format::signed(expected),
                        w::Value::Plain,
                    );
                }
                w::kv(
                    ui,
                    lang.text("นับได้", "Counted"),
                    &format::money(count.counted),
                    w::Value::Strong,
                );
                if let Ok(diff) = s.difference(count.counted) {
                    let (label, _) = difference_label(lang, diff);
                    w::kv(
                        ui,
                        &format!("{} · {label}", lang.text("ส่วนต่าง", "Difference")),
                        &format::delta(diff),
                        if diff.satang() < 0 {
                            w::Value::Danger
                        } else {
                            w::Value::Strong
                        },
                    );
                }
                if !count.reason.is_empty() {
                    w::help_text(
                        ui,
                        &format!("{}: {}", lang.text("เหตุผล", "Reason"), count.reason),
                    );
                }
                w::help_text(
                    ui,
                    lang.text(
                        "ปิดแล้วแก้ยอดนับไม่ได้ ต้องเปิดกะใหม่เพื่อรับเงินต่อ",
                        "Closing is permanent. Open a new shift before taking more payment.",
                    ),
                );
            }
        }
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            let label = match &pending {
                Pending::Move(m) if m.kind == CashKind::CashIn => {
                    lang.text("บันทึกเงินเข้า", "Record cash in")
                }
                Pending::Move(_) => lang.text("บันทึกเงินออก", "Record cash out"),
                Pending::Close(..) if app.print.enabled() => {
                    lang.text("ปิดกะและพิมพ์สรุป", "Close & print summary")
                }
                Pending::Close(..) => lang.text("ปิดกะ", "Close shift"),
            };
            if w::button(label)
                .primary()
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("กลับไปแก้", "Go back"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out
    });
    match response.inner {
        Some(false) => return,
        None if !app.busy && response.should_close() => return,
        Some(true) => match &pending {
            Pending::Move(m) => app.send(Command::MoveCash(m.clone())),
            Pending::Close(s, count) => {
                app.send(Command::CloseShift(s.id, s.revision, count.clone()))
            }
        },
        None => {}
    }
    app.drawer.pending = Some(pending);
}

#[cfg(debug_assertions)]
pub fn preview_confirm(app: &mut App) {
    if let Some(s) = app.data.shift.clone()
        && let (Some(d), Some(c)) = (app.drawer.denominations(), app.drawer.counted())
    {
        app.drawer.pending = Some(Pending::Close(
            Box::new(s),
            ShiftCount {
                counted: c,
                reason: app.drawer.close_reason.clone(),
                denominations: d,
            },
        ));
    }
}
pub fn goto_open(app: &mut App) {
    app.drawer.selected = None;
    app.drawer.tab = Tab::Overview;
    app.navigate(Page::Shift);
}
