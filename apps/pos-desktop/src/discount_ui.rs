//! Discount dialog for one bill line (F7) or the whole bill (F8).
//! Prices come from `App::bill_price`, the same rules checkout applies.
use crate::{
    app::App,
    ui::{Theme, format, space, type_scale, w},
};
use eframe::egui::{self, Id, Key};
use pos_domain::{Discount, DomainError, Money};

const REASONS: [(&str, &str); 4] = [
    ("สมาชิก", "Member"),
    ("โปรโมชัน", "Promotion"),
    ("พนักงาน", "Staff"),
    ("สินค้ามีตำหนิ", "Damaged item"),
];
const PERCENTS: [u32; 5] = [5, 10, 15, 20, 50];

pub struct DiscountEditor {
    /// The line being discounted, or None for the whole bill.
    pub line: Option<usize>,
    pub percent: bool,
    pub value: String,
    pub reason: String,
    existing: bool,
}
impl DiscountEditor {
    pub fn new(app: &App, line: Option<usize>) -> Self {
        let rule = match line {
            Some(i) => app.cart.lines.get(i).and_then(|l| l.discount),
            None => app.cart.discount,
        };
        let (percent, value) = match rule {
            Some(Discount::Percent(bp)) => (
                true,
                format::discount_rule(Discount::Percent(bp))
                    .trim_end_matches('%')
                    .to_owned(),
            ),
            Some(Discount::Amount(m)) => (false, format::money(m)),
            None => (true, String::new()),
        };
        Self {
            line,
            percent,
            value,
            reason: app.cart.discount_reason.clone(),
            existing: rule.is_some(),
        }
    }
    fn rule(&self) -> Result<Discount, DomainError> {
        let n = format::parse_money(&self.value)?;
        // Percent input reuses money parsing: "12.5" → 1250 hundredths = basis points.
        let rule = if self.percent {
            Discount::Percent(u32::try_from(n.satang()).map_err(|_| DomainError::InvalidDiscount)?)
        } else {
            Discount::Amount(n)
        };
        rule.validate()?;
        Ok(rule)
    }
}
/// The price the discount applies to: the line's gross amount, or the bill
/// after line discounts.
fn base(app: &App, line: Option<usize>) -> Option<Money> {
    match line {
        Some(i) => app.cart.lines.get(i).and_then(|l| app.price(l)),
        None => {
            let mut cart = app.cart.clone();
            cart.discount = None;
            app.price_cart(&cart).ok().map(|p| p.total)
        }
    }
}

pub fn open(app: &mut App, line: Option<usize>) {
    if app.busy || app.cart.lines.is_empty() {
        return;
    }
    if line.is_some_and(|i| i >= app.cart.lines.len()) {
        return;
    }
    app.sell.discount = Some(DiscountEditor::new(app, line));
}

pub fn dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut editor) = app.sell.discount.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let Some(base) = base(app, editor.line) else {
        return;
    };
    let title = match editor.line.and_then(|i| app.cart.lines.get(i)) {
        Some(line) => {
            let name = app
                .known
                .get(&line.product_id)
                .map_or_else(String::new, |p| p.name.clone());
            format!("{} {name}", lang.text("ส่วนลดรายการ", "Discount on"))
        }
        None => lang.text("ส่วนลดทั้งบิล", "Bill discount").to_owned(),
    };
    let rule = editor.rule();
    let off = rule.and_then(|r| r.off(base));
    let typed = !editor.value.trim().is_empty();
    let response = w::dialog(ctx, "discount", 460.0, |ui| {
        w::dialog_title(ui, &title, false);
        ui.label(t.muted(
            format!(
                "{} {}",
                if editor.line.is_some() {
                    lang.text("ราคารายการ", "Line price")
                } else {
                    lang.text("ยอดก่อนลดทั้งบิล", "Bill before this discount")
                },
                format::baht(base)
            ),
            type_scale::BODY,
        ));
        ui.add_space(space::S2);
        ui.horizontal(|ui| {
            let before = editor.percent;
            w::segmented(
                ui,
                &mut editor.percent,
                &[
                    (true, lang.text("เปอร์เซ็นต์", "Percent")),
                    (false, lang.text("บาท", "Baht")),
                ],
            );
            if before != editor.percent {
                editor.value.clear();
            }
            ui.add_space(space::S2);
            let r = w::money_input(
                ui,
                &mut editor.value,
                w::MoneyOpts {
                    id: Id::new("discount-value"),
                    primary: false,
                    width: Some(ui.available_width().min(160.0)),
                    large: true,
                    invalid: typed && off.is_err(),
                    enabled: true,
                },
            );
            if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
                r.request_focus();
            }
            ui.label(t.text(
                if editor.percent { "%" } else { "฿" },
                type_scale::BODY_STRONG,
            ));
        });
        if editor.percent {
            let labels: Vec<(String, bool)> = PERCENTS
                .iter()
                .map(|p| (format!("{p}%"), rule == Ok(Discount::Percent(p * 100))))
                .collect();
            if let Some(i) = w::chip_row(ui, &labels) {
                editor.value = PERCENTS[i].to_string();
            }
        }
        ui.add_space(space::S1);
        t.sunken().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = space::S1;
            match &off {
                Ok(off) => {
                    w::kv(
                        ui,
                        lang.text("ส่วนลด", "Discount"),
                        &format::minus(*off),
                        w::Value::Danger,
                    );
                    let left = Money::from_satang(base.satang() - off.satang()).unwrap_or_default();
                    w::kv_styled(
                        ui,
                        lang.text("เหลือ", "Leaves"),
                        &format::money(left),
                        w::Value::Strong,
                        type_scale::MONEY_LG,
                    );
                }
                Err(DomainError::DiscountTooLarge) => w::inline_error(
                    ui,
                    lang.text("ส่วนลดมากกว่าราคา", "The discount is more than the price"),
                ),
                Err(_) if typed => w::inline_error(
                    ui,
                    if editor.percent {
                        lang.text("ใส่ 0.01–100%", "Enter 0.01–100%")
                    } else {
                        lang.text("ใส่จำนวนเงินมากกว่า 0", "Enter an amount above 0")
                    },
                ),
                Err(_) => w::help_text(
                    ui,
                    lang.text(
                        "ใส่ตัวเลขหรือแตะเปอร์เซ็นต์ด้านบน",
                        "Type a number or tap a percentage above",
                    ),
                ),
            }
        });
        if !crate::staff_ui::may(app, pos_domain::Permission::Discount) {
            ui.horizontal(|ui| {
                crate::ui::icons::show(ui, crate::ui::Icon::Lock, 16.0, t.p.fg_muted);
                w::help_text(
                    ui,
                    &format!(
                        "{} {} {}",
                        lang.text("ส่วนลดรวมเกิน", "Discounts above"),
                        format::discount_rule(Discount::Percent(
                            crate::staff_ui::discount_limit(app).max(1)
                        )),
                        lang.text(
                            "ของบิล ต้องให้ผู้จัดการอนุมัติตอนรับเงิน",
                            "of the bill need a manager's approval at payment",
                        )
                    ),
                );
            });
        }
        w::field_label(
            ui,
            lang.text("เหตุผล", "Reason"),
            Some(lang.text("ไม่บังคับ · ใช้กับทั้งบิล", "optional · shared by the bill")),
        );
        ui.horizontal_wrapped(|ui| {
            for (th, en) in REASONS {
                let label = lang.text(th, en);
                if w::chip(ui, label, editor.reason == label, false).clicked() {
                    editor.reason = if editor.reason == label {
                        String::new()
                    } else {
                        label.to_owned()
                    };
                }
            }
        });
        w::text_input(
            ui,
            &mut editor.reason,
            w::FieldOpts {
                hint: lang.text("หรือพิมพ์เหตุผลเอง", "Or type a reason"),
                limit: 240,
                ..Default::default()
            },
        );
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ใช้ส่วนลด", "Apply discount"))
                .primary()
                .shortcut("Enter")
                .enabled(off.is_ok())
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
            if editor.existing
                && w::button(lang.text("ลบส่วนลด", "Remove discount"))
                    .danger_quiet()
                    .show(ui)
                    .clicked()
            {
                out = Some(false);
                set(app, editor.line, None);
            }
        });
        if off.is_ok() && ui.input(|i| i.key_pressed(Key::Enter)) {
            out = Some(true);
        }
        out
    });
    match response.inner {
        Some(true) => {
            if let Ok(rule) = rule {
                set(app, editor.line, Some(rule));
                app.cart.discount_reason = editor.reason.trim().to_owned();
            }
        }
        Some(false) => {}
        None if response.should_close() => {}
        None => app.sell.discount = Some(editor),
    }
}
fn set(app: &mut App, line: Option<usize>, rule: Option<Discount>) {
    match line {
        Some(i) => {
            if let Some(l) = app.cart.lines.get_mut(i) {
                l.discount = rule;
            }
        }
        None => app.cart.discount = rule,
    }
    if !app.cart.has_discount() {
        app.cart.discount_reason.clear();
    }
    // A bill discount may no longer fit after a line discount changes.
    if app.bill_price().is_err() && line.is_some() {
        app.cart.discount = None;
        app.toast(app.lang.text(
            "ลบส่วนลดทั้งบิลแล้ว เพราะมากกว่ายอดที่เหลือ",
            "Bill discount removed: it was more than what is left.",
        ));
    }
    app.payment_review = None;
}
/// Shown under a discounted line and in receipts: "ส่วนลด 10%".
pub fn label(lang: crate::i18n::Language, rule: Discount) -> String {
    format!(
        "{} {}",
        lang.text("ส่วนลด", "Discount"),
        format::discount_rule(rule)
    )
}
