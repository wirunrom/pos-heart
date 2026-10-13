//! Promotions: the Settings section that lists and edits them, and the
//! coupon dialog on a bill. Promotions apply automatically while pricing
//! (`App::promoted`), exactly as checkout applies them.
use crate::{
    app::App,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align};
use pos_domain::{ALL_DAYS, Discount, Money, PromoRule, PromoTarget, Promotion};
use uuid::Uuid;

#[derive(Default)]
pub struct PromoState {
    pub editor: Option<Editor>,
    /// Coupon code being typed on the open bill.
    pub coupon: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Percent,
    Baht,
    BuyGet,
    Bill,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    All,
    Category,
    Product,
}
/// Text fields of a promotion being edited.
#[derive(Clone)]
pub struct Editor {
    promo: Promotion,
    kind: Kind,
    value: String,
    buy: String,
    free: String,
    min_spend: String,
    aim: Aim,
    category: String,
    product: Option<Uuid>,
    search: String,
    coupon: bool,
    max_uses: String,
    hours: bool,
    from: String,
    to: String,
    delete: bool,
}
fn hhmm(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}
fn parse_hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h <= 24 && m < 60 && h * 60 + m <= 1440).then_some(h * 60 + m)
}
fn percent_text(bp: u32) -> String {
    let s = format!("{}.{:02}", bp / 100, bp % 100);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
fn parse_percent(s: &str) -> Option<u32> {
    let m = format::parse_money(s).ok()?;
    u32::try_from(m.satang())
        .ok()
        .filter(|bp| (1..=10_000).contains(bp))
}
impl Editor {
    pub fn new(promo: Promotion) -> Self {
        let (kind, value, buy, free, min_spend) = match promo.rule {
            PromoRule::Off {
                discount: Discount::Percent(bp),
            } => (
                Kind::Percent,
                percent_text(bp),
                String::new(),
                String::new(),
                String::new(),
            ),
            PromoRule::Off {
                discount: Discount::Amount(m),
            } => (
                Kind::Baht,
                format::money(m),
                String::new(),
                String::new(),
                String::new(),
            ),
            PromoRule::BuyGet { buy, free } => (
                Kind::BuyGet,
                String::new(),
                buy.to_string(),
                free.to_string(),
                String::new(),
            ),
            PromoRule::Bill {
                min_spend,
                discount,
            } => (
                Kind::Bill,
                // A bill promotion's percent keeps its % sign in the field.
                match discount {
                    Discount::Percent(bp) => format!("{}%", percent_text(bp)),
                    Discount::Amount(m) => format::money(m),
                },
                String::new(),
                String::new(),
                format::money(min_spend),
            ),
        };
        let (aim, category, product) = match &promo.target {
            PromoTarget::All => (Aim::All, String::new(), None),
            PromoTarget::Category(c) => (Aim::Category, c.clone(), None),
            PromoTarget::Product(id) => (Aim::Product, String::new(), Some(*id)),
        };
        Self {
            kind,
            value,
            buy,
            free,
            min_spend,
            aim,
            category,
            product,
            search: String::new(),
            coupon: promo.is_coupon(),
            max_uses: promo.max_uses.map(|n| n.to_string()).unwrap_or_default(),
            hours: promo.from_minute != promo.to_minute,
            from: hhmm(promo.from_minute),
            to: hhmm(promo.to_minute),
            delete: false,
            promo,
        }
    }
    /// The promotion as typed, or None while a field is invalid. A bill
    /// promotion's percent/baht follows `bill_percent`.
    fn build(&self, bill_percent: bool) -> Option<Promotion> {
        let mut p = self.promo.clone();
        p.rule = match self.kind {
            Kind::Percent => PromoRule::Off {
                discount: Discount::Percent(parse_percent(&self.value)?),
            },
            Kind::Baht => PromoRule::Off {
                discount: Discount::Amount(
                    format::parse_money(&self.value)
                        .ok()
                        .filter(|m| *m > Money::ZERO)?,
                ),
            },
            Kind::BuyGet => PromoRule::BuyGet {
                buy: self.buy.trim().parse().ok()?,
                free: self.free.trim().parse().ok()?,
            },
            Kind::Bill => PromoRule::Bill {
                min_spend: format::parse_money(&self.min_spend).ok()?,
                discount: if bill_percent {
                    Discount::Percent(parse_percent(&self.value)?)
                } else {
                    Discount::Amount(
                        format::parse_money(&self.value)
                            .ok()
                            .filter(|m| *m > Money::ZERO)?,
                    )
                },
            },
        };
        p.target = match (self.kind, self.aim) {
            (Kind::Bill, _) | (_, Aim::All) => PromoTarget::All,
            (_, Aim::Category) => PromoTarget::Category(self.category.trim().to_owned()),
            (_, Aim::Product) => PromoTarget::Product(self.product?),
        };
        if !self.coupon {
            p.coupon.clear();
        }
        p.max_uses = if self.max_uses.trim().is_empty() {
            None
        } else {
            Some(self.max_uses.trim().parse().ok().filter(|n| *n > 0)?)
        };
        if self.hours {
            p.from_minute = parse_hhmm(&self.from)?;
            p.to_minute = parse_hhmm(&self.to)?;
        } else {
            p.from_minute = 0;
            p.to_minute = 0;
        }
        p.starts = p.starts.trim().to_owned();
        p.ends = p.ends.trim().to_owned();
        p.validate().ok()?;
        Some(p)
    }
}
/// "ลด 10% · เครื่องดื่ม · 17:00–19:00 · คูปอง WELCOME"
pub fn summary(app: &App, p: &Promotion) -> String {
    let lang = app.lang;
    let off = |d: Discount| match d {
        Discount::Percent(bp) => format!("{}%", percent_text(bp)),
        Discount::Amount(m) => format::baht(m),
    };
    let mut parts = vec![match p.rule {
        PromoRule::Off {
            discount: d @ Discount::Percent(_),
        } => format!("{} {}", lang.text("ลด", "Off"), off(d)),
        PromoRule::Off { discount } => {
            format!(
                "{} {} {}",
                lang.text("ลด", "Off"),
                off(discount),
                lang.text("ต่อชิ้น", "each")
            )
        }
        PromoRule::BuyGet { buy, free } => {
            format!(
                "{} {buy} {} {free}",
                lang.text("ซื้อ", "Buy"),
                lang.text("แถม", "get free")
            )
        }
        PromoRule::Bill {
            min_spend,
            discount,
        } => format!(
            "{} {} {} {}",
            lang.text("ครบ", "Spend"),
            format::baht(min_spend),
            lang.text("ลด", "get"),
            off(discount)
        ),
    }];
    match &p.target {
        PromoTarget::All => {}
        PromoTarget::Category(c) => parts.push(c.clone()),
        PromoTarget::Product(id) => parts.push(
            app.known
                .get(id)
                .or_else(|| app.data.products.iter().find(|x| x.id == *id))
                .map_or_else(
                    || lang.text("สินค้าที่ลบแล้ว", "removed product").to_owned(),
                    |x| x.name.clone(),
                ),
        ),
    }
    if p.days != ALL_DAYS {
        let names = lang.text("จ อ พ พฤ ศ ส อา", "Mo Tu We Th Fr Sa Su");
        let days: Vec<&str> = names
            .split(' ')
            .enumerate()
            .filter(|(i, _)| p.days & (1 << i) != 0)
            .map(|(_, d)| d)
            .collect();
        parts.push(days.join(" "));
    }
    if p.from_minute != p.to_minute {
        parts.push(format!("{}–{}", hhmm(p.from_minute), hhmm(p.to_minute)));
    }
    if !p.starts.is_empty() || !p.ends.is_empty() {
        parts.push(format!("{} – {}", p.starts, p.ends));
    }
    if p.is_coupon() {
        parts.push(format!("{} {}", lang.text("คูปอง", "coupon"), p.coupon));
    }
    match p.max_uses {
        Some(max) => parts.push(format!("{} {}/{max}", lang.text("ใช้แล้ว", "used"), p.uses)),
        None if p.uses > 0 => parts.push(format!(
            "{} {} {}",
            lang.text("ใช้แล้ว", "used on"),
            p.uses,
            lang.text("บิล", "bills")
        )),
        None => {}
    }
    parts.join(" · ")
}

/// Settings → Promotions.
pub fn settings_card(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    w::title(ui, lang.text("โปรโมชันและคูปอง", "Promotions & coupons"));
    w::help_text(
        ui,
        lang.text(
            "คิดให้อัตโนมัติตอนขายตามวันและเวลาที่ตั้ง แต่ละรายการได้โปรที่คุ้มที่สุดหนึ่งโปร ส่วนลดที่พนักงานใส่เองมาก่อนโปรเสมอ โปรคูปองใช้เมื่อใส่รหัสในบิล",
            "Applied automatically while selling, by day and time. Each line gets its single best promotion; a cashier's own discount always comes first. Coupon promotions need their code on the bill.",
        ),
    );
    let may = crate::staff_ui::may(app, pos_domain::Permission::Settings);
    let promos = app.data.promotions.clone();
    if promos.is_empty() {
        ui.label(t.muted(
            lang.text("ยังไม่มีโปรโมชัน", "No promotions yet"),
            type_scale::BODY,
        ));
    }
    for p in &promos {
        ui.horizontal(|ui| {
            let text_w = (ui.available_width() - 90.0).max(160.0);
            ui.allocate_ui_with_layout(
                egui::vec2(text_w, 0.0),
                egui::Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(text_w);
                    ui.horizontal(|ui| {
                        w::name_label(ui, &p.name, type_scale::BODY_STRONG, t.p.fg_default);
                        if !p.active {
                            w::badge(ui, lang.text("ปิดอยู่", "Off"), w::Tone::Neutral);
                        } else if p.max_uses.is_some_and(|m| p.uses >= m) {
                            w::badge(ui, lang.text("ใช้ครบแล้ว", "Used up"), w::Tone::Warning);
                        }
                    });
                    ui.add(egui::Label::new(t.muted(summary(app, p), type_scale::CAPTION)).wrap());
                },
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if may
                    && w::button(lang.text("แก้ไข", "Edit"))
                        .ghost()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                {
                    app.error = None;
                    app.promotions.editor = Some(Editor::new(p.clone()));
                }
            });
        });
        w::divider(ui);
    }
    if may
        && w::button(lang.text("เพิ่มโปรโมชัน", "Add promotion"))
            .icon(Icon::Plus)
            .enabled(app.ready && !app.busy)
            .show(ui)
            .clicked()
    {
        app.error = None;
        app.promotions.editor = Some(Editor::new(Promotion::new(
            "",
            PromoRule::Off {
                discount: Discount::Percent(1000),
            },
            PromoTarget::All,
        )));
    }
}

pub fn editor_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut e) = app.promotions.editor.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let exists = app.data.promotions.iter().any(|p| p.id == e.promo.id);
    // A bill promotion's value is a percent when it ends with %.
    let bill_percent = e.value.trim().ends_with('%');
    let built = Editor {
        value: e.value.trim().trim_end_matches('%').trim().to_owned(),
        ..e.clone()
    }
    .build(bill_percent);
    let response = w::dialog(ctx, "promo-editor", 560.0, |ui| {
        w::dialog_title(
            ui,
            if exists {
                lang.text("แก้ไขโปรโมชัน", "Edit promotion")
            } else {
                lang.text("เพิ่มโปรโมชัน", "Add promotion")
            },
            false,
        );
        let mut valid = true;
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 220.0).clamp(240.0, 620.0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = space::S2;
                w::field_label(ui, lang.text("ชื่อโปร", "Name"), Some(lang.text("แสดงบนใบเสร็จ", "printed on receipts")));
                w::text_input(
                    ui,
                    &mut e.promo.name,
                    w::FieldOpts {
                        hint: lang.text("เช่น ชาไทย 2 แถม 1", "e.g. Thai tea buy 2 get 1"),
                        limit: 60,
                        ..Default::default()
                    },
                );
                w::field_label(ui, lang.text("แบบโปร", "Type"), None);
                w::segmented(
                    ui,
                    &mut e.kind,
                    &[
                        (Kind::Percent, lang.text("ลด %", "% off")),
                        (Kind::Baht, lang.text("ลดบาท/ชิ้น", "฿ off each")),
                        (Kind::BuyGet, lang.text("ซื้อ X แถม Y", "Buy X get Y")),
                        (Kind::Bill, lang.text("ครบยอดลด", "Spend & save")),
                    ],
                );
                match e.kind {
                    Kind::Percent | Kind::Baht => {
                        ui.horizontal(|ui| {
                            ui.label(t.text(
                                if e.kind == Kind::Percent {
                                    lang.text("ลดกี่ %", "Percent off")
                                } else {
                                    lang.text("ลดชิ้นละ (บาท)", "Baht off each")
                                },
                                type_scale::BODY,
                            ));
                            w::text_input(
                                ui,
                                &mut e.value,
                                w::FieldOpts {
                                    hint: if e.kind == Kind::Percent { "10" } else { "5.00" },
                                    limit: 10,
                                    width: Some(110.0),
                                    ..Default::default()
                                },
                            );
                        });
                    }
                    Kind::BuyGet => {
                        ui.horizontal(|ui| {
                            ui.label(t.text(lang.text("ซื้อ", "Buy"), type_scale::BODY));
                            w::text_input(ui, &mut e.buy, w::FieldOpts { hint: "2", limit: 2, width: Some(60.0), ..Default::default() });
                            ui.label(t.text(lang.text("แถม", "get free"), type_scale::BODY));
                            w::text_input(ui, &mut e.free, w::FieldOpts { hint: "1", limit: 2, width: Some(60.0), ..Default::default() });
                        });
                        w::help_text(ui, lang.text("นับในบรรทัดเดียวกัน (สินค้าและตัวเลือกเดียวกัน) ของแถมคือชิ้นราคาเดียวกัน", "Counted within one bill line (same product and options); the free units are of that line."));
                    }
                    Kind::Bill => {
                        ui.horizontal(|ui| {
                            ui.label(t.text(lang.text("เมื่อซื้อครบ (บาท)", "When the bill reaches"), type_scale::BODY));
                            w::text_input(ui, &mut e.min_spend, w::FieldOpts { hint: "500.00", limit: 12, width: Some(110.0), ..Default::default() });
                        });
                        ui.horizontal(|ui| {
                            ui.label(t.text(lang.text("ลด (บาท หรือใส่ % ต่อท้าย)", "Take off (baht, or end with %)"), type_scale::BODY));
                            w::text_input(ui, &mut e.value, w::FieldOpts { hint: "50 / 10%", limit: 10, width: Some(110.0), ..Default::default() });
                        });
                        w::help_text(ui, lang.text("ใช้กับบิลที่ไม่มีส่วนลดทั้งบิลจากพนักงาน คิดจากยอดหลังส่วนลดรายการ", "Applies when the cashier gave no bill discount, judged after line discounts."));
                    }
                }
                if e.kind != Kind::Bill {
                    w::field_label(ui, lang.text("ใช้กับ", "Applies to"), None);
                    w::segmented(
                        ui,
                        &mut e.aim,
                        &[
                            (Aim::All, lang.text("ทุกสินค้า", "Everything")),
                            (Aim::Category, lang.text("หมวดหมู่", "Category")),
                            (Aim::Product, lang.text("สินค้า", "Product")),
                        ],
                    );
                    match e.aim {
                        Aim::All => {}
                        Aim::Category => {
                            let mut categories: Vec<String> = app.data.counts.categories.iter().map(|(c, _)| c.clone()).filter(|c| !c.is_empty()).collect();
                            categories.dedup();
                            ui.horizontal_wrapped(|ui| {
                                for c in categories {
                                    if w::chip(ui, &c, e.category == c, false).clicked() {
                                        e.category = c;
                                    }
                                }
                            });
                            if e.category.is_empty() {
                                valid = false;
                            }
                        }
                        Aim::Product => {
                            w::text_input(
                                ui,
                                &mut e.search,
                                w::FieldOpts {
                                    hint: lang.text("ค้นหาสินค้า", "Search products"),
                                    limit: 60,
                                    ..Default::default()
                                },
                            );
                            let q = e.search.trim().to_lowercase();
                            let matches: Vec<(Uuid, String)> = app
                                .data
                                .products
                                .iter()
                                .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q) || p.sku.to_lowercase().contains(&q))
                                .take(8)
                                .map(|p| (p.id, p.name.clone()))
                                .collect();
                            ui.horizontal_wrapped(|ui| {
                                for (id, name) in matches {
                                    if w::chip(ui, &name, e.product == Some(id), false).clicked() {
                                        e.product = Some(id);
                                    }
                                }
                            });
                            if e.product.is_none() {
                                valid = false;
                            }
                        }
                    }
                }
                w::field_label(ui, lang.text("วันที่ใช้ได้", "Days"), None);
                ui.horizontal_wrapped(|ui| {
                    let names = lang.text("จ อ พ พฤ ศ ส อา", "Mo Tu We Th Fr Sa Su");
                    for (i, d) in names.split(' ').enumerate() {
                        let on = e.promo.days & (1 << i) != 0;
                        if w::chip(ui, d, on, false).clicked() {
                            e.promo.days ^= 1 << i;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    w::switch(ui, &mut e.hours, lang.text("เฉพาะช่วงเวลา", "Only at certain hours"));
                    ui.label(t.text(lang.text("เฉพาะช่วงเวลา", "Only at certain hours"), type_scale::BODY));
                    if e.hours {
                        let (bad_from, bad_to) = (parse_hhmm(&e.from).is_none(), parse_hhmm(&e.to).is_none());
                        w::text_input(ui, &mut e.from, w::FieldOpts { hint: "17:00", limit: 5, width: Some(72.0), invalid: bad_from, ..Default::default() });
                        ui.label("–");
                        w::text_input(ui, &mut e.to, w::FieldOpts { hint: "19:00", limit: 5, width: Some(72.0), invalid: bad_to, ..Default::default() });
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t.text(lang.text("ตั้งแต่", "From"), type_scale::BODY));
                    w::text_input(ui, &mut e.promo.starts, w::FieldOpts { hint: lang.text("ปปปป-ดด-วว", "YYYY-MM-DD"), limit: 10, width: Some(120.0), ..Default::default() });
                    ui.label(t.text(lang.text("ถึง", "to"), type_scale::BODY));
                    w::text_input(ui, &mut e.promo.ends, w::FieldOpts { hint: lang.text("ปปปป-ดด-วว", "YYYY-MM-DD"), limit: 10, width: Some(120.0), ..Default::default() });
                });
                w::help_text(ui, lang.text("วันที่แบบ ปี-เดือน-วัน (ค.ศ.) เว้นว่าง = ไม่จำกัด", "Dates as YYYY-MM-DD; empty means open-ended."));
                ui.horizontal(|ui| {
                    w::switch(ui, &mut e.coupon, lang.text("ต้องใช้คูปอง", "Needs a coupon"));
                    ui.label(t.text(lang.text("ต้องใช้คูปอง", "Needs a coupon"), type_scale::BODY));
                    if e.coupon {
                        w::text_input(ui, &mut e.promo.coupon, w::FieldOpts { hint: "WELCOME", limit: 32, width: Some(150.0), ..Default::default() });
                    }
                });
                if e.coupon && e.promo.coupon.trim().is_empty() {
                    valid = false;
                }
                ui.horizontal(|ui| {
                    ui.label(t.text(lang.text("ใช้ได้สูงสุด (บิล)", "Use on at most (bills)"), type_scale::BODY));
                    w::text_input(ui, &mut e.max_uses, w::FieldOpts { hint: lang.text("ไม่จำกัด", "unlimited"), limit: 6, width: Some(110.0), ..Default::default() });
                    if exists {
                        ui.label(t.muted(format!("{} {}", lang.text("ใช้ไปแล้ว", "used"), e.promo.uses), type_scale::CAPTION));
                    }
                });
                ui.horizontal(|ui| {
                    w::switch(ui, &mut e.promo.active, lang.text("เปิดใช้", "Active"));
                    ui.label(t.text(lang.text("เปิดใช้", "Active"), type_scale::BODY));
                });
            });
        let ready = valid && built.is_some();
        if !ready {
            w::help_text(
                ui,
                lang.text(
                    "กรอกชื่อ เงื่อนไข และช่องที่เลือกให้ครบและถูกต้อง",
                    "Fill in the name, the rule and every chosen field correctly.",
                ),
            );
        }
        if let Some(error) = &app.error {
            w::inline_error(ui, error);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
                .enabled(ready && !app.busy)
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
            if exists {
                let label = if e.delete {
                    lang.text("ยืนยันลบโปรนี้", "Confirm delete")
                } else {
                    lang.text("ลบ", "Delete")
                };
                if w::button(label)
                    .danger()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
                {
                    if e.delete {
                        out = Some(2);
                    } else {
                        e.delete = true;
                    }
                }
            }
        });
        out
    });
    match response.inner {
        Some(1) => {
            if let Some(promo) = built {
                app.send(Command::SavePromotion(promo));
            }
            app.promotions.editor = Some(e);
        }
        Some(2) => {
            app.send(Command::DeletePromotion(e.promo.id));
            app.promotions.editor = Some(e);
        }
        Some(_) => app.error = None,
        None if response.should_close() => {}
        None => app.promotions.editor = Some(e),
    }
}
/// "Coupon" on a bill: type a code; it must belong to a promotion.
pub fn coupon_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut code) = app.promotions.coupon.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let known = app
        .data
        .promotions
        .iter()
        .find(|p| p.is_coupon() && pos_domain::same_code(&p.coupon, &code))
        .cloned();
    let response = w::dialog(ctx, "coupon", 420.0, |ui| {
        w::dialog_title(ui, lang.text("ใส่คูปอง", "Enter a coupon"), false);
        let r = w::text_input(
            ui,
            &mut code,
            w::FieldOpts {
                hint: "WELCOME",
                limit: 32,
                ..Default::default()
            },
        );
        if !r.has_focus() && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        match &known {
            Some(p) => {
                ui.label(t.text(&p.name, type_scale::BODY_STRONG));
                ui.add(egui::Label::new(t.muted(summary(app, p), type_scale::CAPTION)).wrap());
                let now = time::OffsetDateTime::now_utc().unix_timestamp();
                if !p.live(
                    &pos_domain::LocalTime::bangkok(now),
                    std::slice::from_ref(&code),
                ) {
                    w::inline_error(
                        ui,
                        lang.text(
                            "คูปองนี้ใช้ตอนนี้ไม่ได้ (ปิดอยู่ หมดเขต นอกวัน/เวลา หรือใช้ครบแล้ว)",
                            "This coupon cannot be used now (off, expired, outside its days/hours, or used up).",
                        ),
                    );
                }
            }
            None if !code.trim().is_empty() => {
                w::inline_error(ui, lang.text("ไม่พบคูปองนี้", "No such coupon"));
            }
            None => {}
        }
        if !app.cart.coupons.is_empty() {
            ui.label(t.muted(lang.text("คูปองในบิลนี้", "On this bill"), type_scale::LABEL));
            let mut remove = None;
            ui.horizontal_wrapped(|ui| {
                for (i, c) in app.cart.coupons.iter().enumerate() {
                    if w::chip(ui, &format!("{c} ✕"), true, false)
                        .on_hover_text(lang.text("เอาออก", "Remove"))
                        .clicked()
                    {
                        remove = Some(i);
                    }
                }
            });
            if let Some(i) = remove {
                app.cart.coupons.remove(i);
                app.payment_review = None;
            }
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (w::button(lang.text("ใช้คูปอง", "Apply"))
                .primary()
                .enabled(known.is_some() && app.cart.coupons.len() < 5)
                .show(ui)
                .clicked()
                || enter)
                && known.is_some()
            {
                out = Some(true);
            }
            if w::button(lang.text("ปิด", "Close"))
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
            if let Some(p) = known
                && !app
                    .cart
                    .coupons
                    .iter()
                    .any(|c| pos_domain::same_code(c, &p.coupon))
                && app.cart.coupons.len() < 5
            {
                app.cart.coupons.push(p.coupon.clone());
                app.payment_review = None;
                app.toast(format!(
                    "{} {}",
                    lang.text("ใช้คูปองแล้ว:", "Coupon applied:"),
                    p.name
                ));
            }
        }
        Some(false) => {}
        None if response.should_close() => {}
        None => app.promotions.coupon = Some(code),
    }
}
