//! Sales history: receipts table with an inspector, and the full-void dialog.
//! Receipts are immutable; a void is a separate audit record (AGENTS.md).
use crate::{
    app::App,
    i18n::Language,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, Align2, Color32, Rect, RichText, Sense, Stroke, pos2, vec2};
use pos_domain::{PaymentMethod, Sale, SaleRecord, VoidRequest};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

#[derive(Default)]
pub struct HistoryState {
    pub offset: u32,
    /// Quick filter chips above the list.
    pub kind: Kind,
    pub selected: Option<SaleRecord>,
    pub filter: String,
    pub refund_editor: Option<RefundEditor>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    All,
    Refunded,
    Voided,
    NonCash,
}
impl Kind {
    fn matches(self, r: &SaleRecord) -> bool {
        match self {
            Kind::All => true,
            Kind::Refunded => r.cancellation.is_none() && !r.refunds.is_empty(),
            Kind::Voided => r.cancellation.is_some(),
            Kind::NonCash => r.sale.is_split() || r.sale.payment_method != PaymentMethod::Cash,
        }
    }
}
fn status(lang: Language, record: &SaleRecord) -> (&'static str, w::Tone) {
    if record.fully_refunded() {
        (lang.text("คืนเงินแล้ว", "Refunded"), w::Tone::Danger)
    } else if !record.refunds.is_empty() {
        (lang.text("คืนบางส่วน", "Part refunded"), w::Tone::Warning)
    } else {
        (lang.text("ชำระแล้ว", "Paid"), w::Tone::Success)
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let wide = full.width() >= 1180.0;
    let detail_w = if wide { 400.0 } else { 0.0 };
    let main = Rect::from_min_max(
        full.min,
        pos2(
            full.right() - if wide { detail_w + t.pane_gap() } else { 0.0 },
            full.bottom(),
        ),
    );
    let lang = app.lang;
    let shown = app.data.sales.len() as u64;
    let (mut head, mut body) = w::pane_uis(ui, main);
    w::pane_title(
        &mut head,
        lang.text("ประวัติขาย · ล่าสุดก่อน", "Sales history · newest first"),
    );
    w::pane_count(
        &mut head,
        &format!(
            "{} {}",
            format::count(shown),
            lang.noun(shown, "บิล", "bill", "bills")
        ),
    );
    egui::Frame::new()
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(&mut body, |ui| {
            ui.set_min_size(ui.available_size());
            list(app, ui, !wide);
        });
    if wide {
        let rect = Rect::from_min_max(pos2(full.right() - detail_w, full.top()), full.max);
        let selected = app.history.selected.clone();
        let (mut head, mut body) = w::pane_uis(ui, rect);
        match &selected {
            Some(record) => {
                w::pane_title(
                    &mut head,
                    &format!(
                        "{} {}",
                        lang.text("บิล", "Receipt"),
                        format::receipt_number(record.sale.number)
                    ),
                );
                let (label, tone) = status(lang, record);
                head.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(space::S2);
                    w::badge(ui, label, tone);
                });
            }
            None => w::pane_title(&mut head, lang.text("รายละเอียดบิล", "Receipt")),
        }
        egui::Frame::new()
            .inner_margin(egui::Margin::same(space::S3 as i8))
            .show(&mut body, |ui| {
                ui.set_min_size(ui.available_size());
                match selected {
                    Some(record) => detail(app, ui, &record),
                    None => w::state_view(
                        ui,
                        Icon::History,
                        lang.text("เลือกบิลเพื่อดูรายละเอียด", "Select a receipt"),
                        lang.text(
                            "ดูรายการ ยอด และยกเลิกบิลได้ที่นี่",
                            "See lines and totals, or void it here.",
                        ),
                    ),
                }
            });
    }
    let _ = t;
}

fn list(app: &mut App, ui: &mut egui::Ui, dialog_detail: bool) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.spacing_mut().item_spacing.y = space::S3;
    w::search_input(
        ui,
        &mut app.history.filter,
        lang.text(
            "กรองเลขบิล ชื่อบิล หรือวิธีชำระในหน้านี้",
            "Filter this page by receipt, bill name or method",
        ),
        None,
        egui::Id::new("history-filter"),
    );
    let q = app.history.filter.trim().to_lowercase();
    // Chips count this page (the list is paged by 50).
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = space::S2;
        for (kind, label) in [
            (Kind::All, lang.text("ทั้งหมด", "All")),
            (Kind::Refunded, lang.text("คืนเงิน", "Refunded")),
            (Kind::Voided, lang.text("ยกเลิก", "Voided")),
            (Kind::NonCash, lang.text("ไม่ใช่เงินสด", "Non-cash")),
        ] {
            let n = app.data.sales.iter().filter(|r| kind.matches(r)).count();
            if w::chip_dot(ui, &format!("{label} {n}"), app.history.kind == kind, None).clicked() {
                app.history.kind = kind;
            }
        }
    });
    let kind = app.history.kind;
    let records: Vec<SaleRecord> = app
        .data
        .sales
        .iter()
        .filter(|r| kind.matches(r))
        .filter(|r| {
            q.is_empty()
                || format::receipt_number(r.sale.number).contains(&q)
                || r.sale.number.to_string().contains(&q)
                || r.sale.label.to_lowercase().contains(&q)
                || lang.sale_method(&r.sale).to_lowercase().contains(&q)
        })
        .cloned()
        .collect();
    let width = ui.available_width();
    egui::ScrollArea::vertical()
        .id_salt("history")
        .auto_shrink([false, false])
        .max_height(ui.available_height() - t.m.control_md - space::S3)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if records.is_empty() {
                if app.data.sales.is_empty() {
                    w::state_view(
                        ui,
                        Icon::History,
                        lang.text("ยังไม่มีบิลที่ชำระแล้ว", "No completed sales yet"),
                        lang.text(
                            "เมื่อรับเงินแล้ว บิลจะแสดงที่นี่",
                            "Completed sales appear here after payment.",
                        ),
                    );
                } else {
                    w::state_view(
                        ui,
                        Icon::Search,
                        lang.text("ไม่มีบิลตามตัวกรอง", "No receipts match"),
                        lang.text(
                            "ตัวกรองค้นเฉพาะหน้านี้ ลองหน้าเก่ากว่าหรือเลือก \"ทั้งหมด\"",
                            "Filters search this page only; try older pages or All.",
                        ),
                    );
                }
            }
            // Bills grouped by the hour they were paid (Bangkok time).
            let hour_of = |ts: i64| ts.div_euclid(3600);
            let mut i = 0;
            while i < records.len() {
                let key = hour_of(records[i].sale.created_at);
                let group: Vec<&SaleRecord> = records[i..]
                    .iter()
                    .take_while(|r| hour_of(r.sale.created_at) == key)
                    .collect();
                i += group.len();
                let total: i64 = group
                    .iter()
                    .filter(|r| r.cancellation.is_none())
                    .map(|r| r.sale.total.satang())
                    .sum();
                group_header(
                    ui,
                    &t,
                    lang,
                    group[0].sale.created_at,
                    group.len(),
                    total,
                    width,
                );
                for record in group {
                    if history_row(app, ui, &t, record, width) {
                        if dialog_detail {
                            app.receipt = Some(record.clone());
                        }
                        app.history.selected = Some(record.clone());
                    }
                }
            }
        });
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!app.busy, |ui| {
            if w::button(lang.text("ใหม่กว่า", "Newer"))
                .icon(Icon::ChevronLeft)
                .size(w::Size::Sm)
                .enabled(app.history.offset > 0)
                .show(ui)
                .clicked()
            {
                app.history.offset = app.history.offset.saturating_sub(50);
                app.refresh();
            }
            ui.label(t.muted(
                format!(
                    "{} {}",
                    lang.text("หน้า", "Page"),
                    app.history.offset / 50 + 1
                ),
                type_scale::LABEL,
            ));
            if w::button(lang.text("เก่ากว่า", "Older"))
                .icon(Icon::ChevronRight)
                .size(w::Size::Sm)
                .enabled(app.data.sales.len() == 50)
                .show(ui)
                .clicked()
            {
                app.history.offset = app.history.offset.saturating_add(50);
                app.refresh();
            }
        });
    });
}

/// Receipt body shared by the inspector and the receipt dialog.
/// "15 ต.ค. 2569 · 19:00–20:00 ........ 2 บิล · 1,123.00"
fn group_header(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: Language,
    at: i64,
    n: usize,
    total: i64,
    width: f32,
) {
    let (rect, _) = ui.allocate_exact_size(
        vec2(width, if t.touch() { 34.0 } else { 26.0 }),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0, t.p.bg_sunken);
    let label = format::local(at).map_or_else(String::new, |d| {
        format!(
            "{} · {:02}:00–{:02}:00",
            format::date(lang, d.date()),
            d.hour(),
            (d.hour() + 1) % 24
        )
    });
    ui.painter().text(
        pos2(rect.left() + space::S3, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        t.strong_font(type_scale::CAPTION),
        t.p.fg_muted,
    );
    let n = n as u64;
    let right = format!(
        "{} {} · {}",
        n,
        lang.noun(n, "บิล", "bill", "bills"),
        pos_domain::Money::from_satang(total).map_or("—".into(), format::money)
    );
    ui.painter().text(
        pos2(rect.right() - space::S3, rect.center().y),
        Align2::RIGHT_CENTER,
        right,
        t.strong_font(type_scale::MONEY_SM),
        t.p.fg_default,
    );
}
/// One receipt: number, time, name over method, status, total. True = picked.
fn history_row(app: &App, ui: &mut egui::Ui, t: &Theme, record: &SaleRecord, width: f32) -> bool {
    let lang = app.lang;
    let selected = app
        .history
        .selected
        .as_ref()
        .is_some_and(|s| s.sale.id == record.sale.id);
    let h = if t.touch() { 60.0 } else { 46.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            true,
            selected,
            format!(
                "{} {}",
                format::receipt_number(record.sale.number),
                format::baht(record.sale.total)
            ),
        )
    });
    let inner = rect.shrink2(vec2(if t.soft() { 4.0 } else { 0.0 }, 2.0));
    let bg = if selected {
        t.p.bg_selected
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(inner, t.radius(8), bg);
    if selected && !t.soft() {
        ui.painter().rect_filled(
            Rect::from_min_size(inner.min, vec2(2.0, inner.height())),
            0,
            t.p.border_selected,
        );
    }
    let cy = rect.center().y;
    let voided = record.cancellation.is_some();
    let ink = if voided { t.p.fg_muted } else { t.p.fg_default };
    let mut x = rect.left() + space::S3;
    ui.painter().text(
        pos2(x, cy),
        Align2::LEFT_CENTER,
        format::receipt_number(record.sale.number),
        t.strong_font(type_scale::MONEY),
        ink,
    );
    x += 90.0;
    ui.painter().text(
        pos2(x, cy),
        Align2::LEFT_CENTER,
        format::clock(record.sale.created_at),
        t.font(type_scale::MONEY_SM),
        t.p.fg_muted,
    );
    x += 56.0;
    let total_w = 110.0;
    let status_w = 120.0;
    let name_w = (rect.right() - x - total_w - status_w - space::S3).max(60.0);
    let (name, name_color) = if record.sale.label.is_empty() {
        (
            lang.text("ไม่มีชื่อบิล", "No bill name").to_owned(),
            t.p.fg_subtle,
        )
    } else {
        (record.sale.label.clone(), ink)
    };
    let mut job =
        egui::text::LayoutJob::simple_singleline(name, t.font(type_scale::BODY), name_color);
    job.wrap.max_width = name_w;
    job.wrap.max_rows = 1;
    let g = ui.painter().layout_job(job);
    let gh = g.size().y;
    ui.painter().galley(pos2(x, cy - gh + 1.0), g, name_color);
    let method = if record.sale.is_split() {
        lang.text("แบ่งจ่าย", "Split")
    } else {
        lang.payment_method(record.sale.payment_method)
    };
    let staff = record
        .sale
        .staff
        .by
        .as_ref()
        .map(|s| format!(" · {}", s.name))
        .unwrap_or_default();
    ui.painter().text(
        pos2(x, cy + 2.0),
        Align2::LEFT_TOP,
        format!("{method}{staff}"),
        t.font(type_scale::CAPTION),
        t.p.fg_muted,
    );
    let (label, tone) = if voided {
        (lang.text("ยกเลิก", "Voided"), w::Tone::Danger)
    } else {
        status(lang, record)
    };
    let (bg, fg) = tone.colors(t);
    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), t.strong_font(type_scale::CAPTION), fg);
    let status_x = rect.right() - space::S3 - total_w - g.size().x - 12.0;
    let r = Rect::from_min_size(pos2(status_x, cy - 11.0), vec2(g.size().x + 12.0, 22.0));
    ui.painter().rect_filled(r, t.radius(4), bg);
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
    // The receipt total never changes; voided bills read as struck out.
    let amount = ui.painter().text(
        pos2(rect.right() - space::S3, cy),
        Align2::RIGHT_CENTER,
        format::money(record.sale.total),
        t.strong_font(type_scale::MONEY),
        ink,
    );
    if voided {
        ui.painter().hline(
            amount.x_range(),
            amount.center().y,
            Stroke::new(1.0, t.p.fg_muted),
        );
    }
    response.clicked()
}
pub fn receipt(ui: &mut egui::Ui, record: &SaleRecord, lang: Language) {
    let t = Theme::of(ui);
    let sale = &record.sale;
    ui.spacing_mut().item_spacing.y = space::S2;
    ui.horizontal(|ui| {
        ui.label(t.text(
            format!(
                "{} {}",
                lang.text("บิล", "Receipt"),
                format::receipt_number(sale.number)
            ),
            type_scale::TITLE,
        ));
        let (label, tone) = status(lang, record);
        w::badge(ui, label, tone);
    });
    ui.label(t.muted(
        format!(
            "{} · {}{}",
            format::date_time(lang, sale.created_at),
            lang.sale_method(sale),
            if sale.label.is_empty() {
                String::new()
            } else {
                format!(" · {}", sale.label)
            }
        ),
        type_scale::LABEL,
    ));
    if let Some(c) = &record.cancellation {
        let (bg, fg) = w::Tone::Danger.colors(&t);
        egui::Frame::new()
            .fill(bg)
            .corner_radius(t.radius(8))
            .inner_margin(egui::Margin::same(space::S3 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} {}",
                        lang.text("คืนเงิน", "Refunded"),
                        format::baht(c.refund),
                        lang.text("ผ่าน", "via"),
                        lang.payment_method(c.refund_method)
                    ))
                    .font(t.strong_font(type_scale::BODY))
                    .color(fg),
                );
                ui.label(t.text(
                    format!("{} · {}", format::date_time(lang, c.created_at), c.reason),
                    type_scale::LABEL,
                ));
                let units: u64 = c.restocked.iter().map(|r| u64::from(r.quantity)).sum();
                ui.label(t.muted(
                    format!(
                        "{} {}",
                        lang.text("คืนสต็อก", "Restocked"),
                        format::count(units)
                    ),
                    type_scale::CAPTION,
                ));
            });
    }
    for r in &record.refunds {
        let (bg, fg) = w::Tone::Warning.colors(&t);
        egui::Frame::new()
            .fill(bg)
            .corner_radius(t.radius(8))
            .inner_margin(egui::Margin::same(space::S3 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} {}",
                        lang.text("คืนบางส่วน", "Partial refund"),
                        format::baht(r.refund),
                        lang.text("ผ่าน", "via"),
                        lang.payment_method(r.refund_method)
                    ))
                    .font(t.strong_font(type_scale::BODY))
                    .color(fg),
                );
                ui.label(t.text(
                    format!("{} · {}", format::date_time(lang, r.created_at), r.reason),
                    type_scale::LABEL,
                ));
                let items: Vec<String> = r
                    .lines
                    .iter()
                    .map(|l| format!("{} × {}", l.name, l.quantity))
                    .collect();
                ui.label(t.muted(items.join(" · "), type_scale::CAPTION));
            });
    }
    w::divider(ui);
    for line in &sale.lines {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format::count(u64::from(line.quantity)))
                    .font(t.strong_font(type_scale::MONEY_SM)),
            );
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 110.0).max(80.0));
                ui.spacing_mut().item_spacing.y = 0.0;
                w::name_label(ui, &line.name, type_scale::BODY, t.p.fg_default);
                let mut sub = format!("{} · × {}", line.sku, format::money(line.unit_price));
                if !line.note.is_empty() {
                    sub = format!("{} · {sub}", line.note);
                }
                let options: Vec<String> = line
                    .options
                    .iter()
                    .map(|o| {
                        if o.price.satang() > 0 {
                            format!("{} +{}", o.name, format::money(o.price))
                        } else {
                            o.name.clone()
                        }
                    })
                    .collect();
                if !options.is_empty() {
                    ui.label(
                        RichText::new(options.join(" · "))
                            .font(t.strong_font(type_scale::CAPTION))
                            .color(t.p.fg_accent),
                    );
                }
                ui.label(t.muted(sub, type_scale::CAPTION));
                if line.discount.satang() > 0 {
                    let rule = match &line.promotion {
                        Some(name) => format!("{} {name}", lang.text("โปร", "Promo")),
                        None => line.discount_rule.map_or(
                            lang.text("ส่วนลด", "Discount").to_owned(),
                            |r| crate::discount_ui::label(lang, r),
                        ),
                    };
                    ui.label(
                        RichText::new(format!("{rule} {}", format::minus(line.discount)))
                            .font(t.font(type_scale::CAPTION))
                            .color(t.p.danger_fg),
                    );
                }
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                ui.label(RichText::new(format::money(line.total)).font(t.font(type_scale::MONEY)));
            });
        });
    }
    w::divider(ui);
    let extras =
        sale.service_charge.satang() > 0 || sale.vat_added.satang() > 0 || sale.rounding != 0;
    if sale.discounts().satang() > 0 || extras {
        w::kv(
            ui,
            lang.text("รวมรายการ", "Items total"),
            &format::money(sale.subtotal()),
            w::Value::Plain,
        );
    }
    if sale.discounts().satang() > 0 {
        let line_off = sale.discounts().satang() - sale.discount.satang();
        if line_off > 0 {
            w::kv(
                ui,
                lang.text("ส่วนลดรายการ", "Line discounts"),
                &format::signed(pos_domain::SignedMoney::from_satang(-line_off)),
                w::Value::Danger,
            );
        }
        if sale.discount.satang() > 0 {
            let label = match (bill_promo(sale), sale.discount_rule) {
                (Some(name), _) => format!("{} {name}", lang.text("โปร", "Promo")),
                (None, Some(rule)) => format!(
                    "{} {}",
                    lang.text("ส่วนลดทั้งบิล", "Bill discount"),
                    format::discount_rule(rule)
                ),
                (None, None) => lang.text("ส่วนลดทั้งบิล", "Bill discount").to_owned(),
            };
            w::kv(ui, &label, &format::minus(sale.discount), w::Value::Danger);
        }
        if !sale.discount_reason.is_empty() {
            ui.label(t.muted(
                format!(
                    "{} {}",
                    lang.text("เหตุผลส่วนลด:", "Discount reason:"),
                    sale.discount_reason
                ),
                type_scale::CAPTION,
            ));
        }
    }
    for (label, value) in pricing_rows(lang, sale) {
        w::kv(ui, &label, &value, w::Value::Plain);
    }
    w::kv_styled(
        ui,
        lang.text("ยอดรวม", "Total"),
        &format::baht(sale.total),
        w::Value::Strong,
        type_scale::MONEY_LG,
    );
    if let Some(tax) = &sale.tax {
        w::kv(
            ui,
            lang.text("มูลค่าก่อน VAT", "Value before VAT"),
            &format::money(tax.base),
            w::Value::Plain,
        );
        w::kv(
            ui,
            &format!("VAT {}", crate::tax_ui::rate_label(tax.rate_bp)),
            &format::money(tax.vat),
            w::Value::Plain,
        );
    }
    if sale.is_split() {
        for part in &sale.tenders {
            w::kv(
                ui,
                lang.payment_method(part.method),
                &format::money(part.amount),
                w::Value::Plain,
            );
        }
    }
    if sale.payment_method == PaymentMethod::Cash || sale.cash > pos_domain::Money::ZERO {
        w::kv(
            ui,
            lang.text("รับเงินสด", "Cash received"),
            &format::money(sale.cash),
            w::Value::Plain,
        );
        w::kv(
            ui,
            lang.text("เงินทอน", "Change"),
            &format::money(sale.change),
            w::Value::Plain,
        );
    }
    if let Some(by) = &sale.staff.by {
        let mut text = format!("{} {}", lang.text("พนักงาน:", "Staff:"), by.name);
        if let Some(a) = &sale.staff.approved_by {
            text = format!(
                "{text} · {} {}",
                lang.text("ส่วนลดอนุมัติโดย", "discount approved by"),
                a.name
            );
        }
        w::help_text(ui, &text);
    }
    for (when, staff) in record
        .refunds
        .iter()
        .map(|r| (r.created_at, &r.staff))
        .chain(record.cancellation.iter().map(|c| (c.created_at, &c.staff)))
    {
        if let Some(by) = &staff.by {
            let mut text = format!(
                "{} {} · {}",
                lang.text("คืนเงินโดย", "Refunded by"),
                by.name,
                format::date_time(lang, when)
            );
            if let Some(a) = &staff.approved_by {
                text = format!(
                    "{text} · {} {}",
                    lang.text("อนุมัติโดย", "approved by"),
                    a.name
                );
            }
            w::help_text(ui, &text);
        }
    }
    if let Some(invoice) = &record.tax_invoice {
        w::help_text(
            ui,
            &format!(
                "{} TI-{:06} · {} · {}",
                lang.text("ใบกำกับภาษีเต็มรูป", "Full tax invoice"),
                invoice.number,
                invoice.buyer.name,
                format::date_time(lang, invoice.created_at)
            ),
        );
    }
    for note in &record.credit_notes {
        w::help_text(
            ui,
            &format!(
                "{} CN-{:06} · {} {} · {}",
                lang.text("ใบลดหนี้", "Credit note"),
                note.number,
                lang.text("ลด", "credited"),
                format::money(note.total),
                format::date_time(lang, note.created_at)
            ),
        );
    }
    w::help_text(
        ui,
        if sale.tax.is_some() {
            lang.text(
                "ใบกำกับภาษีอย่างย่อ · ราคารวม VAT แล้ว",
                "Abbreviated tax invoice · VAT included",
            )
        } else {
            lang.text(
                "บันทึกการขายภายในร้าน · ไม่ใช่ใบกำกับภาษี",
                "Internal sale record · not a tax invoice",
            )
        },
    );
}
fn detail(app: &mut App, ui: &mut egui::Ui, record: &SaleRecord) {
    let lang = app.lang;
    let t = Theme::of(ui);
    egui::ScrollArea::vertical()
        .id_salt("receipt-detail")
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 60.0)
        .show(ui, |ui| {
            // The receipt as a paper slip, then what happened to it.
            let width = ui.available_width().min(320.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                egui::Frame::new()
                    .fill(t.p.bg_raised)
                    .stroke(Stroke::new(1.0, t.p.border_subtle))
                    .shadow(t.shadow(1))
                    .corner_radius(t.radius(crate::ui::tokens::radius::SM))
                    .inner_margin(egui::Margin::same(space::S4 as i8))
                    .show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
                            ui.set_width(width - space::S4 * 2.0);
                            receipt(ui, record, lang);
                        });
                    });
            });
            ui.add_space(space::S4);
            ui.label(t.text(lang.text("ลำดับเหตุการณ์", "Timeline"), type_scale::LABEL));
            ui.add_space(space::S1);
            timeline(ui, &t, lang, record);
        });
    w::divider(ui);
    ui.add_space(space::S2);
    ui.horizontal_wrapped(|ui| {
        crate::print_ui::receipt_button(app, ui, record, true);
        crate::tax_ui::invoice_button(app, ui, record);
        for note in &record.credit_notes {
            if w::button(format!(
                "{} CN-{:06}",
                lang.text("ใบลดหนี้", "Credit note"),
                note.number
            ))
            .icon(Icon::Printer)
            .ghost()
            .show(ui)
            .clicked()
            {
                app.preview_credit_note(note);
            }
        }
        if !record.fully_refunded() {
            refund_actions(app, ui, record);
        }
    });
}
/// Payment, invoice, refunds and void of a receipt, oldest first.
fn timeline(ui: &mut egui::Ui, t: &Theme, lang: Language, record: &SaleRecord) {
    let by = |a: &pos_domain::Attribution| {
        a.by.as_ref()
            .map(|s| format!(" · {}", s.name))
            .unwrap_or_default()
    };
    let sale = &record.sale;
    let mut events: Vec<(i64, String, String, Color32)> = Vec::new();
    let paid = if sale.is_split() {
        sale.tenders
            .iter()
            .map(|p| {
                format!(
                    "{} {}",
                    lang.payment_method(p.method),
                    format::money(p.amount)
                )
            })
            .collect::<Vec<_>>()
            .join(" + ")
    } else if sale.payment_method == PaymentMethod::Cash {
        format!(
            "{} {} {} {}",
            lang.text("รับ", "received"),
            format::money(sale.cash),
            lang.text("ทอน", "change"),
            format::money(sale.change)
        )
    } else {
        format::money(sale.total)
    };
    events.push((
        sale.created_at,
        format!(
            "{} {}",
            lang.text("ชำระเงิน", "Paid"),
            if sale.is_split() {
                lang.text("แบ่งจ่าย", "split")
            } else {
                lang.payment_method(sale.payment_method)
            }
        ),
        format!(
            "{} · {paid}{}",
            format::date_time(lang, sale.created_at),
            by(&sale.staff)
        ),
        t.p.success_solid,
    ));
    if let Some(inv) = &record.tax_invoice {
        events.push((
            inv.created_at,
            format!(
                "{} {}",
                lang.text("ออกใบกำกับภาษี", "Tax invoice"),
                inv.number
            ),
            format::date_time(lang, inv.created_at),
            t.p.info_solid,
        ));
    }
    for r in &record.refunds {
        events.push((
            r.created_at,
            format!(
                "{} {}",
                lang.text("คืนเงิน", "Refund"),
                format::money(r.refund)
            ),
            format!(
                "{} · {}{}",
                format::date_time(lang, r.created_at),
                if r.reason.is_empty() {
                    lang.payment_method(r.refund_method).to_owned()
                } else {
                    r.reason.clone()
                },
                by(&r.staff)
            ),
            t.p.warning_solid,
        ));
    }
    if let Some(v) = &record.cancellation {
        events.push((
            v.created_at,
            format!(
                "{} · {} {}",
                lang.text("ยกเลิกทั้งบิล", "Voided"),
                lang.text("คืน", "refund"),
                format::money(v.refund)
            ),
            format!("{} · {}", format::date_time(lang, v.created_at), v.reason),
            t.p.danger_solid,
        ));
    }
    events.sort_by_key(|e| e.0);
    let n = events.len();
    for (i, (_, title, detail, color)) in events.into_iter().enumerate() {
        let r = ui
            .horizontal_top(|ui| {
                ui.add_space(22.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(t.text(title, type_scale::BODY_STRONG));
                    ui.add(egui::Label::new(t.muted(detail, type_scale::CAPTION)).wrap());
                });
            })
            .response
            .rect;
        let c = pos2(r.left() + 8.0, r.top() + 9.0);
        if i + 1 < n {
            ui.painter().vline(
                c.x,
                egui::Rangef::new(c.y, r.bottom() + space::S2 + 9.0),
                Stroke::new(1.5, t.p.border_default),
            );
        }
        ui.painter().circle_filled(c, 5.0, color);
        ui.add_space(space::S2);
    }
}
/// Refund some items; a whole-sale void only while nothing was refunded yet.
fn refund_actions(app: &mut App, ui: &mut egui::Ui, record: &SaleRecord) {
    let lang = app.lang;
    let mut open = None;
    // Cashiers see the lock: a manager approves with a PIN (patterns §6).
    // Approval shows a lock (patterns §6); a denied action is disabled.
    let gate =
        |b: w::Button, icon: Icon, p: pos_domain::Permission| match crate::staff_ui::access(app, p)
        {
            pos_domain::Access::Allow => b.icon(icon).enabled(!app.busy),
            pos_domain::Access::Approval => b
                .icon(Icon::Lock)
                .tooltip(lang.text("ต้องให้ผู้มีสิทธิ์อนุมัติ", "Needs approval"))
                .enabled(!app.busy),
            pos_domain::Access::Deny => b
                .icon(Icon::Lock)
                .tooltip(lang.text("บทบาทของคุณทำไม่ได้", "Your role cannot do this"))
                .enabled(false),
        };
    if record.refunds.is_empty()
        && gate(
            w::button(lang.text("ยกเลิกทั้งบิล", "Void whole sale")).danger_quiet(),
            Icon::Undo,
            pos_domain::Permission::Void,
        )
        .show(ui)
        .clicked()
    {
        open = Some(true);
    }
    if gate(
        w::button(lang.text("คืนสินค้าบางรายการ", "Refund items")).danger_quiet(),
        Icon::Minus,
        pos_domain::Permission::Refund,
    )
    .show(ui)
    .clicked()
    {
        open = Some(false);
    }
    let Some(whole) = open else {
        return;
    };
    {
        if app.data.shift.is_some() {
            app.receipt = None;
            if whole {
                app.void_editor = Some(VoidEditor::new(record.sale.clone()));
            } else {
                app.history.refund_editor = Some(RefundEditor::new(record.clone()));
            }
        } else {
            app.toast(lang.text(
                "เปิดกะก่อนคืนเงิน แล้วกลับมาที่ประวัติขาย",
                "Open a shift, then come back to refund.",
            ));
            crate::shift_ui::goto_open(app);
        }
    }
}
pub fn receipt_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(record) = app.receipt.clone() else {
        return;
    };
    let lang = app.lang;
    let r = w::dialog(ctx, "receipt", 480.0, |ui| {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                if w::icon_button_sized(
                    ui,
                    Icon::Close,
                    lang.text("ปิด", "Close"),
                    Some(32.0),
                    false,
                )
                .clicked()
                {
                    app.receipt = None;
                }
            });
        });
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 220.0).max(200.0))
            .show(ui, |ui| receipt(ui, &record, lang));
        w::dialog_actions(ui, |ui| {
            if !record.fully_refunded() {
                refund_actions(app, ui, &record);
            }
            crate::tax_ui::invoice_button(app, ui, &record);
            crate::print_ui::receipt_button(app, ui, &record, true);
        });
    });
    if r.should_close() {
        app.receipt = None;
    }
}

pub struct VoidEditor {
    sale: Sale,
    reason: String,
    legacy_reviewed: bool,
    legacy_restock: HashSet<Uuid>,
}
impl VoidEditor {
    pub fn new(sale: Sale) -> Self {
        Self {
            sale,
            reason: String::new(),
            legacy_reviewed: false,
            legacy_restock: HashSet::new(),
        }
    }
}
const REASONS: [(&str, &str); 4] = [
    ("ลูกค้าเปลี่ยนใจ", "Customer changed mind"),
    ("คิดเงินผิด", "Charged incorrectly"),
    ("สินค้ามีปัญหา", "Product problem"),
    ("ทำรายการซ้ำ", "Duplicate sale"),
];
/// Kept after failure so the operator can retry without losing the reason.
pub fn void_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut editor) = app.void_editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let response = w::dialog(ctx, "void-sale", 520.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} {} {}?",
                lang.text("ยกเลิกบิล", "Void receipt"),
                format::receipt_number(editor.sale.number),
                lang.text("ทั้งบิล", "entirely")
            ),
            true,
        );
        let valid = egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 360.0).clamp(160.0, 460.0))
            .show(ui, |ui| void_contents(&mut editor, ui, lang, app.busy, &t))
            .inner;
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        w::help_text(
            ui,
            if editor.sale.payment_method == PaymentMethod::Cash
                || editor.sale.cash > pos_domain::Money::ZERO
            {
                lang.text(
                    "ส่งเงินสดคืนลูกค้าแล้วจึงกดยืนยัน",
                    "Confirm after handing back the cash.",
                )
            } else {
                lang.text(
                    "คืนเงินผ่านช่องทางเดิมให้เสร็จก่อนยืนยัน แอปไม่โอนเงินหรือคืนยอดบัตรให้",
                    "Complete the refund through the original channel first. The app does not transfer money or reverse card charges.",
                )
            },
        );
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ยกเลิกบิลและคืนเงิน", "Void & refund"))
                .danger()
                .money(format::baht(editor.sale.total))
                .enabled(valid && app.data.shift.is_some())
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("ไม่ยกเลิก", "Keep receipt"))
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
        Some(true) => {
            if let Some(shift) = &app.data.shift {
                app.send(Command::Void(
                    VoidRequest {
                        sale_id: editor.sale.id,
                        refund_method: editor.sale.payment_method,
                        reason: editor.reason.trim().to_owned(),
                        legacy_restock: editor
                            .sale
                            .lines
                            .iter()
                            .any(|line| line.stock_tracked.is_none())
                            .then(|| editor.legacy_restock.iter().copied().collect()),
                    },
                    shift.id,
                ));
            }
        }
        None => {}
    }
    app.void_editor = Some(editor);
}
fn void_contents(
    editor: &mut VoidEditor,
    ui: &mut egui::Ui,
    lang: Language,
    busy: bool,
    t: &Theme,
) -> bool {
    ui.spacing_mut().item_spacing.y = space::S2;
    ui.label(t.muted(
        lang.text("คืนเงินให้ลูกค้า", "Refund to customer"),
        type_scale::LABEL,
    ));
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format::baht(editor.sale.total))
                .font(t.strong_font(type_scale::MONEY_XL)),
        );
        ui.label(t.muted(
            format!(
                "{} {}",
                lang.text("ผ่าน", "via"),
                lang.sale_method(&editor.sale)
            ),
            type_scale::BODY,
        ));
    });
    if editor.sale.is_split() {
        let parts: Vec<String> = editor
            .sale
            .tenders
            .iter()
            .map(|p| {
                format!(
                    "{} {}",
                    lang.payment_method(p.method),
                    format::money(p.amount)
                )
            })
            .collect();
        ui.label(t.text(
            format!(
                "{} {}",
                lang.text(
                    "คืนแต่ละส่วนตามช่องทางเดิม:",
                    "Each part goes back by its method:"
                ),
                parts.join(" · ")
            ),
            type_scale::BODY,
        ));
    }
    ui.add(egui::Label::new(t.text(
        lang.text(
            "บิลเดิมยังอยู่และแก้ไขไม่ได้ ระบบบันทึกการยกเลิก คืนเงินเข้ากะปัจจุบัน และคืนสต็อกเมื่อยืนยัน",
            "The original receipt stays unchanged. Confirming records the void, the refund in the current shift, and returns stock.",
        ),
        type_scale::BODY,
    )).wrap());
    ui.add_enabled_ui(!busy, |ui| {
        w::field_label(ui, lang.text("เหตุผล", "Reason"), None);
        ui.horizontal_wrapped(|ui| {
            for (th, en) in REASONS {
                let label = lang.text(th, en);
                if w::chip(ui, label, editor.reason == label, false).clicked() {
                    editor.reason = label.to_owned();
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
        let mut legacy = BTreeMap::<Uuid, (String, u64)>::new();
        let mut automatic = BTreeMap::<Uuid, (String, u64)>::new();
        for line in &editor.sale.lines {
            let target = match line.stock_tracked {
                None => &mut legacy,
                Some(true) => &mut automatic,
                Some(false) => continue,
            };
            let row = target.entry(line.product_id).or_insert((line.name.clone(), 0));
            row.1 += u64::from(line.quantity);
        }
        if !automatic.is_empty() {
            w::divider(ui);
            ui.label(t.text(lang.text("คืนสต็อกอัตโนมัติ", "Returned to stock automatically"), type_scale::BODY_STRONG));
            for (name, quantity) in automatic.values() {
                w::kv(ui, name, &format::count(*quantity), w::Value::Plain);
            }
        }
        if !legacy.is_empty() {
            w::divider(ui);
            w::banner(
                ui,
                w::Tone::Warning,
                lang.text(
                    "บิลเก่าไม่มีประวัติการตัดสต็อก เลือกเฉพาะสินค้าที่ต้องคืนสต็อก",
                    "This older receipt has no stock record. Select only items that should return to stock.",
                ),
                None,
            );
            for (id, (name, quantity)) in &legacy {
                let mut selected = editor.legacy_restock.contains(id);
                if w::check_label(ui, &mut selected, &format!("{name} × {quantity}")).changed() {
                    editor.legacy_reviewed = false;
                    if selected {
                        editor.legacy_restock.insert(*id);
                    } else {
                        editor.legacy_restock.remove(id);
                    }
                }
            }
            w::check_label(
                ui,
                &mut editor.legacy_reviewed,
                lang.text("ตรวจรายการคืนสต็อกของบิลเก่าแล้ว", "I reviewed the stock choices for this older receipt"),
            );
        }
        !editor.reason.trim().is_empty() && (legacy.is_empty() || editor.legacy_reviewed)
    })
    .inner
}

/// Partial refund: choose units per receipt line. Kept after a failure so the
/// operator can retry with the same request ID.
pub struct RefundEditor {
    record: SaleRecord,
    /// Method the money goes back by; a split sale offers each of its parts.
    method: PaymentMethod,
    id: Uuid,
    quantities: Vec<u32>,
    reason: String,
    legacy_reviewed: bool,
    legacy_restock: HashSet<usize>,
}
impl RefundEditor {
    pub fn new(record: SaleRecord) -> Self {
        let n = record.sale.lines.len();
        Self {
            method: record
                .sale
                .paid_by()
                .first()
                .map_or(record.sale.payment_method, |p| p.method),
            record,
            id: Uuid::new_v4(),
            quantities: vec![0; n],
            reason: String::new(),
            legacy_reviewed: false,
            legacy_restock: HashSet::new(),
        }
    }
    #[cfg(debug_assertions)]
    pub fn preview_pick(&mut self) {
        if let Some(q) = self.quantities.first_mut() {
            *q = 1;
        }
        self.reason = "ลูกค้าคืน".into();
    }
    fn amount(&self) -> Option<pos_domain::Money> {
        self.record
            .sale
            .lines
            .iter()
            .zip(&self.quantities)
            .zip(self.record.refunded_units())
            .try_fold(pos_domain::Money::ZERO, |sum, ((line, q), done)| {
                if *q == 0 {
                    return Some(sum);
                }
                // Same rule as the core: what was paid, after discounts.
                let share = pos_domain::refund_share(line.net(), line.quantity, done, *q).ok()?;
                sum.checked_add(share).ok()
            })
    }
    fn legacy_lines(&self) -> Vec<usize> {
        self.record
            .sale
            .lines
            .iter()
            .enumerate()
            .filter(|(i, l)| l.stock_tracked.is_none() && self.quantities[*i] > 0)
            .map(|(i, _)| i)
            .collect()
    }
    fn request(&self) -> Option<pos_domain::RefundRequest> {
        let lines: Vec<pos_domain::RefundLine> = self
            .quantities
            .iter()
            .enumerate()
            .filter(|(_, q)| **q > 0)
            .map(|(line, q)| pos_domain::RefundLine { line, quantity: *q })
            .collect();
        let legacy = self.legacy_lines();
        let request = pos_domain::RefundRequest {
            id: self.id,
            sale_id: self.record.sale.id,
            refund_method: self.method,
            reason: self.reason.trim().to_owned(),
            lines,
            legacy_restock: (!legacy.is_empty()).then(|| {
                legacy
                    .into_iter()
                    .filter(|i| self.legacy_restock.contains(i))
                    .collect()
            }),
        };
        let reviewed = request.legacy_restock.is_none() || self.legacy_reviewed;
        (request.validate().is_ok() && reviewed).then_some(request)
    }
}
pub fn refund_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut editor) = app.history.refund_editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let left: Vec<u32> = editor
        .record
        .refunded_units()
        .iter()
        .zip(&editor.record.sale.lines)
        .map(|(done, l)| l.quantity.saturating_sub(*done))
        .collect();
    let response = w::dialog(ctx, "refund-items", 560.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} {}",
                lang.text("คืนสินค้าจากบิล", "Refund items from receipt"),
                format::receipt_number(editor.record.sale.number)
            ),
            true,
        );
        ui.add_space(space::S2);
        egui::ScrollArea::vertical()
            // Room left after the title, amount, reason and actions.
            .max_height(
                (ctx.content_rect().height() - if t.touch() { 560.0 } else { 420.0 })
                    .clamp(120.0, 360.0),
            )
            .show(ui, |ui| {
                for (i, line) in editor.record.sale.lines.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 170.0).max(120.0));
                            ui.spacing_mut().item_spacing.y = 0.0;
                            w::name_label(ui, &line.name, type_scale::BODY, t.p.fg_default);
                            let mut sub = format!(
                                "× {} · {} {} · {} {}",
                                format::money(line.unit_price),
                                lang.text("ขาย", "sold"),
                                line.quantity,
                                lang.text("คืนได้อีก", "left"),
                                left[i]
                            );
                            if line.net() != line.total {
                                sub = format!(
                                    "{sub} · {} {}",
                                    lang.text("หลังส่วนลด", "after discount"),
                                    format::money(line.net())
                                );
                            }
                            if !line.options.is_empty() {
                                let names: Vec<&str> =
                                    line.options.iter().map(|o| o.name.as_str()).collect();
                                sub = format!("{} · {sub}", names.join(", "));
                            }
                            ui.label(t.muted(sub, type_scale::CAPTION));
                        });
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = space::S1;
                            let q = &mut editor.quantities[i];
                            ui.add_enabled_ui(*q < left[i] && !app.busy, |ui| {
                                if w::icon_button(ui, Icon::Plus, lang.text("คืนเพิ่ม", "More"))
                                    .clicked()
                                {
                                    *q += 1;
                                }
                            });
                            let (r, _) =
                                ui.allocate_exact_size(vec2(44.0, t.m.control_md), Sense::hover());
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                q.to_string(),
                                t.strong_font(type_scale::MONEY),
                                if *q > 0 {
                                    t.p.fg_default
                                } else {
                                    t.p.fg_subtle
                                },
                            );
                            ui.add_enabled_ui(*q > 0 && !app.busy, |ui| {
                                if w::icon_button(ui, Icon::Minus, lang.text("คืนน้อยลง", "Fewer"))
                                    .clicked()
                                {
                                    *q -= 1;
                                }
                            });
                        });
                    });
                    w::divider(ui);
                }
            });
        if editor.record.tax_invoice.is_some() {
            w::banner(
                ui,
                w::Tone::Info,
                lang.text(
                    "บิลนี้มีใบกำกับภาษีเต็มรูป เมื่อยืนยันระบบจะออกใบลดหนี้ให้อัตโนมัติ พิมพ์ได้จากรายละเอียดบิล",
                    "This sale has a full tax invoice. Confirming issues a credit note, which you can print from the receipt details.",
                ),
                None,
            );
        }
        let legacy = editor.legacy_lines();
        if !legacy.is_empty() {
            w::banner(
                ui,
                w::Tone::Warning,
                lang.text(
                    "บิลเก่าไม่มีประวัติการตัดสต็อก เลือกเฉพาะรายการที่ต้องคืนเข้าสต็อก",
                    "This older receipt has no stock record. Tick only items that go back to stock.",
                ),
                None,
            );
            for i in legacy {
                let mut on = editor.legacy_restock.contains(&i);
                let name = editor.record.sale.lines[i].name.clone();
                if w::check_label(ui, &mut on, &format!("{name} × {}", editor.quantities[i]))
                    .changed()
                {
                    editor.legacy_reviewed = false;
                    if on {
                        editor.legacy_restock.insert(i);
                    } else {
                        editor.legacy_restock.remove(&i);
                    }
                }
            }
            w::check_label(
                ui,
                &mut editor.legacy_reviewed,
                lang.text(
                    "ตรวจรายการคืนสต็อกแล้ว",
                    "I reviewed which items return to stock",
                ),
            );
        }
        let amount = editor.amount();
        ui.add_space(space::S1);
        if editor.record.sale.is_split() {
            // Refunds by a method never exceed what that method paid.
            let options: Vec<(PaymentMethod, String)> = editor
                .record
                .sale
                .tenders
                .iter()
                .map(|p| {
                    let used = editor
                        .record
                        .refunds
                        .iter()
                        .filter(|r| r.refund_method == p.method)
                        .fold(0_i64, |sum, r| sum + r.refund.satang());
                    let left = pos_domain::Money::from_satang(p.amount.satang() - used)
                        .unwrap_or_default();
                    (
                        p.method,
                        format!(
                            "{} ≤ {}",
                            lang.payment_method(p.method),
                            format::money(left)
                        ),
                    )
                })
                .collect();
            let refs: Vec<(PaymentMethod, &str)> =
                options.iter().map(|(m, s)| (*m, s.as_str())).collect();
            ui.horizontal(|ui| {
                ui.label(t.muted(lang.text("คืนผ่าน", "Refund by"), type_scale::LABEL));
                w::segmented(ui, &mut editor.method, &refs);
            });
        }
        ui.horizontal(|ui| {
            ui.label(t.text(lang.text("คืนเงิน", "Refund"), type_scale::BODY_STRONG));
            ui.label(t.muted(
                format!(
                    "{} {}",
                    lang.text("ผ่าน", "via"),
                    lang.payment_method(editor.method)
                ),
                type_scale::LABEL,
            ));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(amount.map_or("—".into(), format::baht))
                        .font(t.strong_font(type_scale::MONEY_LG)),
                );
            });
        });
        w::field_label(ui, lang.text("เหตุผล", "Reason"), None);
        ui.horizontal_wrapped(|ui| {
            for (th, en) in REASONS {
                let label = lang.text(th, en);
                if w::chip(ui, label, editor.reason == label, false).clicked() {
                    editor.reason = label.to_owned();
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
        w::help_text(
            ui,
            if editor.method == PaymentMethod::Cash {
                lang.text(
                    "ส่งเงินสดคืนลูกค้าแล้วจึงกดยืนยัน · บิลเดิมไม่ถูกแก้ไข",
                    "Confirm after handing back the cash · the receipt is not changed",
                )
            } else {
                lang.text(
                    "คืนเงินผ่านช่องทางเดิมให้เสร็จก่อนยืนยัน แอปไม่โอนหรือคืนยอดบัตรให้",
                    "Complete the refund through the original channel first. The app does not transfer or reverse card charges.",
                )
            },
        );
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        let request = editor.request();
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("คืนเงิน", "Refund"))
                .danger()
                .money(
                    amount
                        .filter(|_| request.is_some())
                        .map_or(String::new(), format::baht),
                )
                .enabled(request.is_some() && app.data.shift.is_some())
                .loading(app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(true);
            }
            if w::button(lang.text("ไม่คืน", "Cancel"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                out = Some(false);
            }
        });
        out.map(|go| (go, request))
    });
    match response.inner {
        Some((false, _)) => return,
        None if !app.busy && response.should_close() => return,
        Some((true, Some(request))) => {
            if let Some(shift) = &app.data.shift {
                app.send(Command::RefundItems(request, shift.id));
            }
        }
        _ => {}
    }
    app.history.refund_editor = Some(editor);
}

/// Service charge, VAT added on top and cash rounding, for receipts.
pub fn pricing_rows(lang: Language, sale: &Sale) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    if sale.service_charge.satang() > 0 {
        rows.push((
            format!(
                "Service charge {}",
                crate::tax_ui::rate_label(sale.service_rate_bp)
            ),
            format::money(sale.service_charge),
        ));
    }
    if sale.vat_added.satang() > 0 {
        let rate = sale.tax.as_ref().map_or(0, |t| t.rate_bp);
        rows.push((
            format!("VAT {}", crate::tax_ui::rate_label(rate)),
            format::money(sale.vat_added),
        ));
    }
    if sale.rounding != 0 {
        rows.push((
            lang.text("ปัดเศษเงินสด", "Cash rounding").to_owned(),
            format::signed(pos_domain::SignedMoney::from_satang(sale.rounding)),
        ));
    }
    rows
}
/// The promotion behind a sale's bill discount: one used on the bill that
/// no line names.
pub fn bill_promo(sale: &Sale) -> Option<&str> {
    sale.promotions
        .iter()
        .find(|p| {
            !sale
                .lines
                .iter()
                .any(|l| l.promotion.as_deref() == Some(p.name.as_str()))
        })
        .map(|p| p.name.as_str())
}
