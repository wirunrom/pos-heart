//! Daily report (design/docs/components/ScreenReports.md, StatTile.md).
//! Gross sales, refunds, net sales and receipts by method stay separate; refunds
//! belong to their cancellation day, so net can be negative.
use crate::{
    app::App,
    i18n::Language,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker,
};
use eframe::egui::{self, Align, Align2, Rect, RichText, Sense, Stroke, pos2, vec2};
use pos_domain::{DailySummary, Money, PaymentMethod, SignedMoney};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Amount,
    Quantity,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quick {
    Today,
    Yesterday,
    Week,
    Month,
    Other,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Products,
    Categories,
    Staff,
    Promotions,
    Ingredients,
}
/// Bangkok dates `start..=end` shown in Reports (at most 366 days).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Period {
    pub start: time::Date,
    pub end: time::Date,
}
impl Period {
    pub fn day(date: time::Date) -> Self {
        Self {
            start: date,
            end: date,
        }
    }
    pub fn single(self) -> bool {
        self.start == self.end
    }
    pub fn days(self) -> i64 {
        (self.end - self.start).whole_days() + 1
    }
    /// The comparison period: same weekday last week for one day, otherwise
    /// the equally long period right before.
    pub fn previous(self) -> Option<Self> {
        let back = if self.single() { 7 } else { self.days() };
        Some(Self {
            start: self.start.checked_sub(time::Duration::days(back))?,
            end: self.end.checked_sub(time::Duration::days(back))?,
        })
    }
    pub fn label(self, lang: Language) -> String {
        if self.single() {
            format::date(lang, self.start)
        } else {
            format!(
                "{} – {}",
                format::date(lang, self.start),
                format::date(lang, self.end)
            )
        }
    }
}
fn weekday(lang: Language, d: time::Date) -> &'static str {
    use time::Weekday::*;
    match (lang, d.weekday()) {
        (Language::Thai, Monday) => "จ.",
        (Language::Thai, Tuesday) => "อ.",
        (Language::Thai, Wednesday) => "พ.",
        (Language::Thai, Thursday) => "พฤ.",
        (Language::Thai, Friday) => "ศ.",
        (Language::Thai, Saturday) => "ส.",
        (Language::Thai, Sunday) => "อา.",
        (Language::English, Monday) => "Mon",
        (Language::English, Tuesday) => "Tue",
        (Language::English, Wednesday) => "Wed",
        (Language::English, Thursday) => "Thu",
        (Language::English, Friday) => "Fri",
        (Language::English, Saturday) => "Sat",
        (Language::English, Sunday) => "Sun",
    }
}
fn compare_label(lang: Language, period: Period) -> String {
    if !period.single() {
        return lang.text("ช่วงก่อนหน้า", "previous period").to_owned();
    }
    let previous = period
        .start
        .checked_sub(time::Duration::days(7))
        .unwrap_or(period.start);
    match lang {
        Language::Thai => format!("{} ที่แล้ว", weekday(lang, previous)),
        Language::English => format!("last {}", weekday(lang, previous)),
    }
}
/// Change in tenths of a percent, using integer arithmetic only.
fn change_tenths(now: Money, before: Money) -> Option<i64> {
    if before.satang() == 0 {
        return None;
    }
    let diff = i128::from(now.satang()) - i128::from(before.satang());
    i64::try_from(diff * 1000 / i128::from(before.satang())).ok()
}
fn set_period(app: &mut App, period: Period) {
    let valid = period.start <= period.end
        && period.days() <= pos_core::MAX_REPORT_DAYS
        && period.end.next_day().is_some();
    if valid {
        app.report = period;
        app.refresh();
    } else {
        app.error = Some(
            app.lang
                .text(
                    "ช่วงวันที่ไม่ถูกต้อง: วันเริ่มต้องไม่หลังวันสิ้นสุด และยาวไม่เกิน 366 วัน",
                    "Invalid period: the start must not be after the end, up to 366 days.",
                )
                .into(),
        );
    }
}
fn parse_date(text: &str) -> Option<time::Date> {
    time::Date::parse(
        text.trim(),
        time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    if !crate::staff_ui::may(app, pos_domain::Permission::Reports) {
        w::state_view(
            ui,
            Icon::Lock,
            lang.text("บทบาทของคุณดูรายงานไม่ได้", "Your role cannot view reports"),
            lang.text(
                "ให้เจ้าของร้านเปิดสิทธิ์ \"ดูรายงาน\" ในตั้งค่า → พนักงานและสิทธิ์",
                "Ask an owner to allow \"View reports\" in Settings → Staff & permissions.",
            ),
        );
        return;
    }
    let s = app.data.summary.clone();
    let c = app.data.compare.clone();
    let loading =
        s.date != app.report.start.to_string() || s.end_date != app.report.end.to_string();
    if loading {
        w::state_view(
            ui,
            Icon::Reports,
            lang.text("กำลังโหลดรายงาน…", "Loading report…"),
            "",
        );
        return;
    }
    tiles(app, ui, &s, &c);
    let gap = t.pane_gap();
    ui.add_space(gap - ui.spacing().item_spacing.y);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, egui::Sense::hover());
    let left_w = (full.width() * 0.62).floor();
    let left = egui::Rect::from_min_size(full.min, vec2(left_w, full.height()));
    let right_x = left.right() + gap;
    // Room for the share bar, three methods and the total.
    let pay_h = (if t.touch() { 320.0_f32 } else { 240.0 }).min(full.height() * 0.6);
    let pay = egui::Rect::from_min_max(
        egui::pos2(right_x, full.top()),
        egui::pos2(full.right(), full.top() + pay_h),
    );
    let groups_rect = egui::Rect::from_min_max(egui::pos2(right_x, pay.bottom() + gap), full.max);
    // Sales chart and profit.
    let (mut head, mut body) = w::pane_uis(ui, left);
    w::pane_title(
        &mut head,
        if app.report.single() {
            lang.text("ยอดขายรายชั่วโมง", "Sales by hour")
        } else {
            lang.text("ยอดขายรายวัน", "Sales by day")
        },
    );
    egui::ScrollArea::vertical()
        .id_salt("report-chart")
        .auto_shrink([false, false])
        .show(&mut body, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(space::S3 as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if app.report.single() {
                        hourly(app, ui, &s, &c);
                    } else {
                        daily(app, ui, &s, &c);
                    }
                    profit_body(ui, lang, &s.profit);
                });
        });
    // Receipts by method.
    let (mut head, mut body) = w::pane_uis(ui, pay);
    w::pane_title(&mut head, lang.text("รายรับตามวิธีชำระ", "Receipts by method"));
    egui::ScrollArea::vertical()
        .id_salt("report-pay")
        .auto_shrink([false, false])
        .show(&mut body, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(space::S3 as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    tenders(app, ui, &s);
                });
        });
    // Best sellers and the other groupings.
    let (mut head, mut body) = w::pane_uis(ui, groups_rect);
    let mut groups = vec![
        (Group::Products, lang.text("สินค้าขายดี", "Best sellers"), None),
        (Group::Categories, lang.text("หมวด", "Categories"), None),
    ];
    if app.data.staff_enabled || !s.staff.is_empty() {
        groups.push((Group::Staff, lang.text("พนักงาน", "Staff"), None));
    }
    if !app.data.promotions.is_empty() || !s.promotions.is_empty() {
        groups.push((Group::Promotions, lang.text("โปร", "Promos"), None));
    }
    if !app.data.ingredients.is_empty() || !s.ingredients.is_empty() {
        groups.push((Group::Ingredients, lang.text("วัตถุดิบ", "Ingredients"), None));
    }
    w::pane_tabs(&mut head, &mut app.report_group, &groups);
    egui::Frame::new()
        .inner_margin(egui::Margin::same(space::S3 as i8))
        .show(&mut body, |ui| {
            ui.set_min_size(ui.available_size());
            match app.report_group {
                Group::Products => top_products(app, ui, &s),
                Group::Categories => categories(app, ui, &s),
                Group::Staff => staff(app, ui, &s),
                Group::Promotions => promotions(app, ui, &s),
                Group::Ingredients => ingredients(app, ui, &s),
            }
        });
}
/// Reports toolbar (design v0.2): period, Export CSV, Activity log, and
/// the quick periods on the right.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    if !crate::staff_ui::may(app, pos_domain::Permission::Reports) {
        return;
    }
    let today = worker::today();
    let yesterday = today.previous_day().unwrap_or(today);
    let week = Period {
        start: today.checked_sub(time::Duration::days(6)).unwrap_or(today),
        end: today,
    };
    let month = Period {
        start: today.replace_day(1).unwrap_or(today),
        end: today,
    };
    let quick = match app.report {
        p if p == Period::day(today) => Quick::Today,
        p if p == Period::day(yesterday) => Quick::Yesterday,
        p if p == week => Quick::Week,
        p if p == month => Quick::Month,
        _ => Quick::Other,
    };
    date_picker(app, ui);
    if crate::chrome::tool(
        ui,
        Icon::Download,
        lang.text("ส่งออก CSV", "Export CSV"),
        None,
        !app.busy,
    )
    .on_hover_text(lang.text(
        "บันทึกรายงานช่วงนี้เป็นไฟล์ในโฟลเดอร์ข้อมูล",
        "Save this period's report in the data folder",
    ))
    .clicked()
    {
        app.export_report();
    }
    crate::chrome::separator(ui, &Theme::of(ui));
    if crate::chrome::tool(
        ui,
        Icon::History,
        lang.text("ประวัติกิจกรรม", "Activity log"),
        Some("Alt 8"),
        true,
    )
    .clicked()
    {
        app.navigate(crate::app::Page::Activity);
    }
    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
        let mut q = quick;
        if crate::chrome::segmented(
            ui,
            &mut q,
            &[
                (Quick::Today, lang.text("วันนี้", "Today").to_owned()),
                (
                    Quick::Yesterday,
                    lang.text("เมื่อวาน", "Yesterday").to_owned(),
                ),
                (Quick::Week, lang.text("7 วัน", "7 days").to_owned()),
                (Quick::Month, lang.text("เดือนนี้", "This month").to_owned()),
            ],
        ) {
            set_period(
                app,
                match q {
                    Quick::Today => Period::day(today),
                    Quick::Yesterday => Period::day(yesterday),
                    Quick::Week => week,
                    _ => month,
                },
            );
        }
    });
}
fn date_picker(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let response = crate::chrome::tool(ui, Icon::Calendar, &app.report.label(lang), None, true);
    egui::Popup::menu(&response)
        .frame(
            t.raised(2)
                .inner_margin(egui::Margin::same(space::S3 as i8)),
        )
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        // Menus default to a justified layout, which would stretch wrapped Thai.
        .layout(egui::Layout::top_down(Align::Min))
        .width(320.0)
        .show(|ui| {
            ui.spacing_mut().item_spacing.y = space::S2;
            // Step the whole period back or forward by its own length.
            ui.horizontal(|ui| {
                let len = time::Duration::days(app.report.days());
                if w::icon_button(ui, Icon::ChevronLeft, lang.text("ช่วงก่อน", "Previous")).clicked()
                    && let (Some(start), Some(end)) = (
                        app.report.start.checked_sub(len),
                        app.report.end.checked_sub(len),
                    )
                {
                    set_period(app, Period { start, end });
                }
                ui.label(t.text(app.report.label(lang), type_scale::BODY_STRONG));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let next = app
                        .report
                        .start
                        .checked_add(len)
                        .zip(app.report.end.checked_add(len))
                        .filter(|(_, end)| *end <= worker::today());
                    ui.add_enabled_ui(next.is_some(), |ui| {
                        if w::icon_button(ui, Icon::ChevronRight, lang.text("ช่วงถัดไป", "Next"))
                            .clicked()
                            && let Some((start, end)) = next
                        {
                            set_period(app, Period { start, end });
                        }
                    });
                });
            });
            w::field_label(
                ui,
                lang.text("กำหนดเอง (ค.ศ. ปี-เดือน-วัน)", "Custom (YYYY-MM-DD)"),
                None,
            );
            let id = egui::Id::new("report-period-input");
            let (mut from, mut to) = ui
                .data_mut(|d| d.get_temp::<(String, String)>(id))
                .unwrap_or_else(|| (app.report.start.to_string(), app.report.end.to_string()));
            ui.horizontal(|ui| {
                ui.label(t.muted(lang.text("ตั้งแต่", "From"), type_scale::LABEL));
                w::text_input(
                    ui,
                    &mut from,
                    w::FieldOpts {
                        hint: "2026-10-01",
                        limit: 10,
                        width: Some(116.0),
                        ..Default::default()
                    },
                );
                ui.label(t.muted(lang.text("ถึง", "to"), type_scale::LABEL));
                w::text_input(
                    ui,
                    &mut to,
                    w::FieldOpts {
                        hint: "2026-10-13",
                        limit: 10,
                        width: Some(116.0),
                        ..Default::default()
                    },
                );
            });
            let parsed = parse_date(&from).zip(parse_date(&to));
            if parsed.is_none() {
                w::help_text(
                    ui,
                    lang.text("ใช้รูปแบบ 2026-10-13", "Use the form 2026-10-13"),
                );
            }
            if w::button(lang.text("ดูรายงานช่วงนี้", "Show this period"))
                .size(w::Size::Sm)
                .enabled(parsed.is_some())
                .show(ui)
                .clicked()
                && let Some((start, end)) = parsed
            {
                set_period(app, Period { start, end });
                ui.close();
            }
            ui.data_mut(|d| d.insert_temp(id, (from, to)));
            w::help_text(
                ui,
                lang.text(
                    "เวลาไทย UTC+7 · ตามวันที่รับเงินหรือคืนเงิน · สูงสุด 366 วัน",
                    "Bangkok UTC+7 · by payment or refund day · up to 366 days",
                ),
            );
        });
}
fn tiles(app: &App, ui: &mut egui::Ui, s: &DailySummary, c: &DailySummary) {
    let lang = app.lang;
    let gap = Theme::of(ui).pane_gap();
    let width = ((ui.available_width() - gap * 4.0) / 5.0).floor();
    let compare = compare_label(lang, app.report);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            let sub = match change_tenths(s.gross_sales, c.gross_sales) {
                Some(tenths) => vec![
                    (
                        // Signed text instead of ▲▼: the bundled fonts lack those
                        // glyphs, and the sign carries meaning without colour.
                        format!(
                            "{}{}.{}%",
                            if tenths >= 0 { "+" } else { "\u{2212}" },
                            tenths.unsigned_abs() / 10,
                            tenths.unsigned_abs() % 10
                        ),
                        Some(if tenths >= 0 {
                            w::Tone::Success
                        } else {
                            w::Tone::Danger
                        }),
                    ),
                    (format!("{} {compare}", lang.text("เทียบ", "vs")), None),
                ],
                None => vec![(format!("{} {compare} —", lang.text("เทียบ", "vs")), None)],
            };
            w::stat_tile(
                ui,
                lang.text("ยอดขายก่อนคืน", "Gross sales"),
                &format::money(s.gross_sales),
                false,
                &sub,
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            w::stat_tile(
                ui,
                lang.text("คืนเงิน", "Refunds"),
                &format::signed(format::negative(s.refunds)),
                s.refunds.satang() > 0,
                &[(
                    format!(
                        "{} {} · {} {}",
                        s.void_count + s.refund_count,
                        lang.noun(s.void_count + s.refund_count, "ครั้ง", "refund", "refunds"),
                        format::count(s.restocked_units),
                        lang.noun(
                            s.restocked_units,
                            "ชิ้นคืนสต็อก",
                            "item restocked",
                            "items restocked"
                        )
                    ),
                    None,
                )],
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            w::stat_tile(
                ui,
                lang.text("ยอดขายสุทธิ", "Net sales"),
                &format::signed(s.net),
                s.net.satang() < 0,
                &[(
                    lang.text("= ก่อนคืน − คืน", "= gross − refunds").to_owned(),
                    None,
                )],
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            let avg = (s.sale_count > 0)
                .then(|| {
                    Money::from_satang(
                        s.gross_sales.satang() / i64::try_from(s.sale_count).unwrap_or(1),
                    )
                    .ok()
                })
                .flatten();
            w::stat_tile(
                ui,
                lang.text("บิล", "Bills"),
                &format::count(s.sale_count),
                false,
                &[(
                    format!(
                        "{} {} · {} {}",
                        lang.text("เฉลี่ย", "Avg"),
                        avg.map_or("—".into(), format::money),
                        lang.text("ยกเลิก", "voided"),
                        s.void_count
                    ),
                    None,
                )],
            );
        });
        ui.allocate_ui_with_layout(vec2(width, 0.0), egui::Layout::top_down(Align::Min), |ui| {
            let p = &s.profit;
            let known = p.known_sold_units > 0;
            w::stat_tile(
                ui,
                lang.text("กำไรขั้นต้น", "Gross profit"),
                &if known {
                    format::signed(p.gross_profit)
                } else {
                    lang.text("ยังคำนวณไม่ได้", "Not yet known").to_owned()
                },
                p.gross_profit.satang() < 0,
                &[(
                    format!(
                        "{} {}/{} {}",
                        lang.text("รู้ต้นทุน", "Cost known"),
                        p.known_sold_units,
                        p.known_sold_units + p.unknown_sold_units,
                        lang.text("ชิ้น", "units")
                    ),
                    None,
                )],
            );
        });
    });
}
fn axis_label(satang: i64) -> String {
    let baht = satang / 100;
    if baht >= 10_000 {
        format!("{}K", baht / 1000)
    } else {
        format::count(u64::try_from(baht).unwrap_or_default())
    }
}
fn nice_step(max: i64) -> i64 {
    let rough = (max / 4).max(1);
    let mut step = 100;
    for mult in [1, 2, 5].iter().cycle() {
        if step * mult >= rough {
            return step * mult;
        }
        if *mult == 5 {
            step *= 10;
        }
        if step > i64::MAX / 100 {
            return rough;
        }
    }
    rough
}
fn hourly(app: &App, ui: &mut egui::Ui, s: &DailySummary, c: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let compare = compare_label(lang, app.report);
    let today_label = if app.report == Period::day(worker::today()) {
        lang.text("วันนี้", "Today").to_owned()
    } else {
        app.report.label(lang)
    };
    ui.horizontal(|ui| {
        // The pane header names the chart; this row keeps the legend.
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            for (label, color) in [
                (compare.as_str(), t.p.chart_2),
                (today_label.as_str(), t.p.chart_1),
            ] {
                ui.label(t.muted(label, type_scale::CAPTION));
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, t.radius(2), color);
                ui.add_space(space::S2);
            }
        });
    });
    let active: Vec<usize> = (0..24)
        .filter(|h| s.hourly[*h].satang() > 0 || c.hourly[*h].satang() > 0)
        .collect();
    let first = active.first().copied().unwrap_or(8).min(8);
    let last = active.last().copied().unwrap_or(20).max(20);
    let max = (first..=last)
        .map(|h| s.hourly[h].satang().max(c.hourly[h].satang()))
        .max()
        .unwrap_or(0);
    let step = nice_step(max.max(10_000));
    let top = ((max + step - 1) / step).max(1) * step;
    // Shorter chart on short windows so receipts by method stay in view.
    let height = ((ui.ctx().content_rect().height() - 520.0) * 0.6).clamp(140.0, 220.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let axis_w = 48.0;
    let plot = Rect::from_min_max(
        pos2(rect.left() + axis_w, rect.top() + 8.0),
        pos2(rect.right(), rect.bottom() - 22.0),
    );
    let painter = ui.painter();
    let mut v = 0;
    while v <= top {
        let y = plot.bottom() - plot.height() * (v as f32 / top as f32);
        painter.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, t.p.chart_grid),
        );
        painter.text(
            pos2(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            axis_label(v),
            t.font(type_scale::MONEY_SM),
            t.p.fg_muted,
        );
        v += step;
    }
    let hours = last - first + 1;
    let slot = plot.width() / hours as f32;
    let bar = (slot * 0.34).clamp(4.0, 18.0);
    let mut hovered = None;
    for (i, h) in (first..=last).enumerate() {
        let x = plot.left() + slot * (i as f32 + 0.5);
        for (value, color, dx) in [
            (c.hourly[h], t.p.chart_2, -bar / 2.0 - 1.0),
            (s.hourly[h], t.p.chart_1, bar / 2.0 + 1.0),
        ] {
            if value.satang() <= 0 {
                continue;
            }
            let hgt = plot.height() * (value.satang() as f32 / top as f32);
            let r = Rect::from_center_size(pos2(x + dx, plot.bottom() - hgt / 2.0), vec2(bar, hgt));
            painter.rect_filled(
                r,
                egui::CornerRadius {
                    nw: 3,
                    ne: 3,
                    sw: 0,
                    se: 0,
                },
                color,
            );
        }
        if slot >= 24.0 || i % 2 == 0 {
            painter.text(
                pos2(x, plot.bottom() + 6.0),
                Align2::CENTER_TOP,
                h.to_string(),
                t.font(type_scale::MONEY_SM),
                t.p.fg_muted,
            );
        }
        let column = Rect::from_min_max(
            pos2(x - slot / 2.0, plot.top()),
            pos2(x + slot / 2.0, plot.bottom()),
        );
        if ui.rect_contains_pointer(column) {
            hovered = Some((h, column));
        }
    }
    if let Some((h, column)) = hovered {
        ui.painter()
            .rect_filled(column, t.radius(4), t.p.bg_hover.gamma_multiply(0.5));
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new("hour-tip"),
            egui::PopupAnchor::Pointer,
        )
        .show(|ui| {
            ui.label(t.text(format!("{h:02}:00–{h:02}:59"), type_scale::BODY_STRONG));
            ui.label(t.text(
                format!("{today_label} {}", format::baht(s.hourly[h])),
                type_scale::BODY,
            ));
            ui.label(t.muted(
                format!("{compare} {}", format::baht(c.hourly[h])),
                type_scale::BODY,
            ));
        });
    }
    let peak = (0..24).max_by_key(|h| (s.hourly[*h], std::cmp::Reverse(*h)));
    match peak.filter(|h| s.hourly[*h].satang() > 0) {
        Some(h) => w::help_text(
            ui,
            &format!(
                "{} {h:02}:00–{:02}:00 · {}",
                lang.text("ช่วงขายดีสุด", "Peak"),
                h + 1,
                format::baht(s.hourly[h])
            ),
        ),
        None => w::help_text(
            ui,
            lang.text("ยังไม่มียอดขายในวันนี้", "No sales on this day yet."),
        ),
    }
}
/// Daily bars for a multi-day period, against the previous period by position.
fn daily(app: &App, ui: &mut egui::Ui, s: &DailySummary, c: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let compare = compare_label(lang, app.report);
    ui.horizontal(|ui| {
        // The pane header names the chart; this row keeps the legend.
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            for (label, color) in [
                (compare.as_str(), t.p.chart_2),
                (lang.text("ช่วงนี้", "This period"), t.p.chart_1),
            ] {
                ui.label(t.muted(label, type_scale::CAPTION));
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, t.radius(2), color);
                ui.add_space(space::S2);
            }
        });
    });
    let n = s.days.len().max(1);
    let value =
        |list: &[pos_domain::DaySales], i: usize| list.get(i).map_or(0, |d| d.gross.satang());
    let max = (0..n)
        .map(|i| value(&s.days, i).max(value(&c.days, i)))
        .max()
        .unwrap_or(0);
    let step = nice_step(max.max(10_000));
    let top = ((max + step - 1) / step).max(1) * step;
    let height = ((ui.ctx().content_rect().height() - 520.0) * 0.6).clamp(140.0, 220.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let plot = Rect::from_min_max(
        pos2(rect.left() + 48.0, rect.top() + 8.0),
        pos2(rect.right(), rect.bottom() - 22.0),
    );
    let painter = ui.painter();
    let mut v = 0;
    while v <= top {
        let y = plot.bottom() - plot.height() * (v as f32 / top as f32);
        painter.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0, t.p.chart_grid),
        );
        painter.text(
            pos2(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            axis_label(v),
            t.font(type_scale::MONEY_SM),
            t.p.fg_muted,
        );
        v += step;
    }
    let slot = plot.width() / n as f32;
    let paired = slot >= 10.0;
    let bar = if paired {
        (slot * 0.34).clamp(2.0, 18.0)
    } else {
        (slot * 0.7).max(1.0)
    };
    let label_every = ((36.0 / slot).ceil() as usize).max(1);
    let mut hovered = None;
    for i in 0..n {
        let x = plot.left() + slot * (i as f32 + 0.5);
        let series: &[(i64, egui::Color32, f32)] = if paired {
            &[
                (value(&c.days, i), t.p.chart_2, -bar / 2.0 - 1.0),
                (value(&s.days, i), t.p.chart_1, bar / 2.0 + 1.0),
            ]
        } else {
            &[(value(&s.days, i), t.p.chart_1, 0.0)]
        };
        for (amount, color, dx) in series.iter().copied() {
            if amount <= 0 {
                continue;
            }
            let h = plot.height() * (amount as f32 / top as f32);
            let r = Rect::from_center_size(pos2(x + dx, plot.bottom() - h / 2.0), vec2(bar, h));
            painter.rect_filled(
                r,
                egui::CornerRadius {
                    nw: 3,
                    ne: 3,
                    sw: 0,
                    se: 0,
                },
                color,
            );
        }
        if i % label_every == 0
            && let Some(day) = s.days.get(i).and_then(|d| parse_date(&d.date))
        {
            painter.text(
                pos2(x, plot.bottom() + 6.0),
                Align2::CENTER_TOP,
                day.day().to_string(),
                t.font(type_scale::MONEY_SM),
                t.p.fg_muted,
            );
        }
        let column = Rect::from_min_max(
            pos2(x - slot / 2.0, plot.top()),
            pos2(x + slot / 2.0, plot.bottom()),
        );
        if ui.rect_contains_pointer(column) {
            hovered = Some((i, column));
        }
    }
    if let Some((i, column)) = hovered
        && let Some(day) = s.days.get(i)
    {
        ui.painter()
            .rect_filled(column, t.radius(4), t.p.bg_hover.gamma_multiply(0.5));
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new("day-tip"),
            egui::PopupAnchor::Pointer,
        )
        .show(|ui| {
            let date = parse_date(&day.date).map_or(day.date.clone(), |d| format::date(lang, d));
            ui.label(t.text(date, type_scale::BODY_STRONG));
            ui.label(t.text(
                format!("{} {}", lang.text("ขาย", "Sales"), format::baht(day.gross)),
                type_scale::BODY,
            ));
            if day.refunds.satang() > 0 {
                ui.label(t.text(
                    format!(
                        "{} {}",
                        lang.text("คืน", "Refunds"),
                        format::signed(format::negative(day.refunds))
                    ),
                    type_scale::BODY,
                ));
            }
            if let Some(prev) = c.days.get(i) {
                ui.label(t.muted(
                    format!("{compare} {}", format::baht(prev.gross)),
                    type_scale::BODY,
                ));
            }
        });
    }
    let best = s
        .days
        .iter()
        .filter(|d| d.gross.satang() > 0)
        .max_by_key(|d| d.gross);
    match best {
        Some(d) => w::help_text(
            ui,
            &format!(
                "{} {} · {}",
                lang.text("วันขายดีสุด", "Best day"),
                parse_date(&d.date).map_or(d.date.clone(), |x| format::date(lang, x)),
                format::baht(d.gross)
            ),
        ),
        None => w::help_text(
            ui,
            lang.text("ยังไม่มียอดขายในช่วงนี้", "No sales in this period yet."),
        ),
    }
}
/// Net sales per product category (the product's current category).
fn staff(app: &App, ui: &mut egui::Ui, s: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    if s.staff.is_empty() {
        w::state_view(
            ui,
            Icon::Reports,
            lang.text("ยังไม่มีการขายที่บันทึกชื่อพนักงาน", "No sales with a staff name yet"),
            lang.text(
                "บิลก่อนเริ่มใช้ระบบพนักงานไม่มีชื่อผู้ขาย",
                "Sales from before staff sign-in have no name.",
            ),
        );
        return;
    }
    for p in &s.staff {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 160.0).max(80.0));
                ui.spacing_mut().item_spacing.y = 0.0;
                w::name_label(ui, &p.name, type_scale::BODY_STRONG, t.p.fg_default);
                let mut sub = format!(
                    "{} {}",
                    format::count(p.sale_count),
                    lang.noun(p.sale_count, "บิล", "sale", "sales")
                );
                if p.refunds.satang() > 0 {
                    sub = format!(
                        "{sub} · {} {}",
                        lang.text("คืนเงิน", "refunds"),
                        format::money(p.refunds)
                    );
                }
                ui.label(t.muted(sub, type_scale::CAPTION));
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format::money(p.sales)).font(t.font(type_scale::MONEY)),
                );
            });
        });
        w::divider(ui);
    }
    w::help_text(
        ui,
        lang.text(
            "ยอดขายนับตามผู้รับเงิน การคืนเงินนับตามผู้ทำรายการคืน",
            "Sales count for who took payment; refunds for who processed them.",
        ),
    );
}
/// Bills and amounts per promotion; voids take bills back out.
fn promotions(app: &App, ui: &mut egui::Ui, s: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    if s.promotions.is_empty() {
        w::state_view(
            ui,
            Icon::Tag,
            lang.text(
                "ยังไม่มีบิลที่ใช้โปรในช่วงนี้",
                "No bills used a promotion in this period",
            ),
            lang.text(
                "ตั้งโปรได้ที่ ตั้งค่า → โปรโมชัน",
                "Set promotions in Settings → Promotions.",
            ),
        );
        return;
    }
    let mut total = pos_domain::SignedMoney::default();
    for p in &s.promotions {
        total = total.checked_add(p.amount).unwrap_or(total);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 160.0).max(80.0));
                ui.spacing_mut().item_spacing.y = 0.0;
                w::name_label(ui, &p.name, type_scale::BODY_STRONG, t.p.fg_default);
                ui.label(t.muted(
                    format!(
                        "{} {}",
                        format::signed_count(p.bills),
                        lang.text("บิล", if p.bills == 1 { "bill" } else { "bills" })
                    ),
                    type_scale::CAPTION,
                ));
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format::signed(p.amount)).font(t.font(type_scale::MONEY)),
                );
            });
        });
        w::divider(ui);
    }
    w::kv(
        ui,
        lang.text("ลดไปทั้งหมด", "Total given"),
        &format::signed(total),
        w::Value::Strong,
    );
    w::help_text(
        ui,
        lang.text(
            "นับในวันที่ขาย ยกเลิกทั้งบิลหักออก คืนบางรายการไม่หัก ยอดนี้รวมอยู่ในส่วนลดของรายงานแล้ว",
            "Counted on the sale day; whole voids subtract, partial refunds do not. Already included in the report's discounts.",
        ),
    );
}
/// Ingredients used by sales (net of voids) and removed by hand.
fn ingredients(app: &App, ui: &mut egui::Ui, s: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    if s.ingredients.is_empty() {
        w::state_view(
            ui,
            Icon::Products,
            lang.text("ยังไม่มีการใช้วัตถุดิบในช่วงนี้", "No ingredient use in this period"),
            lang.text(
                "ใส่สูตรให้สินค้าในหน้าแก้ไขสินค้า แล้วยอดใช้จะขึ้นที่นี่",
                "Give products a recipe in the product editor; use shows up here.",
            ),
        );
        return;
    }
    let mut cost = Some(Money::ZERO);
    for u in &s.ingredients {
        cost = match (cost, u.cost) {
            (Some(sum), Some(c)) => sum.checked_add(c).ok(),
            (sum, None) if u.used <= 0 => sum,
            _ => None,
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 180.0).max(80.0));
                ui.spacing_mut().item_spacing.y = 0.0;
                w::name_label(ui, &u.name, type_scale::BODY_STRONG, t.p.fg_default);
                let mut sub = Vec::new();
                if u.removed != 0 {
                    sub.push(format!(
                        "{} {} {}",
                        lang.text("นำออก", "removed"),
                        format::qty(u.removed),
                        u.unit
                    ));
                }
                if let Some(c) = u.cost {
                    sub.push(format!(
                        "{} {}",
                        lang.text("ต้นทุน", "cost"),
                        format::money(c)
                    ));
                }
                if !sub.is_empty() {
                    ui.label(t.muted(sub.join(" · "), type_scale::CAPTION));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(t.muted(&u.unit, type_scale::LABEL));
                ui.label(egui::RichText::new(format::qty(u.used)).font(t.font(type_scale::MONEY)));
            });
        });
        w::divider(ui);
    }
    if let Some(cost) = cost {
        w::kv(
            ui,
            lang.text("ต้นทุนวัตถุดิบที่ขายไป", "Cost of ingredients sold"),
            &format::money(cost),
            w::Value::Strong,
        );
    }
    w::help_text(
        ui,
        lang.text(
            "ใช้ = ตัดตามสูตรตอนขาย หักที่ยกเลิกทั้งบิล · นำออก = เสีย/ทิ้ง/พนักงานทาน ที่บันทึกเอง · ต้นทุนคิดจากต้นทุนต่อหน่วยปัจจุบัน",
            "Used = deducted by recipes at sale, less whole voids · removed = waste and staff meals recorded by hand · cost uses today's cost per unit.",
        ),
    );
}
fn categories(app: &App, ui: &mut egui::Ui, s: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    if s.categories.is_empty() {
        w::state_view(
            ui,
            Icon::Reports,
            lang.text("ยังไม่มีรายการในช่วงนี้", "No activity in this period"),
            "",
        );
        return;
    }
    let max = s
        .categories
        .iter()
        .map(|c| c.net_sales.satang().max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let width = ui.available_width();
    for c in &s.categories {
        let name = if c.category.is_empty() {
            lang.text("ไม่มีหมวด", "Uncategorised").to_owned()
        } else {
            c.category.clone()
        };
        let name_w = width - 200.0;
        let (g, _) = crate::ui::text::wrapped(
            ui.painter(),
            &name,
            t.font(type_scale::BODY),
            t.p.fg_default,
            name_w,
            0,
        );
        let gh = g.size().y;
        let (rect, _) =
            ui.allocate_exact_size(vec2(width, (gh + 22.0).max(t.m.row_height)), Sense::hover());
        let cy = rect.top() + 10.0 + gh / 2.0;
        ui.painter()
            .galley(pos2(rect.left(), rect.top() + 10.0), g, t.p.fg_default);
        let bar_w = name_w * (c.net_sales.satang().max(0) as f32 / max as f32);
        if bar_w > 0.0 {
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(rect.left(), rect.top() + 12.0 + gh), vec2(bar_w, 5.0)),
                3,
                t.p.chart_1,
            );
        }
        ui.painter().text(
            pos2(rect.right() - 120.0, cy),
            Align2::RIGHT_CENTER,
            format::signed_count(c.net_quantity),
            t.font(type_scale::MONEY),
            t.p.fg_default,
        );
        ui.painter().text(
            pos2(rect.right() - space::S2, cy),
            Align2::RIGHT_CENTER,
            format::signed(c.net_sales),
            t.font(type_scale::MONEY),
            if c.net_sales.satang() < 0 {
                t.p.danger_fg
            } else {
                t.p.fg_default
            },
        );
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, t.p.border_subtle),
        );
    }
    w::help_text(
        ui,
        lang.text(
            "จัดตามหมวดปัจจุบันของสินค้า · ชิ้นสุทธิและยอดสุทธิหลังคืน",
            "Grouped by each product's current category · net units and sales after refunds",
        ),
    );
}
fn tenders(app: &App, ui: &mut egui::Ui, s: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    // Share of each method as one stacked bar (positive net only).
    let parts: Vec<(i64, egui::Color32)> = [
        (PaymentMethod::Cash, t.p.chart_1),
        (PaymentMethod::Transfer, t.p.chart_3),
        (PaymentMethod::Card, t.p.chart_2),
    ]
    .iter()
    .map(|(m, c)| (s.payments.method(*m).net().satang().max(0), *c))
    .collect();
    let sum: i64 = parts.iter().map(|(v, _)| v).sum();
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), egui::Sense::hover());
    ui.painter().rect_filled(bar, t.radius(4), t.p.bg_sunken);
    if sum > 0 {
        let mut x = bar.left();
        for (v, c) in &parts {
            let w_ = bar.width() * *v as f32 / sum as f32;
            if w_ > 0.5 {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x, bar.top()),
                        vec2(w_ - 2.0, bar.height()),
                    ),
                    t.radius(3),
                    *c,
                );
            }
            x += w_;
        }
    }
    ui.add_space(space::S1);
    let color_of = |m: PaymentMethod| match m {
        PaymentMethod::Cash => t.p.chart_1,
        PaymentMethod::Transfer => t.p.chart_3,
        PaymentMethod::Card => t.p.chart_2,
    };
    let mut total = SignedMoney::default();
    for (method, icon) in [
        (PaymentMethod::Cash, Icon::Banknote),
        (PaymentMethod::Transfer, Icon::Transfer),
        (PaymentMethod::Card, Icon::Card),
    ] {
        let m = s.payments.method(method);
        total = total.checked_add(m.net()).unwrap_or(total);
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(dot, 2, color_of(method));
            let _ = icon;
            ui.label(t.text(lang.payment_method(method), type_scale::BODY));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format::signed(m.net()))
                        .font(t.font(type_scale::MONEY))
                        .color(if m.net().satang() < 0 {
                            t.p.danger_fg
                        } else {
                            t.p.fg_default
                        }),
                );
                if m.refunds.satang() > 0 {
                    ui.label(t.muted(
                        format!(
                            "{} {} · {} {}",
                            lang.text("ขาย", "sales"),
                            format::money(m.sales),
                            lang.text("คืน", "refunds"),
                            format::signed(format::negative(m.refunds))
                        ),
                        type_scale::CAPTION,
                    ));
                }
            });
        });
    }
    w::dashed(ui);
    w::kv(
        ui,
        lang.text("รวมรายรับสุทธิ", "Net receipts"),
        &format::signed(total),
        w::Value::Strong,
    );
    w::help_text(
        ui,
        lang.text(
            "คืนเงินนับในวันที่ยกเลิกบิล · ไม่รวมเงินตั้งต้นและเงินเข้าออกลิ้นชัก",
            "Refunds count on the cancellation day · excludes opening float and drawer movements",
        ),
    );
    if s.vat_sales.satang() > 0 || s.vat_refunds.satang() > 0 {
        let net =
            pos_domain::SignedMoney::from_satang(s.vat_sales.satang() - s.vat_refunds.satang());
        w::help_text(
            ui,
            &format!(
                "{} {} · {} {} · {} {}",
                lang.text("VAT ในยอดขาย", "VAT in sales"),
                format::money(s.vat_sales),
                lang.text("ในยอดคืน", "in refunds"),
                format::money(s.vat_refunds),
                lang.text("สุทธิ", "net"),
                format::signed(net)
            ),
        );
    }
    if s.service_charges.satang() > 0 || s.rounding.satang() != 0 {
        w::help_text(
            ui,
            &format!(
                "Service charge {} · {} {}",
                format::money(s.service_charges),
                lang.text("ปัดเศษเงินสด", "cash rounding"),
                format::signed(s.rounding)
            ),
        );
    }
    if s.discounts.satang() > 0 {
        w::help_text(
            ui,
            &format!(
                "{} {} · {}",
                lang.text("ให้ส่วนลด", "Discounts given"),
                format::money(s.discounts),
                lang.text("ยอดขายเป็นยอดหลังหักส่วนลดแล้ว", "sales are after discounts")
            ),
        );
    }
    if s.cash_received.satang() > 0 {
        w::help_text(
            ui,
            &format!(
                "{} {} · {} {}",
                lang.text("รับเงินสดก่อนทอน", "Cash tendered"),
                format::money(s.cash_received),
                lang.text("ทอน", "change"),
                format::money(s.change_given)
            ),
        );
    }
}
fn top_products(app: &mut App, ui: &mut egui::Ui, s: &DailySummary) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.horizontal(|ui| {
        // The pane tab names the list; this row keeps the sort switch.
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            w::segmented(
                ui,
                &mut app.report_metric,
                &[
                    (Metric::Amount, lang.text("ยอดเงิน", "Amount")),
                    (Metric::Quantity, lang.text("จำนวน", "Qty")),
                ],
            );
        });
    });
    let mut list = s.top_products.clone();
    if app.report_metric == Metric::Amount {
        list.sort_by(|a, b| {
            b.net_sales
                .cmp(&a.net_sales)
                .then(b.net_quantity.cmp(&a.net_quantity))
        });
    }
    if list.is_empty() {
        w::state_view(
            ui,
            Icon::Reports,
            lang.text("ยังไม่มีรายการในวันนี้", "No activity on this day"),
            lang.text("เลือกวันอื่นเพื่อดูยอดขาย", "Pick another date to see sales."),
        );
        return;
    }
    let width = ui.available_width();
    let (head, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    ui.painter().line_segment(
        [head.left_bottom(), head.right_bottom()],
        Stroke::new(1.0, t.p.border_default),
    );
    let qty_x = head.right() - 120.0;
    ui.painter().text(
        pos2(head.left() + 40.0, head.center().y),
        Align2::LEFT_CENTER,
        lang.text("สินค้า", "Product"),
        t.strong_font(type_scale::CAPTION),
        t.p.fg_muted,
    );
    ui.painter().text(
        pos2(qty_x, head.center().y),
        Align2::RIGHT_CENTER,
        lang.text("ชิ้น", "Qty"),
        t.strong_font(type_scale::CAPTION),
        t.p.fg_muted,
    );
    ui.painter().text(
        pos2(head.right() - space::S2, head.center().y),
        Align2::RIGHT_CENTER,
        lang.text("ยอดสุทธิ", "Net"),
        t.strong_font(type_scale::CAPTION),
        t.p.fg_muted,
    );
    let max = list
        .iter()
        .map(|p| match app.report_metric {
            Metric::Amount => p.net_sales.satang().max(0),
            Metric::Quantity => p.net_quantity.max(0),
        })
        .max()
        .unwrap_or(1)
        .max(1);
    for (i, p) in list.iter().enumerate() {
        let name_w = qty_x - head.left() - 40.0 - 48.0;
        let (g, _) = crate::ui::text::wrapped(
            ui.painter(),
            &p.name,
            t.font(type_scale::BODY),
            t.p.fg_default,
            name_w,
            0,
        );
        let h = g.size().y + 22.0;
        let (rect, response) =
            ui.allocate_exact_size(vec2(width, h.max(t.m.row_height)), Sense::hover());
        let cy = rect.top() + 10.0 + g.size().y / 2.0;
        ui.painter().text(
            pos2(rect.left() + 16.0, cy),
            Align2::CENTER_CENTER,
            (i + 1).to_string(),
            t.font(type_scale::MONEY_SM),
            t.p.fg_muted,
        );
        let gh = g.size().y;
        ui.painter().galley(
            pos2(rect.left() + 40.0, rect.top() + 10.0),
            g,
            t.p.fg_default,
        );
        let value = match app.report_metric {
            Metric::Amount => p.net_sales.satang().max(0),
            Metric::Quantity => p.net_quantity.max(0),
        };
        let bar_w = name_w * (value as f32 / max as f32);
        if bar_w > 0.0 {
            ui.painter().rect_filled(
                Rect::from_min_size(
                    pos2(rect.left() + 40.0, rect.top() + 12.0 + gh),
                    vec2(bar_w, 5.0),
                ),
                3,
                t.p.chart_1,
            );
        }
        ui.painter().text(
            pos2(qty_x, cy),
            Align2::RIGHT_CENTER,
            format::signed_count(p.net_quantity),
            t.font(type_scale::MONEY),
            t.p.fg_default,
        );
        ui.painter().text(
            pos2(rect.right() - space::S2, cy),
            Align2::RIGHT_CENTER,
            format::signed(p.net_sales),
            t.font(type_scale::MONEY),
            if p.net_sales.satang() < 0 {
                t.p.danger_fg
            } else {
                t.p.fg_default
            },
        );
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, t.p.border_subtle),
        );
        response.on_hover_text(format!("{} · {}", p.sku, p.name));
    }
    if list.iter().any(|p| p.net_quantity < 0) {
        w::help_text(
            ui,
            lang.text(
                "จำนวนติดลบ หมายถึงคืนมากกว่าขายในวันนี้",
                "Negative units mean more returns than sales on this day.",
            ),
        );
    }
}

fn profit_body(ui: &mut egui::Ui, lang: Language, p: &pos_domain::ProfitSummary) {
    let t = Theme::of(ui);
    egui::CollapsingHeader::new(lang.text("กำไรขั้นต้นโดยประมาณ", "Estimated gross profit"))
        .id_salt("gross-profit")
        .default_open(true)
        .show(ui, |ui| {
            w::help_text(ui, lang.text(
                "เฉพาะรายการที่บันทึกต้นทุนตอนขาย · หัก VAT และยอดคืนตามวันคืนเงิน · รวมค่าบริการ แต่ยังไม่หักค่าใช้จ่ายร้าน",
                "Only lines with a cost at payment; excludes VAT, reverses on refund day. Includes service charges, before shop expenses.",
            ));
            let rows = [
                (lang.text("รายได้สุทธิที่ทราบต้นทุน", "Covered net revenue"), p.covered_revenue),
                (lang.text("ต้นทุนขายสุทธิ", "Net cost of goods"), p.cost_of_goods),
                (lang.text("กำไรขั้นต้นโดยประมาณ", "Estimated gross profit"), p.gross_profit),
                (lang.text("รายได้ที่ยังไม่ทราบต้นทุน", "Unknown-cost revenue"), p.unknown_revenue),
            ];
            for (label, amount) in rows {
                ui.horizontal(|ui| {
                    ui.label(t.muted(label, type_scale::LABEL));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format::signed(amount)).font(t.font(type_scale::MONEY)).color(if amount.satang() < 0 { t.p.danger_fg } else { t.p.fg_default }));
                    });
                });
            }
            w::help_text(ui, &format!("{}: {} / {} · {}: {} / {}", lang.text("ชิ้นขาย ทราบ/ไม่ทราบต้นทุน", "Sold units known/unknown"), p.known_sold_units, p.unknown_sold_units, lang.text("ชิ้นคืน ทราบ/ไม่ทราบต้นทุน", "Refunded units known/unknown"), p.known_refunded_units, p.unknown_refunded_units));
        });
    w::divider(ui);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn change_is_integer_tenths_of_a_percent() -> Result<(), pos_domain::DomainError> {
        assert_eq!(
            change_tenths(Money::from_satang(1_124)?, Money::from_satang(1_000)?),
            Some(124)
        );
        assert_eq!(
            change_tenths(Money::from_satang(900)?, Money::from_satang(1_000)?),
            Some(-100)
        );
        assert_eq!(change_tenths(Money::from_satang(900)?, Money::ZERO), None);
        Ok(())
    }
    #[test]
    fn comparison_period_is_last_week_for_a_day_else_the_period_before() {
        use time::macros::date;
        let day = Period::day(date!(2026 - 10 - 13));
        assert_eq!(day.previous(), Some(Period::day(date!(2026 - 10 - 06))));
        let week = Period {
            start: date!(2026 - 10 - 07),
            end: date!(2026 - 10 - 13),
        };
        assert_eq!(week.days(), 7);
        assert_eq!(
            week.previous(),
            Some(Period {
                start: date!(2026 - 09 - 30),
                end: date!(2026 - 10 - 06),
            })
        );
    }
    #[test]
    fn axis_steps_are_round() {
        assert_eq!(nice_step(300_000), 100_000);
        assert_eq!(nice_step(1_000_000), 500_000);
        assert!(nice_step(i64::MAX / 2) > 0);
    }
}
