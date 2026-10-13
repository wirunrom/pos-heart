//! Products workspace (design/docs/components/ScreenProducts.md): SideNav with
//! categories and saved views, FilterBar, BulkBar, DataTable and an Inspector
//! (a Sheet below 1200 points) with explicit Save / Discard.
use crate::{
    app::{App, Page},
    sale_ui::nav_item,
    shell::Undo,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Id, Key, Modifiers, Rect, RichText, Sense, Stroke,
    pos2, vec2,
};
use pos_core::ProductView;
use pos_domain::{Product, ProductInput, StockKind, StockRequest};
use std::collections::BTreeSet;
use uuid::Uuid;

/// Editable option group; prices stay text until saved.
#[derive(Clone, PartialEq, Eq)]
pub struct GroupDraft {
    id: Uuid,
    name: String,
    required: bool,
    multiple: bool,
    choices: Vec<ChoiceDraft>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct ChoiceDraft {
    id: Uuid,
    name: String,
    price: String,
}
impl From<&pos_domain::OptionGroup> for GroupDraft {
    fn from(g: &pos_domain::OptionGroup) -> Self {
        Self {
            id: g.id,
            name: g.name.clone(),
            required: g.required,
            multiple: g.multiple,
            choices: g
                .choices
                .iter()
                .map(|c| ChoiceDraft {
                    id: c.id,
                    name: c.name.clone(),
                    price: format::money(c.price),
                })
                .collect(),
        }
    }
}
impl GroupDraft {
    fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            required: false,
            multiple: false,
            choices: vec![ChoiceDraft::new()],
        }
    }
    fn group(&self) -> Option<pos_domain::OptionGroup> {
        Some(pos_domain::OptionGroup {
            id: self.id,
            name: self.name.trim().to_owned(),
            required: self.required,
            multiple: self.multiple,
            choices: self
                .choices
                .iter()
                .map(|c| {
                    Some(pos_domain::OptionChoice {
                        id: c.id,
                        name: c.name.trim().to_owned(),
                        price: if c.price.trim().is_empty() {
                            pos_domain::Money::ZERO
                        } else {
                            format::parse_money(&c.price).ok()?
                        },
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        })
    }
}
impl ChoiceDraft {
    fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            price: String::new(),
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Editor {
    pub id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub price: String,
    pub cost: String,
    pub tracked: bool,
    pub stock: String,
    pub barcode: String,
    pub on_sale: bool,
    pub service_charge: bool,
    pub kitchen: bool,
    /// Saved values, for "unsaved changes" and "was 89.00".
    original: Option<Box<Editor>>,
    pub previous_price: Option<pos_domain::Money>,
    pub options: Vec<GroupDraft>,
    /// Recipe rows: ingredient and amount per unit sold (typed).
    pub recipe: Vec<(Uuid, String)>,
    pub recipe_search: String,
    /// Option rows: choice, ingredient and signed amount per unit (typed).
    pub option_recipe: Vec<(Uuid, Uuid, String)>,
    /// Choice picked for the next option row.
    pub recipe_choice: Option<Uuid>,
    /// Debug previews scroll the editor to the recipe once.
    pub scroll_to_recipe: bool,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            id: None,
            sku: String::new(),
            name: String::new(),
            category: String::new(),
            price: String::new(),
            cost: String::new(),
            tracked: false,
            stock: "0".into(),
            barcode: String::new(),
            on_sale: true,
            service_charge: true,
            kitchen: true,
            original: None,
            previous_price: None,
            options: Vec::new(),
            recipe: Vec::new(),
            recipe_search: String::new(),
            option_recipe: Vec::new(),
            recipe_choice: None,
            scroll_to_recipe: false,
        }
    }
}
impl From<Product> for Editor {
    fn from(p: Product) -> Self {
        let mut e = Self {
            id: Some(p.id),
            sku: p.sku,
            name: p.name,
            category: p.category,
            price: format::money(p.price),
            cost: p.cost.map(format::money).unwrap_or_default(),
            tracked: p.stock.is_some(),
            stock: p.stock.unwrap_or(0).to_string(),
            barcode: p.barcode.unwrap_or_default(),
            on_sale: p.on_sale,
            service_charge: p.service_charge,
            kitchen: p.kitchen,
            original: None,
            previous_price: Some(p.price),
            options: p.options.iter().map(GroupDraft::from).collect(),
            recipe: p
                .recipe
                .iter()
                .map(|r| (r.ingredient_id, format::qty(r.amount)))
                .collect(),
            recipe_search: String::new(),
            option_recipe: p
                .option_recipe
                .iter()
                .map(|r| (r.choice_id, r.ingredient_id, format::signed_qty(r.amount)))
                .collect(),
            recipe_choice: None,
            scroll_to_recipe: false,
        };
        e.original = Some(Box::new(e.clone()));
        e
    }
}
impl Editor {
    fn fields(&self) -> (&str, &str, &str, &str, bool, &str, &str, bool) {
        (
            &self.sku,
            &self.name,
            &self.category,
            &self.price,
            self.tracked,
            &self.stock,
            &self.barcode,
            self.on_sale,
        )
    }
    pub fn dirty(&self) -> bool {
        match &self.original {
            Some(o) => {
                o.fields() != self.fields()
                    || o.options != self.options
                    || o.recipe != self.recipe
                    || o.option_recipe != self.option_recipe
                    || o.service_charge != self.service_charge
                    || o.kitchen != self.kitchen
                    || o.cost != self.cost
            }
            None => {
                !(self.sku.is_empty()
                    && self.name.is_empty()
                    && self.category.is_empty()
                    && self.price.is_empty()
                    && self.cost.is_empty()
                    && self.barcode.is_empty()
                    && self.options.is_empty()
                    && self.recipe.is_empty())
            }
        }
    }
    fn input(&self) -> Result<ProductInput, String> {
        let price = format::parse_money(&self.price).map_err(|_| "price".to_owned())?;
        let cost = if self.cost.trim().is_empty() {
            None
        } else {
            Some(format::parse_money(&self.cost).map_err(|_| "cost".to_owned())?)
        };
        let stock = if self.tracked {
            Some(
                self.stock
                    .trim()
                    .replace(',', "")
                    .parse::<u32>()
                    .map_err(|_| "stock".to_owned())?,
            )
        } else {
            None
        };
        Ok(ProductInput {
            id: self.id,
            sku: self.sku.trim().to_owned(),
            name: self.name.trim().to_owned(),
            category: self.category.trim().to_owned(),
            price,
            cost,
            stock,
            barcode: Some(self.barcode.trim().to_owned()).filter(|b| !b.is_empty()),
            on_sale: self.on_sale,
            service_charge: self.service_charge,
            kitchen: self.kitchen,
            keep_tracked_stock: true,
            recipe: self
                .recipe
                .iter()
                .map(|(id, amount)| {
                    format::parse_qty(amount).filter(|a| *a > 0).map(|amount| {
                        pos_domain::RecipeItem {
                            ingredient_id: *id,
                            amount,
                        }
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| "recipe".to_owned())?,
            // Rows of choices removed from the options are dropped.
            option_recipe: self
                .option_recipe
                .iter()
                .filter(|(choice, ..)| {
                    self.options
                        .iter()
                        .any(|g| g.choices.iter().any(|c| c.id == *choice))
                })
                .map(|(choice, ingredient, amount)| {
                    format::parse_signed_qty(amount).map(|amount| pos_domain::OptionRecipe {
                        choice_id: *choice,
                        ingredient_id: *ingredient,
                        amount,
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| "recipe".to_owned())?,
            options: self
                .options
                .iter()
                .map(GroupDraft::group)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| "options".to_owned())?,
        })
    }
}
#[derive(Default)]
pub struct CatalogState {
    pub import: Option<crate::import_ui::ImportState>,
    pub search: String,
    pub category: Option<String>,
    pub view: ProductView,
    pub offset: u32,
    pub selected: BTreeSet<Uuid>,
    anchor: Option<usize>,
    pub editor: Option<Editor>,
    pub pending_nav: Option<Page>,
    pending_open: Option<Option<Product>>,
    archive_confirm: bool,
    bulk_category: Option<String>,
    show_errors: bool,
    focus_search: bool,
    last_bulk: Option<(Vec<Uuid>, bool)>,
    save_requested: bool,
    close_after_prompt: bool,
    pub stock_dialog: Option<StockDialog>,
}
/// Receive / remove / count dialog for one tracked product.
pub struct StockDialog {
    id: Uuid,
    product_id: Uuid,
    name: String,
    kind: StockKind,
    current: u32,
    quantity: String,
    reason: String,
}
impl StockDialog {
    pub fn new(product_id: Uuid, name: String, kind: StockKind, current: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            product_id,
            name,
            kind,
            current,
            quantity: if kind == StockKind::Count {
                current.to_string()
            } else {
                String::new()
            },
            reason: String::new(),
        }
    }
    fn request(&self) -> Option<StockRequest> {
        let quantity = self.quantity.trim().replace(',', "").parse::<u32>().ok()?;
        let request = StockRequest {
            id: self.id,
            product_id: self.product_id,
            kind: self.kind,
            quantity,
            expected: Some(self.current),
            reason: self.reason.trim().to_owned(),
        };
        request.validate().ok()?;
        request.after(self.current).ok()?;
        Some(request)
    }
}
fn stock_kind_label(lang: crate::i18n::Language, kind: StockKind) -> &'static str {
    match kind {
        StockKind::Initial => lang.text("เริ่มนับสต็อก", "Started tracking"),
        StockKind::Receive => lang.text("รับเข้า", "Received"),
        StockKind::Remove => lang.text("นำออก", "Removed"),
        StockKind::Count => lang.text("ตรวจนับ", "Counted"),
    }
}
fn stock_history(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: crate::i18n::Language,
    history: &[pos_domain::StockMovement],
    product: Option<Uuid>,
) {
    let rows: Vec<_> = history
        .iter()
        .filter(|m| Some(m.product_id) == product)
        .take(8)
        .collect();
    if rows.is_empty() {
        return;
    }
    ui.add_space(space::S1);
    ui.label(t.muted(
        lang.text("ประวัติสต็อกล่าสุด", "Recent stock changes"),
        type_scale::LABEL,
    ));
    for m in rows {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 110.0).max(80.0));
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(t.text(stock_kind_label(lang, m.kind), type_scale::BODY));
                let when = format::date_time(lang, m.created_at);
                let detail = if m.reason.is_empty() {
                    when
                } else {
                    format!("{when} · {}", m.reason)
                };
                ui.add(egui::Label::new(t.muted(detail, type_scale::CAPTION)).truncate());
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(t.muted(format::count(u64::from(m.after)), type_scale::MONEY_SM));
                let delta = m.delta();
                let text = if delta > 0 {
                    format!("+{}", format::signed_count(delta))
                } else {
                    format::signed_count(delta)
                };
                ui.label(
                    RichText::new(text)
                        .font(t.strong_font(type_scale::MONEY))
                        .color(if delta < 0 {
                            t.p.danger_fg
                        } else {
                            t.p.fg_default
                        }),
                );
            });
        });
    }
}
/// Toast and local refresh after a stock movement commits.
pub fn stock_adjusted(app: &mut App, m: &pos_domain::StockMovement) {
    let lang = app.lang;
    app.catalog.stock_dialog = None;
    if let Some(p) = app.known.get_mut(&m.product_id) {
        p.stock = Some(m.after);
    }
    app.toast(format!(
        "{} · {} {}",
        stock_kind_label(lang, m.kind),
        lang.text("คงเหลือ", "in stock"),
        format::count(u64::from(m.after))
    ));
}
fn stock_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut dialog) = app.catalog.stock_dialog.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let request = dialog.request();
    let after = request.as_ref().and_then(|r| r.after(dialog.current).ok());
    let r = w::dialog(ctx, "stock-change", 460.0, |ui| {
        w::dialog_title(
            ui,
            match dialog.kind {
                StockKind::Receive => lang.text("รับสินค้าเข้า", "Receive stock"),
                StockKind::Remove => lang.text("นำสินค้าออก", "Remove stock"),
                _ => lang.text("ตรวจนับสต็อก", "Count stock"),
            },
            false,
        );
        w::name_label(ui, &dialog.name, type_scale::BODY, t.p.fg_muted);
        w::field_label(
            ui,
            match dialog.kind {
                StockKind::Receive => lang.text("จำนวนที่รับเข้า (ชิ้น)", "Units received"),
                StockKind::Remove => lang.text("จำนวนที่นำออก (ชิ้น)", "Units removed"),
                _ => lang.text("นับได้ทั้งหมด (ชิ้น)", "Units counted"),
            },
            None,
        );
        let id = Id::new(("stock-qty", dialog.id));
        let field = w::text_input(
            ui,
            &mut dialog.quantity,
            w::FieldOpts {
                hint: "0",
                limit: 10,
                id: Some(id),
                width: Some(200.0),
                ..Default::default()
            },
        );
        dialog.quantity.retain(|c| c.is_ascii_digit() || c == ',');
        if ui.memory(|m| m.focused().is_none()) {
            field.request_focus();
        }
        if let Some(after) = after {
            let delta = i64::from(after) - i64::from(dialog.current);
            ui.label(t.text(
                format!(
                    "{} {} → {} ({})",
                    lang.text("คงเหลือ", "Stock"),
                    format::count(u64::from(dialog.current)),
                    format::count(u64::from(after)),
                    if delta > 0 {
                        format!("+{}", format::signed_count(delta))
                    } else {
                        format::signed_count(delta)
                    }
                ),
                type_scale::BODY_STRONG,
            ));
        } else if dialog.kind == StockKind::Remove
            && dialog
                .quantity
                .trim()
                .replace(',', "")
                .parse::<u32>()
                .is_ok_and(|q| q > dialog.current)
        {
            w::inline_error(ui, lang.text("นำออกเกินจำนวนที่มี", "More than is in stock"));
        }
        w::field_label(
            ui,
            lang.text("เหตุผล", "Reason"),
            (dialog.kind != StockKind::Remove).then(|| lang.text("(ไม่บังคับ)", "(optional)")),
        );
        let presets: &[(&str, &str)] = match dialog.kind {
            StockKind::Receive => &[("ล็อตใหม่", "New delivery"), ("คืนจากลูกค้า", "Customer return")],
            StockKind::Remove => &[
                ("ชำรุด", "Damaged"),
                ("หมดอายุ", "Expired"),
                ("ใช้ในร้าน", "Used in shop"),
                ("สูญหาย", "Lost"),
            ],
            _ => &[
                ("นับประจำเดือน", "Monthly count"),
                ("นับก่อนปิดร้าน", "End-of-day count"),
            ],
        };
        ui.horizontal_wrapped(|ui| {
            for (th, en) in presets {
                let label = lang.text(th, en);
                if w::chip(ui, label, dialog.reason == label, false).clicked() {
                    dialog.reason = label.to_owned();
                }
            }
        });
        w::text_input(
            ui,
            &mut dialog.reason,
            w::FieldOpts {
                hint: lang.text("หรือพิมพ์เหตุผลเอง", "Or type a reason"),
                limit: 240,
                ..Default::default()
            },
        );
        if dialog.kind == StockKind::Count {
            w::help_text(
                ui,
                lang.text(
                    "ถ้ามีการขายระหว่างนับ ระบบจะให้นับใหม่แทนการบันทึกทับ",
                    "If a sale happens while you count, you will be asked to count again.",
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
                .enabled(request.is_some() && !app.busy)
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
    });
    match r.inner {
        Some(false) => return,
        None if !app.busy && r.should_close() => return,
        Some(true) => {
            if let Some(request) = request {
                app.send(Command::AdjustStock(request));
            }
        }
        None => {}
    }
    app.catalog.stock_dialog = Some(dialog);
}
impl CatalogState {
    /// Cancel queued intents and destructive overlays, retain the product draft.
    pub fn cancel_confirmations(&mut self) {
        self.archive_confirm = false;
        self.pending_nav = None;
        self.pending_open = None;
        self.close_after_prompt = false;
        self.save_requested = false;
        self.stock_dialog = None;
        self.bulk_category = None;
    }
    pub fn dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(Editor::dirty)
    }
}
fn search_id() -> Id {
    Id::new("catalog-search")
}

pub fn saved(app: &mut App, product: Product) {
    let lang = app.lang;
    app.toast(format!(
        "{} {}",
        lang.text("บันทึกแล้ว:", "Saved:"),
        product.name
    ));
    app.known.insert(product.id, product.clone());
    app.catalog.show_errors = false;
    app.catalog.editor = Some(product.into());
}
pub fn bulk_done(app: &mut App, n: usize) {
    let lang = app.lang;
    app.catalog.selected.clear();
    match app.catalog.last_bulk.take() {
        Some((ids, on_sale)) => app.toast_undo(
            format!(
                "{} {n} {}",
                if on_sale {
                    lang.text("เปิดขายแล้ว", "Selling again:")
                } else {
                    lang.text("ปิดขายแล้ว", "Stopped selling:")
                },
                lang.noun(n as u64, "รายการ", "item", "items")
            ),
            Undo::SetOnSale(ids, !on_sale),
        ),
        None => app.toast(format!(
            "{} {n} {}",
            lang.text("อัปเดตแล้ว", "Updated"),
            lang.noun(n as u64, "รายการ", "item", "items")
        )),
    }
    if let Some(editor) = &app.catalog.editor
        && editor
            .id
            .is_some_and(|id| !app.data.products.iter().any(|p| p.id == id))
    {
        app.catalog.editor = None;
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let wide = full.width() >= 1180.0;
    let nav_w = if full.width() < 1100.0 { 176.0 } else { 208.0 };
    let inspector_w = if wide && app.catalog.editor.is_some() {
        340.0
    } else {
        0.0
    };
    let nav = Rect::from_min_size(full.min, vec2(nav_w, full.height()));
    let main = Rect::from_min_max(
        pos2(nav.right() + 10.0, full.top()),
        pos2(
            full.right()
                - if inspector_w > 0.0 {
                    inspector_w + 10.0
                } else {
                    0.0
                },
            full.bottom(),
        ),
    );
    keys(app, ui.ctx());
    panel(ui, &t, nav, space::S2, |ui| side_nav(app, ui));
    if app.ingredients.open {
        // Ingredients replace the product table; the product editor stays closed.
        let main = Rect::from_min_max(main.min, pos2(full.right(), full.bottom()));
        panel(ui, &t, main, space::S4, |ui| {
            crate::ingredients_ui::panel(app, ui)
        });
        return;
    }
    panel(ui, &t, main, space::S4, |ui| table_panel(app, ui));
    if inspector_w > 0.0 {
        let rect = Rect::from_min_max(pos2(full.right() - inspector_w, full.top()), full.max);
        panel(ui, &t, rect, space::S4, |ui| inspector(app, ui));
    } else if app.catalog.editor.is_some() {
        sheet(app, ui.ctx());
    }
}
fn panel(ui: &mut egui::Ui, t: &Theme, rect: Rect, pad: f32, add: impl FnOnce(&mut egui::Ui)) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(Align::Min)),
    );
    t.surface()
        .inner_margin(egui::Margin::same(pad as i8))
        .show(&mut child, |ui| {
            let inner = rect.size() - vec2(pad * 2.0, pad * 2.0);
            ui.set_min_size(inner);
            ui.set_max_size(inner);
            add(ui);
        });
}
fn keys(app: &mut App, ctx: &egui::Context) {
    if app.palette.visible || app.catalog.archive_confirm || app.catalog.pending_nav.is_some() {
        return;
    }
    let typing = ctx.memory(|m| m.focused().is_some());
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::COMMAND, Key::S) && app.catalog.editor.is_some() {
            app.catalog.save_requested = true;
        }
        if !typing && i.consume_key(Modifiers::NONE, Key::Slash) {
            app.catalog.focus_search = true;
        }
        if !typing && i.consume_key(Modifiers::COMMAND, Key::A) {
            app.catalog.selected = app.data.products.iter().map(|p| p.id).collect();
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) && !typing {
            if !app.catalog.selected.is_empty() {
                app.catalog.selected.clear();
            } else if app.catalog.editor.as_ref().is_some_and(|e| !e.dirty()) {
                app.catalog.editor = None;
            }
        }
    });
    if std::mem::take(&mut app.catalog.save_requested) {
        save(app);
    }
}
fn refresh_from_start(app: &mut App) {
    app.catalog.offset = 0;
    app.catalog.selected.clear();
    app.refresh();
}

fn side_nav(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let counts = app.data.counts.clone();
    egui::ScrollArea::vertical()
        .id_salt("catalog-nav")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(space::S2);
            ui.horizontal(|ui| {
                ui.add_space(space::S3);
                w::eyebrow(ui, lang.text("หมวดหมู่", "Categories"));
            });
            ui.add_space(space::S1);
            let mut entries = vec![(
                None,
                lang.text("ทั้งหมด", "All products").to_owned(),
                counts.total,
            )];
            entries.extend(counts.categories.iter().map(|(c, n)| {
                (
                    Some(c.clone()),
                    if c.is_empty() {
                        lang.text("ไม่มีหมวด", "Uncategorised").to_owned()
                    } else {
                        c.clone()
                    },
                    *n,
                )
            }));
            for (value, label, n) in entries {
                let active = app.catalog.category == value
                    && app.catalog.view == ProductView::All
                    && !app.ingredients.open;
                if nav_item(ui, &t, &label, n, active).clicked() {
                    app.ingredients.open = false;
                    app.catalog.category = value;
                    app.catalog.view = ProductView::All;
                    refresh_from_start(app);
                }
            }
            ui.add_space(space::S4);
            ui.horizontal(|ui| {
                ui.add_space(space::S3);
                w::eyebrow(ui, lang.text("มุมมอง", "Views"));
            });
            ui.add_space(space::S1);
            for (view, label, n, tone) in [
                (
                    ProductView::LowStock,
                    lang.text("สต็อกต่ำ", "Low stock"),
                    counts.low_stock,
                    w::Tone::Warning,
                ),
                (
                    ProductView::OutOfStock,
                    lang.text("หมดสต็อก", "Out of stock"),
                    counts.out_of_stock,
                    w::Tone::Danger,
                ),
                (
                    ProductView::NotForSale,
                    lang.text("ปิดขาย", "Not for sale"),
                    counts.not_for_sale,
                    w::Tone::Neutral,
                ),
            ] {
                let active = app.catalog.view == view && !app.ingredients.open;
                if view_item(ui, &t, label, n, tone, active).clicked() {
                    app.ingredients.open = false;
                    app.catalog.view = view;
                    app.catalog.category = None;
                    refresh_from_start(app);
                }
            }
            ui.add_space(space::S4);
            ui.horizontal(|ui| {
                ui.add_space(space::S3);
                w::eyebrow(ui, lang.text("ครัว", "Kitchen"));
            });
            ui.add_space(space::S1);
            let low = crate::ingredients_ui::low_count(app) as u64;
            if view_item(
                ui,
                &t,
                lang.text("วัตถุดิบ", "Ingredients"),
                low,
                w::Tone::Warning,
                app.ingredients.open,
            )
            .clicked()
                && !app.catalog.dirty()
            {
                app.ingredients.open = true;
                app.catalog.editor = None;
            }
        });
}
fn view_item(
    ui: &mut egui::Ui,
    t: &Theme,
    label: &str,
    n: u64,
    tone: w::Tone,
    active: bool,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, t.m.control_md), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, label));
    let bg = if active {
        t.p.bg_sunken
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    ui.painter().text(
        pos2(rect.left() + space::S3, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        if active {
            t.strong_font(type_scale::BODY)
        } else {
            t.font(type_scale::BODY)
        },
        if active { t.p.fg_default } else { t.p.fg_muted },
    );
    let (bg, fg) = if n == 0 {
        w::Tone::Neutral.colors(t)
    } else {
        tone.colors(t)
    };
    let text = format::count(n);
    let g = ui
        .painter()
        .layout_no_wrap(text, t.strong_font(type_scale::MONEY_SM), fg);
    let badge = Rect::from_center_size(
        pos2(
            rect.right() - space::S3 - (g.size().x + 10.0).max(20.0) / 2.0,
            rect.center().y,
        ),
        vec2((g.size().x + 10.0).max(20.0), 20.0),
    );
    ui.painter().rect_filled(badge, 4, bg);
    ui.painter().galley(badge.center() - g.size() / 2.0, g, fg);
    w::focus_ring(ui, rect, CornerRadius::same(8), &response);
    response
}

fn title_text(app: &App) -> String {
    let lang = app.lang;
    match app.catalog.view {
        ProductView::LowStock => lang.text("สต็อกต่ำ", "Low stock").into(),
        ProductView::OutOfStock => lang.text("หมดสต็อก", "Out of stock").into(),
        ProductView::NotForSale => lang.text("ปิดขาย", "Not for sale").into(),
        ProductView::All => match &app.catalog.category {
            Some(c) if c.is_empty() => lang.text("ไม่มีหมวด", "Uncategorised").into(),
            Some(c) => c.clone(),
            None => lang.text("สินค้าทั้งหมด", "All products").into(),
        },
    }
}
fn table_panel(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let total = match (&app.catalog.view, &app.catalog.category) {
        (ProductView::LowStock, _) => app.data.counts.low_stock,
        (ProductView::OutOfStock, _) => app.data.counts.out_of_stock,
        (ProductView::NotForSale, _) => app.data.counts.not_for_sale,
        (ProductView::All, Some(c)) => app
            .data
            .counts
            .categories
            .iter()
            .find(|(n, _)| n == c)
            .map_or(0, |(_, n)| *n),
        (ProductView::All, None) => app.data.counts.total,
    };
    ui.spacing_mut().item_spacing.y = space::S3;
    ui.horizontal(|ui| {
        ui.label(t.text(title_text(app), type_scale::TITLE_LG));
        ui.label(t.muted(format::count(total), type_scale::BODY));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = space::S2;
            let add = w::button(lang.text("เพิ่มสินค้า", "Add product")).icon(Icon::Plus);
            let add = if app.catalog.editor.is_some() {
                add
            } else {
                add.primary()
            };
            if add.enabled(!app.busy).show(ui).clicked() {
                open_editor(app, None);
            }
            if w::button(lang.text("นำเข้า CSV", "Import CSV"))
                .enabled(!app.busy && !app.catalog.dirty())
                .show(ui)
                .clicked()
            {
                app.catalog.import = Some(Default::default());
            }
            if w::button(lang.text("ส่งออก", "Export"))
                .icon(Icon::Download)
                .enabled(!app.busy && app.data.counts.total > 0)
                .tooltip(lang.text(
                    "บันทึกสินค้าทั้งหมดเป็นไฟล์ CSV ในโฟลเดอร์ข้อมูล",
                    "Save every product as CSV in the data folder",
                ))
                .show(ui)
                .clicked()
            {
                app.export_products();
            }
        });
    });
    if app.catalog.selected.is_empty() {
        filter_bar(app, ui);
    } else {
        bulk_bar(app, ui);
    }
    table(app, ui);
}
fn filter_bar(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S2;
        let selects_w = 360.0_f32.min(ui.available_width() * 0.45);
        ui.allocate_ui(
            vec2(ui.available_width() - selects_w - space::S2, t.m.control_md),
            |ui| {
                let r = w::search_input(
                    ui,
                    &mut app.catalog.search,
                    lang.text("ค้นหาชื่อ รหัส บาร์โค้ด", "Search name, SKU, barcode"),
                    Some("/"),
                    search_id(),
                );
                if std::mem::take(&mut app.catalog.focus_search) {
                    r.request_focus();
                }
                if r.changed() {
                    app.catalog.offset = 0;
                    app.search_changed = Some(std::time::Instant::now());
                }
            },
        );
        let mut categories: Vec<(Option<String>, String)> =
            vec![(None, lang.text("หมวด: ทั้งหมด", "Category: All").to_owned())];
        categories.extend(app.data.counts.categories.iter().map(|(c, _)| {
            (
                Some(c.clone()),
                if c.is_empty() {
                    lang.text("ไม่มีหมวด", "Uncategorised").to_owned()
                } else {
                    c.clone()
                },
            )
        }));
        let current = categories
            .iter()
            .find(|(v, _)| *v == app.catalog.category)
            .map_or_else(|| categories[0].1.clone(), |(_, l)| l.clone());
        let mut chosen = None;
        egui::ComboBox::from_id_salt("catalog-category")
            .icon(w::chevron)
            .selected_text(truncate(&current, 22))
            .width(selects_w / 2.0 - 4.0)
            .height(360.0)
            .show_ui(ui, |ui| {
                for (value, label) in &categories {
                    if ui
                        .selectable_label(*value == app.catalog.category, label)
                        .clicked()
                    {
                        chosen = Some(value.clone());
                    }
                }
            });
        if let Some(value) = chosen {
            app.catalog.category = value;
            if app.catalog.view != ProductView::All && app.catalog.view != ProductView::NotForSale {
                app.catalog.view = ProductView::All;
            }
            refresh_from_start(app);
        }
        let views = [
            (ProductView::All, lang.text("สถานะ: ทั้งหมด", "Status: All")),
            (ProductView::LowStock, lang.text("สต็อกต่ำ", "Low stock")),
            (
                ProductView::OutOfStock,
                lang.text("หมดสต็อก", "Out of stock"),
            ),
            (ProductView::NotForSale, lang.text("ปิดขาย", "Not for sale")),
        ];
        let mut view = app.catalog.view;
        egui::ComboBox::from_id_salt("catalog-status")
            .icon(w::chevron)
            .selected_text(
                views
                    .iter()
                    .find(|(v, _)| *v == view)
                    .map_or("", |(_, l)| l),
            )
            .width(selects_w / 2.0 - 4.0)
            .show_ui(ui, |ui| {
                for (v, label) in views {
                    ui.selectable_value(&mut view, v, label);
                }
            });
        if view != app.catalog.view {
            app.catalog.view = view;
            refresh_from_start(app);
        }
    });
}
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(max - 1).collect::<String>())
    }
}
fn bulk_bar(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let ids: Vec<Uuid> = app.catalog.selected.iter().copied().collect();
    let all_off = app
        .data
        .products
        .iter()
        .filter(|p| app.catalog.selected.contains(&p.id))
        .all(|p| !p.on_sale);
    let height = t.m.control_lg;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), t.p.bg_inverse);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(space::S3, 0.0)))
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = space::S2;
    let fg = t.p.fg_inverse;
    ui.label(
        RichText::new(format!("{} {}", lang.text("เลือก", "Selected"), ids.len()))
            .font(t.strong_font(type_scale::BODY))
            .color(fg),
    );
    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
        let inverse_button = |ui: &mut egui::Ui, icon: Icon, label: &str| -> egui::Response {
            let g =
                ui.painter()
                    .layout_no_wrap(label.to_owned(), t.strong_font(type_scale::BODY), fg);
            let (r, resp) =
                ui.allocate_exact_size(vec2(g.size().x + 42.0, height - 10.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            if resp.hovered() {
                ui.painter().rect_filled(r, 6, fg.gamma_multiply(0.15));
            }
            crate::ui::icons::paint_sized(
                ui.painter(),
                icon,
                pos2(r.left() + 16.0, r.center().y),
                16.0,
                fg,
            );
            ui.painter().galley(
                pos2(r.left() + 30.0, r.center().y - g.size().y / 2.0),
                g,
                fg,
            );
            w::focus_ring(ui, r, CornerRadius::same(6), &resp);
            resp
        };
        if inverse_button(ui, Icon::Close, lang.text("ล้าง", "Clear")).clicked() {
            app.catalog.selected.clear();
        }
        ui.add_enabled_ui(!app.busy, |ui| {
            let (icon, label) = if all_off {
                (Icon::Unlock, lang.text("เปิดขาย", "Resume selling"))
            } else {
                (Icon::Lock, lang.text("ปิดขาย", "Stop selling"))
            };
            if inverse_button(ui, icon, label).clicked() {
                app.catalog.last_bulk = Some((ids.clone(), all_off));
                app.send(Command::SetOnSale(ids.clone(), all_off));
            }
            if inverse_button(ui, Icon::Tag, lang.text("เปลี่ยนหมวด", "Category")).clicked()
            {
                app.catalog.bulk_category = Some(String::new());
            }
        });
    });
}

struct Columns {
    check: f32,
    name: f32,
    category: f32,
    stock: f32,
    price: f32,
    status: f32,
}
fn columns(width: f32, t: &Theme) -> Columns {
    let check = if t.touch() { 48.0 } else { 40.0 };
    let stock = 112.0;
    let price = 104.0;
    let status = 96.0;
    let category = if width >= 760.0 { 150.0 } else { 0.0 };
    let name = (width - check - category - stock - price - status).max(120.0);
    Columns {
        check,
        name,
        category,
        stock,
        price,
        status,
    }
}
fn table(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let products = app.data.products.clone();
    let width = ui.available_width();
    let c = columns(width, &t);
    // Header (sticky: drawn outside the scroll area).
    let (head, _) = ui.allocate_exact_size(vec2(width, 36.0), Sense::hover());
    ui.painter().line_segment(
        [head.left_bottom(), head.right_bottom()],
        Stroke::new(1.0, t.p.border_default),
    );
    let all = !products.is_empty()
        && products
            .iter()
            .all(|p| app.catalog.selected.contains(&p.id));
    let some = products
        .iter()
        .any(|p| app.catalog.selected.contains(&p.id));
    let mut header_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(head.min, vec2(c.check, 36.0)))
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    if w::checkbox(
        &mut header_ui,
        if all {
            w::Check::On
        } else if some {
            w::Check::Mixed
        } else {
            w::Check::Off
        },
        lang.text("เลือกทั้งหมดในหน้านี้", "Select all on this page"),
    )
    .clicked()
    {
        if all {
            app.catalog.selected.clear();
        } else {
            app.catalog.selected = products.iter().map(|p| p.id).collect();
        }
    }
    let head_font = t.strong_font(type_scale::CAPTION);
    let muted = t.p.fg_muted;
    let mut x = head.left() + c.check;
    let y = head.center().y;
    let painter = ui.painter().clone();
    painter.text(
        pos2(x + 4.0, y),
        Align2::LEFT_CENTER,
        lang.text("สินค้า", "Product"),
        head_font.clone(),
        muted,
    );
    x += c.name;
    if c.category > 0.0 {
        painter.text(
            pos2(x + space::S3, y),
            Align2::LEFT_CENTER,
            lang.text("หมวด", "Category"),
            head_font.clone(),
            muted,
        );
        x += c.category;
    }
    x += c.stock;
    painter.text(
        pos2(x - space::S3, y),
        Align2::RIGHT_CENTER,
        lang.text("คงเหลือ", "Stock"),
        head_font.clone(),
        muted,
    );
    x += c.price;
    painter.text(
        pos2(x - space::S3, y),
        Align2::RIGHT_CENTER,
        lang.text("ราคา", "Price"),
        head_font.clone(),
        muted,
    );
    painter.text(
        pos2(x + space::S3, y),
        Align2::LEFT_CENTER,
        lang.text("สถานะ", "Status"),
        head_font,
        muted,
    );
    let footer_h = t.m.control_md + space::S2;
    egui::ScrollArea::vertical()
        .id_salt("catalog-table")
        .auto_shrink([false, false])
        .max_height(ui.available_height() - footer_h)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if products.is_empty() {
                empty(app, ui);
            }
            for (index, p) in products.iter().enumerate() {
                row(app, ui, &c, index, p);
            }
        });
    ui.horizontal(|ui| {
        ui.add_enabled_ui(!app.busy, |ui| {
            if w::button(lang.text("ก่อนหน้า", "Previous"))
                .icon(Icon::ChevronLeft)
                .size(w::Size::Sm)
                .enabled(app.catalog.offset > 0)
                .show(ui)
                .clicked()
            {
                app.catalog.offset = app.catalog.offset.saturating_sub(100);
                app.catalog.selected.clear();
                app.refresh();
            }
            ui.label(t.muted(
                format!(
                    "{} {}",
                    lang.text("หน้า", "Page"),
                    app.catalog.offset / 100 + 1
                ),
                type_scale::LABEL,
            ));
            if w::button(lang.text("ถัดไป", "Next"))
                .icon(Icon::ChevronRight)
                .size(w::Size::Sm)
                .enabled(products.len() == 100)
                .show(ui)
                .clicked()
            {
                app.catalog.offset = app.catalog.offset.saturating_add(100);
                app.catalog.selected.clear();
                app.refresh();
            }
        });
    });
}
fn empty(app: &mut App, ui: &mut egui::Ui) {
    let lang = app.lang;
    if app.data.counts.total == 0 {
        w::state_view(
            ui,
            Icon::Products,
            lang.text("ยังไม่มีสินค้า", "No products yet"),
            lang.text(
                "เพิ่มสินค้าแรกเพื่อเริ่มขาย",
                "Add your first product to start selling.",
            ),
        );
        ui.vertical_centered(|ui| {
            if w::button(lang.text("โหลดสินค้าตัวอย่าง", "Load sample products"))
                .ghost()
                .enabled(!app.busy)
                .show(ui)
                .clicked()
            {
                app.send(Command::Demo);
            }
        });
    } else {
        w::state_view(
            ui,
            Icon::Search,
            &if app.catalog.search.trim().is_empty() {
                lang.text("ไม่มีสินค้าในมุมมองนี้", "Nothing in this view")
                    .to_owned()
            } else {
                format!(
                    "{} “{}”",
                    lang.text("ไม่พบ", "No match for"),
                    app.catalog.search.trim()
                )
            },
            &format!("{} · {}", lang.text("ตัวกรอง", "Filters"), title_text(app)),
        );
        ui.vertical_centered(|ui| {
            if w::button(lang.text("ล้างตัวกรอง", "Clear filters"))
                .show(ui)
                .clicked()
            {
                app.catalog.search.clear();
                app.catalog.category = None;
                app.catalog.view = ProductView::All;
                refresh_from_start(app);
            }
        });
    }
}
fn row(app: &mut App, ui: &mut egui::Ui, c: &Columns, index: usize, p: &Product) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let width = ui.available_width();
    let (name_g, _) = crate::ui::text::wrapped(
        ui.painter(),
        &p.name,
        t.font(type_scale::BODY),
        t.p.fg_default,
        c.name - space::S2,
        0,
    );
    let code = match &p.barcode {
        Some(b) => format!("{} · {b}", p.sku),
        None => p.sku.clone(),
    };
    let h = (name_g.size().y + 18.0 + space::S2 * 2.0).max(t.m.row_height);
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    let selected = app.catalog.selected.contains(&p.id);
    let open = app.catalog.editor.as_ref().and_then(|e| e.id) == Some(p.id);
    let bg = if selected || open {
        t.p.bg_selected
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0, bg);
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, t.p.border_subtle),
    );
    // Checkbox column is its own target: tap selects, tap elsewhere opens.
    let mut check_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(rect.min, vec2(c.check, h)))
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    let check = w::checkbox(
        &mut check_ui,
        if selected {
            w::Check::On
        } else {
            w::Check::Off
        },
        &format!("{} {}", lang.text("เลือก", "Select"), p.name),
    );
    if check.clicked() {
        let shift = ui.input(|i| i.modifiers.shift);
        match (shift, app.catalog.anchor) {
            (true, Some(anchor)) => {
                let (a, b) = (anchor.min(index), anchor.max(index));
                for q in &app.data.products[a..=b.min(app.data.products.len() - 1)] {
                    app.catalog.selected.insert(q.id);
                }
            }
            _ => {
                if !app.catalog.selected.remove(&p.id) {
                    app.catalog.selected.insert(p.id);
                }
            }
        }
        app.catalog.anchor = Some(index);
    } else if response.clicked() {
        open_editor(app, Some(p.clone()));
    }
    let mut x = rect.left() + c.check;
    let top = rect.top() + space::S2;
    let name_h = name_g.size().y;
    ui.painter()
        .galley(pos2(x + 4.0, top), name_g, t.p.fg_default);
    ui.painter().text(
        pos2(x + 4.0, top + name_h + 1.0),
        Align2::LEFT_TOP,
        code,
        t.font(type_scale::MONEY_SM),
        t.p.fg_muted,
    );
    x += c.name;
    let cy = rect.center().y;
    if c.category > 0.0 {
        let (g, _) = crate::ui::text::wrapped(
            ui.painter(),
            &p.category,
            t.font(type_scale::BODY),
            t.p.fg_muted,
            c.category - space::S3 * 2.0,
            2,
        );
        ui.painter()
            .galley(pos2(x + space::S3, cy - g.size().y / 2.0), g, t.p.fg_muted);
        x += c.category;
    }
    x += c.stock;
    match p.stock {
        None => {
            ui.painter().text(
                pos2(x - space::S3, cy),
                Align2::RIGHT_CENTER,
                lang.text("ไม่นับสต็อก", "Not tracked"),
                t.font(type_scale::CAPTION),
                t.p.fg_muted,
            );
        }
        Some(n) if n == 0 || p.is_low_stock() => {
            let (tone, label) = if n == 0 {
                (w::Tone::Danger, lang.text("หมด", "Out"))
            } else {
                (w::Tone::Warning, lang.text("ต่ำ", "Low"))
            };
            let (bg, fg) = tone.colors(&t);
            let text = format!("{label} · {n}");
            let g = ui
                .painter()
                .layout_no_wrap(text, t.strong_font(type_scale::MONEY_SM), fg);
            let r = Rect::from_min_size(
                pos2(x - space::S3 - g.size().x - 12.0, cy - 11.0),
                vec2(g.size().x + 12.0, 22.0),
            );
            ui.painter().rect_filled(r, 4, bg);
            ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
        }
        Some(n) => {
            ui.painter().text(
                pos2(x - space::S3, cy),
                Align2::RIGHT_CENTER,
                format::count(u64::from(n)),
                t.font(type_scale::MONEY),
                t.p.fg_default,
            );
        }
    }
    x += c.price;
    ui.painter().text(
        pos2(x - space::S3, cy),
        Align2::RIGHT_CENTER,
        format::money(p.price),
        t.font(type_scale::MONEY),
        t.p.fg_default,
    );
    let (tone, label) = if p.on_sale {
        (w::Tone::Success, lang.text("เปิดขาย", "On sale"))
    } else {
        (w::Tone::Neutral, lang.text("ปิดขาย", "Not for sale"))
    };
    let (bg, fg) = tone.colors(&t);
    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), t.strong_font(type_scale::CAPTION), fg);
    let r = Rect::from_min_size(
        pos2(x + space::S3, cy - 11.0),
        vec2(g.size().x + 12.0, 22.0),
    );
    ui.painter().rect_filled(r, 4, bg);
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
    let _ = c.status;
}

fn open_editor(app: &mut App, product: Option<Product>) {
    if app.catalog.dirty() {
        app.catalog.pending_open = Some(product);
        return;
    }
    app.catalog.show_errors = false;
    app.catalog.archive_confirm = false;
    let existing = product.is_some();
    app.catalog.editor = Some(product.map_or_else(
        || Editor {
            category: app.catalog.category.clone().unwrap_or_default(),
            ..Editor::default()
        },
        Editor::from,
    ));
    if existing {
        app.refresh();
    }
}
fn save(app: &mut App) {
    let Some(editor) = &app.catalog.editor else {
        return;
    };
    match editor.input() {
        Ok(input) if !input.sku.is_empty() && !input.name.is_empty() => {
            app.catalog.show_errors = false;
            app.send(Command::Save(input));
        }
        _ => app.catalog.show_errors = true,
    }
}

fn inspector(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let service_on = app.data.pricing.service_bp > 0;
    let kitchen_on = !app.data.tables.is_empty();
    let ingredients = app.data.ingredients.clone();
    let Some(editor) = app.catalog.editor.as_mut() else {
        return;
    };
    let dirty = editor.dirty();
    let new = editor.id.is_none();
    let errors = app.catalog.show_errors;
    let busy = app.busy;
    let mut close = false;
    let mut archive = false;
    let mut open_stock: Option<StockDialog> = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 40.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(t.muted(
                if new {
                    lang.text("สินค้าใหม่", "New product")
                } else {
                    lang.text("แก้ไขสินค้า", "Edit product")
                },
                type_scale::LABEL,
            ));
            let title = if editor.name.trim().is_empty() {
                lang.text("ยังไม่มีชื่อ", "Untitled").to_owned()
            } else {
                editor.name.clone()
            };
            w::name_label(ui, &title, type_scale::TITLE, t.p.fg_default);
        });
        ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
            if w::icon_button_sized(ui, Icon::Close, lang.text("ปิด", "Close"), Some(32.0), false)
                .clicked()
            {
                close = true;
            }
        });
    });
    w::divider(ui);
    let footer_h = t.m.control_md + space::S4 * 2.0;
    egui::ScrollArea::vertical()
        .id_salt("inspector")
        .auto_shrink([false, false])
        .max_height(ui.available_height() - footer_h)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = space::S2;
            ui.add_enabled_ui(!busy, |ui| {
                if dirty {
                    w::banner(
                        ui,
                        w::Tone::Warning,
                        if t.m.show_shortcut_hints {
                            lang.text(
                                "มีการแก้ไขที่ยังไม่บันทึก · กด Ctrl S เพื่อบันทึก",
                                "Unsaved changes · press Ctrl S to save",
                            )
                        } else {
                            lang.text("มีการแก้ไขที่ยังไม่บันทึก", "Unsaved changes")
                        },
                        None,
                    );
                }
                w::field_label(ui, lang.text("ชื่อสินค้า", "Name"), None);
                w::text_area(
                    ui,
                    &mut editor.name,
                    lang.text("เช่น ชาไทยเย็น", "e.g. Thai iced tea"),
                    160,
                    2,
                );
                if errors && editor.name.trim().is_empty() {
                    w::inline_error(ui, lang.text("ใส่ชื่อสินค้า", "Enter a product name"));
                }
                ui.add_space(space::S1);
                w::field_label(ui, lang.text("ราคา (บาท)", "Price (THB)"), None);
                let price_bad = errors && format::parse_money(&editor.price).is_err();
                ui.horizontal(|ui| {
                    w::money_input(
                        ui,
                        &mut editor.price,
                        w::MoneyOpts {
                            id: Id::new("product-price"),
                            primary: false,
                            width: Some(ui.available_width().min(200.0)),
                            large: false,
                            invalid: price_bad,
                            enabled: true,
                        },
                    );
                    if let Some(old) = editor.previous_price
                        && format::parse_money(&editor.price).ok() != Some(old)
                    {
                        ui.label(t.muted(
                            format!("{} {}", lang.text("เดิม", "Was"), format::money(old)),
                            type_scale::CAPTION,
                        ));
                    }
                });
                if price_bad {
                    w::inline_error(
                        ui,
                        lang.text(
                            "ราคาต้องไม่ติดลบ ทศนิยมไม่เกิน 2 ตำแหน่ง",
                            "Price must be 0 or more with up to 2 decimals",
                        ),
                    );
                }
                w::field_label(ui, lang.text("ต้นทุนมาตรฐานต่อชิ้น (บาท)", "Standard unit cost (THB)"), Some(lang.text("(ไม่บังคับ)", "(optional)")));
                let cost_bad = errors && !editor.cost.trim().is_empty() && format::parse_money(&editor.cost).is_err();
                w::money_input(ui, &mut editor.cost, w::MoneyOpts {
                    id: Id::new("product-cost"), primary: false, width: Some(ui.available_width()), large: false, invalid: cost_bad, enabled: true,
                });
                w::help_text(ui, lang.text(
                    "เว้นว่าง = ไม่ทราบ · 0 = ไม่มีต้นทุน รวมต้นทุนตัวเลือกไว้ในค่านี้ ไม่รวม VAT ที่ขอคืนได้",
                    "Blank = unknown; 0 = no cost. Include typical option costs; exclude recoverable VAT.",
                ));
                if cost_bad { w::inline_error(ui, lang.text("ต้นทุนต้องไม่ติดลบ และทศนิยมไม่เกิน 2 ตำแหน่ง", "Cost must be 0 or more with up to 2 decimals")); }
                // Full-width row so long bilingual category names are never clipped.
                w::field_label(
                    ui,
                    lang.text("หมวด", "Category"),
                    Some(lang.text("(ไม่บังคับ)", "(optional)")),
                );
                category_input(ui, &mut editor.category, &app.data.counts.categories);
                ui.add_space(space::S1);
                ui.horizontal(|ui| {
                    w::field_label(ui, lang.text("นับสต็อก", "Track stock"), None);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        w::switch(ui, &mut editor.tracked, lang.text("นับสต็อก", "Track stock"));
                    });
                });
                let existing_tracked =
                    editor.id.is_some() && editor.original.as_ref().is_some_and(|o| o.tracked);
                if editor.tracked && existing_tracked {
                    // Stock changes go through receive / remove / count so the
                    // history explains every change (ScreenProducts: "Adjust via Stock count").
                    let current = editor
                        .id
                        .and_then(|id| app.data.products.iter().find(|p| p.id == id))
                        .and_then(|p| p.stock);
                    ui.horizontal(|ui| {
                        ui.label(t.muted(lang.text("คงเหลือ", "In stock"), type_scale::BODY));
                        ui.label(
                            RichText::new(
                                current.map_or("—".into(), |n| format::count(u64::from(n))),
                            )
                            .font(t.strong_font(type_scale::MONEY_LG)),
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
                        for (kind, icon, label) in [
                            (
                                StockKind::Receive,
                                Icon::Plus,
                                lang.text("รับเข้า", "Receive"),
                            ),
                            (StockKind::Remove, Icon::Minus, lang.text("นำออก", "Remove")),
                            (StockKind::Count, Icon::Check, lang.text("ตรวจนับ", "Count")),
                        ] {
                            if w::button(label)
                                .icon(icon)
                                .size(w::Size::Sm)
                                .show(ui)
                                .clicked()
                                && let (Some(id), Some(current)) = (editor.id, current)
                            {
                                open_stock =
                                    Some(StockDialog::new(id, editor.name.clone(), kind, current));
                            }
                        }
                    });
                    stock_history(ui, &t, lang, &app.data.stock_history, editor.id);
                } else if editor.tracked {
                    w::field_label(
                        ui,
                        lang.text("จำนวนเริ่มต้น (ชิ้น)", "Starting stock (units)"),
                        None,
                    );
                    let stock_bad =
                        errors && editor.stock.trim().replace(',', "").parse::<u32>().is_err();
                    w::text_input(
                        ui,
                        &mut editor.stock,
                        w::FieldOpts {
                            hint: "0",
                            limit: 10,
                            invalid: stock_bad,
                            ..Default::default()
                        },
                    );
                    editor.stock.retain(|c| c.is_ascii_digit() || c == ',');
                    if stock_bad {
                        w::inline_error(
                            ui,
                            lang.text("ใส่จำนวนเต็มตั้งแต่ 0", "Enter a whole number, 0 or more"),
                        );
                    }
                    w::help_text(
                        ui,
                        lang.text(
                            "หลังบันทึก ปรับสต็อกด้วยรับเข้า นำออก หรือตรวจนับ",
                            "After saving, change stock with Receive, Remove or Count.",
                        ),
                    );
                } else {
                    w::help_text(
                        ui,
                        lang.text(
                            "สำหรับอาหารที่ทำตามออเดอร์ ไม่ตัดสต็อก",
                            "For made-to-order food. Stock is not deducted.",
                        ),
                    );
                }
                ui.add_space(space::S1);
                w::field_label(ui, lang.text("รหัสสินค้า (SKU)", "Product code (SKU)"), None);
                let sku_bad = errors && editor.sku.trim().is_empty();
                w::text_input(
                    ui,
                    &mut editor.sku,
                    w::FieldOpts {
                        hint: "F-001",
                        limit: 64,
                        invalid: sku_bad,
                        ..Default::default()
                    },
                );
                if errors && editor.sku.trim().is_empty() {
                    w::inline_error(ui, lang.text("ใส่รหัสสินค้า", "Enter a product code"));
                }
                w::field_label(
                    ui,
                    lang.text("บาร์โค้ด", "Barcode"),
                    Some(lang.text("(ไม่บังคับ)", "(optional)")),
                );
                w::text_input(
                    ui,
                    &mut editor.barcode,
                    w::FieldOpts {
                        hint: lang.text("สแกนหรือพิมพ์", "Scan or type"),
                        limit: 64,
                        ..Default::default()
                    },
                );
                ui.add_space(space::S2);
                options_editor(ui, &t, lang, &mut editor.options, errors);
                ui.add_space(space::S2);
                recipe_editor(ui, &t, lang, editor, &ingredients, errors);
                ui.add_space(space::S2);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() - 56.0);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(t.text(lang.text("เปิดขาย", "On sale"), type_scale::BODY_STRONG));
                        w::help_text(
                            ui,
                            lang.text(
                                "ปิดไว้ สินค้ายังอยู่แต่ขายไม่ได้",
                                "When off, the product stays listed but cannot be sold.",
                            ),
                        );
                    });
                    w::switch(ui, &mut editor.on_sale, lang.text("เปิดขาย", "On sale"));
                });
                if service_on {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() - 56.0);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(t.text(
                                lang.text("คิด service charge", "Service charge applies"),
                                type_scale::BODY_STRONG,
                            ));
                            w::help_text(
                                ui,
                                lang.text(
                                    "ปิดสำหรับสินค้าปลีก เช่น น้ำดื่มขวด ขนมซอง",
                                    "Turn off for retail goods such as bottled water.",
                                ),
                            );
                        });
                        w::switch(
                            ui,
                            &mut editor.service_charge,
                            lang.text("คิด service charge", "Service charge applies"),
                        );
                    });
                }
                if kitchen_on {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() - 56.0);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(t.text(
                                lang.text("ส่งเข้าครัว", "Goes to the kitchen"),
                                type_scale::BODY_STRONG,
                            ));
                            w::help_text(
                                ui,
                                lang.text(
                                    "ปิดสำหรับของที่หยิบขายได้เลย เช่น น้ำขวด ขนมซอง",
                                    "Turn off for items served as is, e.g. bottled drinks.",
                                ),
                            );
                        });
                        w::switch(
                            ui,
                            &mut editor.kitchen,
                            lang.text("ส่งเข้าครัว", "Goes to the kitchen"),
                        );
                    });
                }
                if !new {
                    ui.add_space(space::S4);
                    w::divider(ui);
                    if w::button(lang.text("ลบออกจากรายการสินค้า", "Remove from catalog"))
                        .icon(Icon::Trash)
                        .danger_quiet()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                    {
                        archive = true;
                    }
                }
            });
        });
    w::divider(ui);
    ui.add_space(space::S2);
    let mut save_clicked = false;
    let mut discard = false;
    ui.horizontal(|ui| {
        if w::button(if dirty {
            lang.text("ทิ้งการแก้ไข", "Discard")
        } else {
            lang.text("ปิด", "Close")
        })
        .ghost()
        .enabled(!busy)
        .show(ui)
        .clicked()
        {
            discard = true;
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            save_clicked = w::button(lang.text("บันทึก", "Save"))
                .primary()
                .shortcut("Ctrl S")
                .enabled(dirty || new)
                .loading(busy)
                .show(ui)
                .clicked();
        });
    });
    if save_clicked {
        save(app);
    }
    if discard || close {
        if close && app.catalog.dirty() {
            app.catalog.pending_open = Some(None);
            app.catalog.close_after_prompt = true;
        } else {
            app.catalog.editor = None;
        }
    }
    if archive {
        app.catalog.archive_confirm = true;
    }
    if open_stock.is_some() {
        app.catalog.stock_dialog = open_stock;
    }
}
/// Free text with suggestions from existing categories (Combobox).
fn category_input(ui: &mut egui::Ui, value: &mut String, categories: &[(String, u64)]) {
    let id = Id::new("category-input");
    let r = w::text_input(
        ui,
        value,
        w::FieldOpts {
            limit: 80,
            id: Some(id),
            ..Default::default()
        },
    );
    {
        let t = Theme::of(ui);
        crate::ui::icons::paint_sized(
            ui.painter(),
            Icon::ChevronDown,
            pos2(r.rect.right() - 16.0, r.rect.center().y),
            16.0,
            t.p.fg_muted,
        );
    }
    let q = value.trim().to_lowercase();
    let matches: Vec<&String> = categories
        .iter()
        .map(|(c, _)| c)
        .filter(|c| !c.is_empty() && *c != value && (q.is_empty() || c.to_lowercase().contains(&q)))
        .take(8)
        .collect();
    if matches.is_empty() {
        return;
    }
    let t = Theme::of(ui);
    let open = r.has_focus();
    egui::Popup::from_response(&r)
        .open(open)
        .frame(
            t.raised(2)
                .inner_margin(egui::Margin::same(space::S1 as i8)),
        )
        .width(r.rect.width().max(200.0))
        .show(|ui| {
            for c in matches {
                if ui
                    .add(
                        egui::Button::selectable(false, c.as_str())
                            .min_size(vec2(ui.available_width(), t.m.control_sm)),
                    )
                    .clicked()
                {
                    *value = c.clone();
                    ui.memory_mut(|m| m.surrender_focus(id));
                }
            }
        });
}
fn sheet(app: &mut App, ctx: &egui::Context) {
    let t = Theme::current(ctx);
    let screen = ctx.content_rect();
    let width = 440.0_f32.min(screen.width() - 40.0);
    let mut close = false;
    egui::Area::new(Id::new("product-sheet-scrim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0, t.p.overlay_scrim);
            if response.clicked() && !app.catalog.dirty() {
                close = true;
            }
        });
    egui::Area::new(Id::new("product-sheet"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(screen.right() - width, screen.top()))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t.p.bg_raised)
                .shadow(t.shadow(3))
                .inner_margin(egui::Margin::same(space::S4 as i8))
                .show(ui, |ui| {
                    ui.set_min_size(vec2(width - 32.0, screen.height() - 32.0));
                    ui.set_max_size(vec2(width - 32.0, screen.height() - 32.0));
                    inspector(app, ui);
                });
        });
    if close {
        app.catalog.editor = None;
    }
}

pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    crate::import_ui::dialog(app, ctx);
    stock_dialog(app, ctx);
    let lang = app.lang;
    if app.catalog.archive_confirm
        && let Some(editor) = &app.catalog.editor
    {
        let name = editor.name.clone();
        let id = editor.id;
        let r = w::dialog(ctx, "archive-product", 460.0, |ui| {
            w::dialog_title(
                ui,
                &format!(
                    "{} “{}”?",
                    lang.text("ลบออกจากรายการสินค้า", "Remove from catalog"),
                    name
                ),
                true,
            );
            ui.label(lang.text(
                "สินค้าจะหายจากหน้าขายและรายการสินค้า ใบเสร็จเดิมยังอยู่ครบ ถ้าแค่หยุดขายชั่วคราว ให้ปิดสวิตช์ “เปิดขาย” แทน",
                "The product leaves Sell and Products. Past receipts are unchanged. To pause selling, switch off “On sale” instead.",
            ));
            let mut out = None;
            w::dialog_actions(ui, |ui| {
                if w::button(lang.text("ลบออกจากรายการ", "Remove from catalog"))
                    .danger()
                    .enabled(!app.busy)
                    .show(ui)
                    .clicked()
                {
                    out = Some(true);
                }
                if w::button(lang.text("ไม่ลบ", "Keep product"))
                    .ghost()
                    .show(ui)
                    .clicked()
                {
                    out = Some(false);
                }
            });
            out
        });
        match r.inner {
            Some(true) => {
                app.catalog.archive_confirm = false;
                if let Some(id) = id {
                    app.catalog.editor = None;
                    app.send(Command::Archive(id));
                }
            }
            Some(false) => app.catalog.archive_confirm = false,
            None if r.should_close() => app.catalog.archive_confirm = false,
            None => {}
        }
    }
    let leaving = app.catalog.pending_nav.is_some() || app.catalog.pending_open.is_some();
    if leaving {
        let r = w::dialog(ctx, "unsaved-product", 460.0, |ui| {
            w::dialog_title(
                ui,
                lang.text("บันทึกการแก้ไขสินค้าก่อนไหม?", "Save changes to this product?"),
                false,
            );
            ui.label(lang.text(
                "มีการแก้ไขที่ยังไม่บันทึก ถ้าทิ้ง การแก้ไขจะหายไป",
                "You have unsaved edits. Discarding loses them.",
            ));
            let mut out = None;
            w::dialog_actions(ui, |ui| {
                if w::button(lang.text("บันทึก", "Save"))
                    .primary()
                    .show(ui)
                    .clicked()
                {
                    out = Some(0);
                }
                if w::button(lang.text("ทิ้งการแก้ไข", "Discard"))
                    .show(ui)
                    .clicked()
                {
                    out = Some(1);
                }
                if w::button(lang.text("แก้ต่อ", "Keep editing"))
                    .ghost()
                    .show(ui)
                    .clicked()
                {
                    out = Some(2);
                }
            });
            out
        });
        match r.inner {
            Some(0) => {
                save(app);
                app.catalog.pending_nav = None;
                app.catalog.pending_open = None;
                app.catalog.close_after_prompt = false;
            }
            Some(1) => {
                app.catalog.editor = None;
                if let Some(page) = app.catalog.pending_nav.take() {
                    app.force_navigate(page);
                }
                if let Some(next) = app.catalog.pending_open.take()
                    && !std::mem::take(&mut app.catalog.close_after_prompt)
                {
                    open_editor(app, next);
                }
            }
            Some(_) => {
                app.catalog.pending_nav = None;
                app.catalog.pending_open = None;
                app.catalog.close_after_prompt = false;
            }
            None if r.should_close() => {
                app.catalog.pending_nav = None;
                app.catalog.pending_open = None;
                app.catalog.close_after_prompt = false;
            }
            None => {}
        }
    }
    if let Some(mut category) = app.catalog.bulk_category.take() {
        let n = app.catalog.selected.len();
        let categories = app.data.counts.categories.clone();
        let r = w::dialog(ctx, "bulk-category", 440.0, |ui| {
            w::dialog_title(
                ui,
                &format!(
                    "{} {n} {}",
                    lang.text("เปลี่ยนหมวดของ", "Change category of"),
                    lang.noun(n as u64, "รายการ", "item", "items")
                ),
                false,
            );
            w::field_label(ui, lang.text("หมวดใหม่", "New category"), None);
            category_input(ui, &mut category, &categories);
            let mut out = None;
            w::dialog_actions(ui, |ui| {
                if w::button(lang.text("เปลี่ยนหมวด", "Change category"))
                    .primary()
                    .enabled(!app.busy)
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
        match r.inner {
            Some(true) => {
                let ids: Vec<Uuid> = app.catalog.selected.iter().copied().collect();
                app.send(Command::SetCategory(ids, category));
            }
            Some(false) => {}
            None if r.should_close() => {}
            None => app.catalog.bulk_category = Some(category),
        }
    }
}

/// Option groups (sweetness, toppings) with per-choice extra prices.
/// Recipe: ingredients one unit uses, with the recipe cost when every
/// ingredient has a cost ("Use as cost" copies it to the product cost).
fn recipe_editor(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: crate::i18n::Language,
    editor: &mut Editor,
    ingredients: &[pos_domain::Ingredient],
    errors: bool,
) {
    if std::mem::take(&mut editor.scroll_to_recipe) {
        ui.scroll_to_cursor(Some(Align::Min));
    }
    ui.horizontal(|ui| {
        ui.label(t.text(
            lang.text("สูตร (วัตถุดิบต่อ 1 ชิ้น)", "Recipe (per unit sold)"),
            type_scale::BODY_STRONG,
        ));
        ui.label(t.muted(lang.text("(ไม่บังคับ)", "(optional)"), type_scale::LABEL));
    });
    if ingredients.is_empty() {
        w::help_text(
            ui,
            lang.text(
                "เพิ่มวัตถุดิบได้ที่ สินค้า → วัตถุดิบ แล้วกลับมาใส่สูตร ขายแล้วระบบตัดวัตถุดิบให้",
                "Add ingredients in Products → Ingredients first; sales then deduct them.",
            ),
        );
        return;
    }
    let mut remove = None;
    let mut cost = Some(pos_domain::Money::ZERO);
    for (i, (id, amount)) in editor.recipe.iter_mut().enumerate() {
        let found = ingredients.iter().find(|x| x.id == *id);
        let parsed = format::parse_qty(amount).filter(|a| *a > 0);
        cost = match (cost, found.zip(parsed).and_then(|(x, a)| x.cost_of(a))) {
            (Some(sum), Some(c)) => sum.checked_add(c).ok(),
            _ => None,
        };
        ui.horizontal(|ui| {
            let name_w = (ui.available_width() - 190.0).max(90.0);
            ui.allocate_ui_with_layout(
                egui::vec2(name_w, 0.0),
                egui::Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(name_w);
                    w::name_label(
                        ui,
                        found.map_or(
                            lang.text("วัตถุดิบที่ลบแล้ว", "Removed ingredient"),
                            |x| x.name.as_str(),
                        ),
                        type_scale::BODY,
                        t.p.fg_default,
                    );
                },
            );
            w::text_input(
                ui,
                amount,
                w::FieldOpts {
                    hint: "0",
                    limit: 12,
                    width: Some(90.0),
                    invalid: parsed.is_none() && (errors || !amount.is_empty()),
                    ..Default::default()
                },
            );
            ui.label(t.muted(found.map_or("", |x| x.unit.as_str()), type_scale::LABEL));
            if w::icon_button(
                ui,
                Icon::Close,
                lang.text("เอาออกจากสูตร", "Remove from recipe"),
            )
            .clicked()
            {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        editor.recipe.remove(i);
    }
    if editor.recipe.len() < pos_domain::MAX_RECIPE {
        w::text_input(
            ui,
            &mut editor.recipe_search,
            w::FieldOpts {
                hint: lang.text("ค้นหาวัตถุดิบเพื่อเพิ่ม", "Search ingredients to add"),
                limit: 60,
                ..Default::default()
            },
        );
        let q = editor.recipe_search.trim().to_lowercase();
        let choices: Vec<&pos_domain::Ingredient> = ingredients
            .iter()
            .filter(|x| x.active && !editor.recipe.iter().any(|(id, _)| *id == x.id))
            .filter(|x| q.is_empty() || x.name.to_lowercase().contains(&q))
            .take(10)
            .collect();
        ui.horizontal_wrapped(|ui| {
            for x in choices {
                if w::chip(ui, &format!("+ {}", x.name), false, false).clicked() {
                    editor.recipe.push((x.id, String::new()));
                    editor.recipe_search.clear();
                }
            }
        });
    }
    option_recipe_editor(ui, t, lang, editor, ingredients, errors);
    if let Some(cost) = cost.filter(|_| !editor.recipe.is_empty()) {
        ui.horizontal(|ui| {
            ui.label(t.muted(
                format!(
                    "{} {}",
                    lang.text("ต้นทุนตามสูตร", "Recipe cost"),
                    format::baht(cost)
                ),
                type_scale::LABEL,
            ));
            if w::button(lang.text("ใช้เป็นต้นทุน", "Use as cost"))
                .ghost()
                .size(w::Size::Sm)
                .show(ui)
                .clicked()
            {
                editor.cost = format::money(cost);
            }
        });
    }
}
/// "Adjust by option": a choice adds or removes ingredients per unit
/// ("+ fried egg" +1 egg, "no egg" −1); totals never go below zero.
fn option_recipe_editor(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: crate::i18n::Language,
    editor: &mut Editor,
    ingredients: &[pos_domain::Ingredient],
    errors: bool,
) {
    let choices: Vec<(Uuid, String)> = editor
        .options
        .iter()
        .flat_map(|g| g.choices.iter().map(|c| (c.id, c.name.clone())))
        .filter(|(_, name)| !name.trim().is_empty())
        .collect();
    if choices.is_empty() {
        return;
    }
    ui.add_space(space::S1);
    ui.label(t.text(
        lang.text("ปรับตามตัวเลือก", "Adjust by option"),
        type_scale::LABEL,
    ));
    w::help_text(
        ui,
        lang.text(
            "เช่น +ไข่ดาว ใส่ +1 ฟอง · ไม่ใส่ไข่ ใส่ -1 ฟอง (ต่อ 1 ชิ้น)",
            "e.g. + fried egg: +1 egg · no egg: -1 egg (per unit)",
        ),
    );
    let mut remove = None;
    for (i, (choice, ingredient, amount)) in editor.option_recipe.iter_mut().enumerate() {
        let choice_name = choices.iter().find(|(id, _)| id == choice).map_or(
            lang.text("ตัวเลือกที่ลบแล้ว", "Removed choice"),
            |(_, n)| n.as_str(),
        );
        let found = ingredients.iter().find(|x| x.id == *ingredient);
        let parsed = format::parse_signed_qty(amount);
        ui.horizontal(|ui| {
            let name_w = (ui.available_width() - 190.0).max(90.0);
            ui.allocate_ui_with_layout(
                egui::vec2(name_w, 0.0),
                egui::Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(name_w);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    w::name_label(ui, choice_name, type_scale::BODY, t.p.fg_default);
                    ui.label(t.muted(
                        found.map_or(
                            lang.text("วัตถุดิบที่ลบแล้ว", "Removed ingredient"),
                            |x| x.name.as_str(),
                        ),
                        type_scale::CAPTION,
                    ));
                },
            );
            w::text_input(
                ui,
                amount,
                w::FieldOpts {
                    hint: "+1 / -1",
                    limit: 12,
                    width: Some(90.0),
                    invalid: parsed.is_none() && (errors || !amount.is_empty()),
                    ..Default::default()
                },
            );
            ui.label(t.muted(found.map_or("", |x| x.unit.as_str()), type_scale::LABEL));
            if w::icon_button(ui, Icon::Close, lang.text("เอาแถวนี้ออก", "Remove this row")).clicked()
            {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        editor.option_recipe.remove(i);
    }
    // Add a row: pick a choice, then an ingredient.
    ui.horizontal_wrapped(|ui| {
        for (id, name) in &choices {
            if w::chip(ui, name, editor.recipe_choice == Some(*id), false).clicked() {
                editor.recipe_choice = if editor.recipe_choice == Some(*id) {
                    None
                } else {
                    Some(*id)
                };
            }
        }
    });
    if let Some(choice) = editor.recipe_choice {
        let available: Vec<&pos_domain::Ingredient> = ingredients
            .iter()
            .filter(|x| {
                x.active
                    && !editor
                        .option_recipe
                        .iter()
                        .any(|(c, i, _)| *c == choice && *i == x.id)
            })
            .take(12)
            .collect();
        ui.horizontal_wrapped(|ui| {
            for x in available {
                if w::chip(ui, &format!("+ {}", x.name), false, false).clicked() {
                    editor.option_recipe.push((choice, x.id, String::new()));
                    editor.recipe_choice = None;
                }
            }
        });
    }
}
fn options_editor(
    ui: &mut egui::Ui,
    t: &Theme,
    lang: crate::i18n::Language,
    groups: &mut Vec<GroupDraft>,
    errors: bool,
) {
    ui.horizontal(|ui| {
        ui.label(t.text(lang.text("ตัวเลือก", "Options"), type_scale::BODY_STRONG));
        ui.label(t.muted(lang.text("(ไม่บังคับ)", "(optional)"), type_scale::LABEL));
    });
    w::help_text(
        ui,
        lang.text(
            "เช่น ความหวาน หรือท็อปปิ้ง ราคาที่ใส่จะบวกเพิ่มต่อชิ้น",
            "e.g. sweetness or toppings. A price adds to each unit.",
        ),
    );
    let mut remove_group = None;
    for (gi, group) in groups.iter_mut().enumerate() {
        ui.push_id(group.id, |ui| {
            t.sunken().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = space::S2;
                ui.horizontal(|ui| {
                    let bad = errors && group.name.trim().is_empty();
                    w::text_input(
                        ui,
                        &mut group.name,
                        w::FieldOpts {
                            hint: lang.text("ชื่อกลุ่ม เช่น ความหวาน", "Group, e.g. Sweetness"),
                            limit: 80,
                            invalid: bad,
                            width: Some(ui.available_width() - 44.0),
                            ..Default::default()
                        },
                    );
                    if w::icon_button_sized(
                        ui,
                        Icon::Trash,
                        lang.text("ลบกลุ่ม", "Remove group"),
                        None,
                        false,
                    )
                    .clicked()
                    {
                        remove_group = Some(gi);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    w::check_label(ui, &mut group.required, lang.text("ต้องเลือก", "Required"));
                    w::check_label(
                        ui,
                        &mut group.multiple,
                        lang.text("เลือกได้หลายอย่าง", "Allow several"),
                    );
                });
                let mut remove_choice = None;
                let single = group.choices.len() == 1;
                for (ci, choice) in group.choices.iter_mut().enumerate() {
                    ui.push_id(choice.id, |ui| {
                        ui.horizontal(|ui| {
                            let price_w = 104.0;
                            let name_bad = errors && choice.name.trim().is_empty();
                            w::text_input(
                                ui,
                                &mut choice.name,
                                w::FieldOpts {
                                    hint: lang
                                        .text("ตัวเลือก เช่น หวานน้อย", "Choice, e.g. Less sweet"),
                                    limit: 80,
                                    invalid: name_bad,
                                    width: Some(
                                        ui.available_width() - price_w - 44.0 - space::S2 * 2.0,
                                    ),
                                    ..Default::default()
                                },
                            );
                            let bad_price = !choice.price.trim().is_empty()
                                && format::parse_money(&choice.price).is_err();
                            w::money_input(
                                ui,
                                &mut choice.price,
                                w::MoneyOpts {
                                    id: Id::new(("choice-price", choice.id)),
                                    primary: false,
                                    width: Some(price_w),
                                    large: false,
                                    invalid: bad_price,
                                    enabled: true,
                                },
                            );
                            ui.add_enabled_ui(!single, |ui| {
                                if w::icon_button_sized(
                                    ui,
                                    Icon::Close,
                                    lang.text("ลบตัวเลือก", "Remove choice"),
                                    None,
                                    false,
                                )
                                .clicked()
                                {
                                    remove_choice = Some(ci);
                                }
                            });
                        });
                    });
                }
                if let Some(ci) = remove_choice {
                    group.choices.remove(ci);
                }
                if group.choices.len() < 20
                    && w::button(lang.text("เพิ่มตัวเลือก", "Add choice"))
                        .icon(Icon::Plus)
                        .ghost()
                        .size(w::Size::Sm)
                        .show(ui)
                        .clicked()
                {
                    group.choices.push(ChoiceDraft::new());
                }
            });
        });
    }
    if let Some(gi) = remove_group {
        groups.remove(gi);
    }
    if groups.len() < 10
        && w::button(lang.text("เพิ่มกลุ่มตัวเลือก", "Add option group"))
            .icon(Icon::Plus)
            .size(w::Size::Sm)
            .show(ui)
            .clicked()
    {
        groups.push(GroupDraft::new());
    }
}
