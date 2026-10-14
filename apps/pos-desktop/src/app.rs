use crate::{
    i18n::Language,
    sale_ui,
    shell::{self, Toast, Undo},
    ui::{Density, Theme, format, space, theme as ds_theme, w},
    worker::{Command, Data, Event, Query, Worker},
};
use eframe::egui;
use pos_core::{ProductQuery, ProductView};
use pos_domain::{Cart, CartLine, Money, Product, Sale, SaleRecord};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    Settings,
    Sell,
    Products,
    History,
    Reports,
    Shift,
    Activity,
    Tables,
}
impl Page {
    /// Workspace names for `--page` (reproducible layout checks).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "home" => Self::Home,
            "sell" => Self::Sell,
            "history" => Self::History,
            "shift" => Self::Shift,
            "products" => Self::Products,
            "reports" => Self::Reports,
            "settings" => Self::Settings,
            "activity" => Self::Activity,
            "tables" => Self::Tables,
            _ => return None,
        })
    }
}
/// Keeps the current bill in SQLite (pattern 3: autosave, no Save button).
#[derive(Default)]
struct Autosave {
    snapshot: Option<Cart>,
    dirty_since: Option<Instant>,
    saved: Option<Uuid>,
}
pub struct App {
    pub worker: Worker,
    pub auto_lock: crate::auto_lock::State,
    pub auto_backup: crate::auto_backup_ui::State,
    /// Immediate local gate; only a current-generation successful PIN clears it.
    pub session_locked: bool,
    /// Canceled confirmation payload, preserved as an inert draft, never replayed.
    _locked_review: Option<sale_ui::PaymentReview>,
    pub recovery: crate::backup_ui::State,
    pub activity: crate::activity_ui::ActivityState,
    pub data: Data,
    pub known: HashMap<Uuid, Product>,
    pub cart: Cart,
    pub cash: String,
    pub payment_method: pos_domain::PaymentMethod,
    pub payment_review: Option<sale_ui::PaymentReview>,
    pub lang: Language,
    pub look: ds_theme::Look,
    pub density: Density,
    pub fullscreen: bool,
    window: crate::window::WindowFit,
    pub shop: crate::shop::ShopSettings,
    pub shop_draft: crate::shop::ShopSettings,
    pub onboarding: bool,
    pub shop_readonly: bool,
    pub print: crate::print_ui::PrintState,
    /// Receipt logo (1-bit), loaded from receipt-logo.png beside the database.
    pub logo: Option<crate::print::render::Bitmap>,
    pub staff: crate::staff_ui::StaffState,
    pub tax: crate::tax_ui::TaxState,
    pub tables: crate::tables_ui::TablesState,
    pub templates: crate::templates_ui::TemplatesState,
    pub settings_section: crate::settings_ui::Section,
    pub promotions: crate::promo_ui::PromoState,
    pub ingredients: crate::ingredients_ui::IngredientsState,
    pub purchasing: crate::purchasing_ui::PurchasingState,
    pub categories: crate::categories_ui::CategoriesState,
    pub option_sets: crate::option_sets_ui::OptionSetsState,
    pub images: crate::images::ImageCache,
    /// Pictures on Sell tiles (device preference, `product-images.txt`).
    pub tile_images: bool,
    pub kitchen: crate::kitchen_ui::KitchenState,
    /// The kitchen screen window is open.
    pub kitchen_open: bool,
    pub pricing_draft: Option<pos_domain::PricingSettings>,
    /// The last command sent, replayed with a manager's approval if refused.
    last_command: Option<Command>,
    /// Reload catalog, purchase list, then receiving history one foreground command at a time.
    purchase_followup: Option<Uuid>,
    pub ctx: egui::Context,
    home_refreshed: Instant,
    pub page: Page,
    pub busy: bool,
    pub ready: bool,
    pub fatal: bool,
    pub error: Option<String>,
    pub search: String,
    pub search_changed: Option<Instant>,
    pub sell: sale_ui::SellState,
    pub catalog: crate::catalog_ui::CatalogState,
    pub history: crate::history_ui::HistoryState,
    pub home: crate::home_ui::HomeCache,
    pub drawer: crate::shift_ui::Editor,
    pub last_sale: Option<Sale>,
    pub receipt: Option<SaleRecord>,
    pub void_editor: Option<crate::history_ui::VoidEditor>,
    pub report: crate::report_ui::Period,
    pub report_group: crate::report_ui::Group,
    pub report_metric: crate::report_ui::Metric,
    pub data_dir: PathBuf,
    pub demo: bool,
    pub toasts: Vec<Toast>,
    pub palette: shell::Palette,
    pub display_open: bool,
    display_dirty: bool,
    pub held_delete: Option<Uuid>,
    close_confirm: bool,
    allow_exit: bool,
    /// Set when closing waits for the open bill to reach SQLite.
    exit_after_save: Option<Instant>,
    autosave: Autosave,
    restore: Option<Uuid>,
    pub restoring_bill: bool,
    screenshot: Option<PathBuf>,
    /// Debug-only overlay preset for reproducible screenshots (`--preview`).
    pub preview: Option<String>,
    screenshot_requested: bool,
    pub frames: u32,
    /// Frame on which a preview preset was applied; captures wait for fades.
    pub preview_frame: u32,
    #[cfg(debug_assertions)]
    pub preview_scroll_bottom: bool,
}
const CURRENT_BILL: &str = "current-bill.txt";
/// Widest layout for Home, Shift, Reports and Settings (points).
const READING_WIDTH: f32 = 1440.0;
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        data_dir: PathBuf,
        demo: bool,
        screenshot: Option<PathBuf>,
        start_page: Option<Page>,
    ) -> std::io::Result<Self> {
        let read = |name: &str| std::fs::read_to_string(data_dir.join(name)).unwrap_or_default();
        let look = ds_theme::Look {
            accent: ds_theme::accent_from_saved(&read("accent.txt")),
            shape: ds_theme::shape_from_saved(&read("shape.txt")),
        };
        let density = ds_theme::density_from_saved(&read("density.txt"));
        let fullscreen = crate::window::fullscreen_from_saved(&read("window.txt"));
        ds_theme::install_fonts(&cc.egui_ctx);
        ds_theme::apply(&cc.egui_ctx, look, density);
        let lang = if read("language.txt") == "en" {
            Language::English
        } else {
            Language::Thai
        };
        let existing = data_dir
            .join("pos-heart.sqlite3")
            .metadata()
            .is_ok_and(|m| m.len() > 0);
        let (shop, settings_error) = match crate::shop::ShopSettings::load(&data_dir) {
            Ok(Some(shop)) => (shop, None),
            Ok(None) => (
                crate::shop::ShopSettings {
                    setup_complete: existing || demo,
                    ..Default::default()
                },
                None,
            ),
            Err(e) => (
                crate::shop::ShopSettings {
                    setup_complete: true,
                    ..Default::default()
                },
                Some(format!(
                    "{}: {e}",
                    lang.text(
                        "อ่านข้อมูลร้านไม่ได้ ไฟล์ shop.json จะไม่ถูกเขียนทับ",
                        "Cannot read shop.json. This file will not be overwritten"
                    )
                )),
            ),
        };
        let shop_failed = settings_error.is_some();
        let (print, print_error) = crate::print_ui::PrintState::load(&data_dir);
        let settings_error = settings_error.or(print_error.map(|e| {
            format!(
                "{}: {e}",
                lang.text(
                    "อ่านการตั้งค่าเครื่องพิมพ์ไม่ได้ ไฟล์ printer.json จะไม่ถูกเขียนทับ",
                    "Cannot read printer.json. This file will not be overwritten"
                )
            )
        }));
        let auto_lock = crate::auto_lock::State::load(&data_dir);
        let settings_error = settings_error.or_else(|| {
            auto_lock.error.as_ref().map(|error| {
                format!(
                    "{}: {error}",
                    lang.text(
                        "อ่านการตั้งค่าล็อกอัตโนมัติไม่ได้ ไฟล์จะไม่ถูกเขียนทับ",
                        "Cannot read auto-lock settings; the file will not be overwritten"
                    )
                )
            })
        });
        let onboarding = !shop.setup_complete;
        let page = start_page.unwrap_or(if shop.start_in_sell && !onboarding {
            Page::Sell
        } else {
            Page::Home
        });
        let restore = Uuid::parse_str(read(CURRENT_BILL).trim()).ok();
        let worker = Worker::start(data_dir.join("pos-heart.sqlite3"), cc.egui_ctx.clone())?;
        Ok(Self {
            worker,
            auto_lock,
            auto_backup: crate::auto_backup_ui::State::load(&data_dir),
            session_locked: false,
            _locked_review: None,
            recovery: crate::backup_ui::State::default(),
            activity: crate::activity_ui::ActivityState::default(),
            data: Data::default(),
            known: HashMap::new(),
            cart: Cart::new(),
            cash: String::new(),
            payment_method: pos_domain::PaymentMethod::Cash,
            payment_review: None,
            lang,
            look,
            density,
            fullscreen,
            window: crate::window::WindowFit::default(),
            page,
            shop_draft: shop.clone(),
            shop,
            onboarding,
            shop_readonly: shop_failed,
            print,
            logo: crate::print::template::load_logo(&data_dir),
            staff: crate::staff_ui::StaffState::default(),
            tax: crate::tax_ui::TaxState::default(),
            tables: crate::tables_ui::TablesState::default(),
            templates: crate::templates_ui::TemplatesState::default(),
            settings_section: crate::settings_ui::Section::default(),
            promotions: crate::promo_ui::PromoState::default(),
            ingredients: crate::ingredients_ui::IngredientsState::default(),
            purchasing: crate::purchasing_ui::PurchasingState::default(),
            categories: crate::categories_ui::CategoriesState::default(),
            option_sets: crate::option_sets_ui::OptionSetsState::default(),
            images: crate::images::ImageCache::default(),
            tile_images: read("product-images.txt").trim() != "off",
            kitchen: crate::kitchen_ui::KitchenState::default(),
            kitchen_open: false,
            pricing_draft: None,
            last_command: None,
            purchase_followup: None,
            ctx: cc.egui_ctx.clone(),
            home_refreshed: Instant::now(),
            busy: true,
            ready: false,
            fatal: false,
            error: settings_error,
            search: String::new(),
            search_changed: None,
            sell: sale_ui::SellState::default(),
            catalog: crate::catalog_ui::CatalogState::default(),
            history: crate::history_ui::HistoryState::default(),
            home: crate::home_ui::HomeCache::default(),
            drawer: crate::shift_ui::Editor::default(),
            last_sale: None,
            receipt: None,
            void_editor: None,
            report: crate::report_ui::Period::day(crate::worker::today()),
            report_group: crate::report_ui::Group::Products,
            report_metric: crate::report_ui::Metric::Amount,
            data_dir,
            demo,
            toasts: Vec::new(),
            palette: shell::Palette::default(),
            display_open: false,
            display_dirty: false,
            held_delete: None,
            close_confirm: false,
            allow_exit: false,
            exit_after_save: None,
            autosave: Autosave::default(),
            restore,
            restoring_bill: false,
            screenshot,
            preview: None,
            screenshot_requested: false,
            frames: 0,
            preview_frame: 0,
            #[cfg(debug_assertions)]
            preview_scroll_bottom: false,
        })
    }
    pub fn send(&mut self, command: Command) {
        if self.busy
            || self.fatal
            || (crate::staff_ui::locked(self)
                && !matches!(&command, Command::SignIn(..) | Command::Refresh(_)))
        {
            return;
        }
        self.last_command = Some(command.clone());
        match self.worker.send(command) {
            true => {
                self.busy = true;
                self.error = None;
            }
            false => {
                self.error = Some(
                    self.lang
                        .text(
                            "ระบบยังไม่พร้อม กรุณาปิดแล้วเปิดแอปใหม่",
                            "Worker unavailable. Close and reopen the app.",
                        )
                        .into(),
                )
            }
        }
    }
    /// Never blocks the UI; a full queue simply retries on the next change.
    /// Kitchen screen actions never block the counter.
    pub fn kitchen_command(&mut self, command: Command) {
        if crate::staff_ui::locked(self) {
            return;
        }
        if !self.send_background(command) {
            self.toast(self.lang.text("ระบบไม่ว่าง ลองอีกครั้ง", "Busy, try again"));
        }
    }
    /// Leaves the open bill held (it is autosaved) and starts a fresh one,
    /// without the autosave discarding the parked bill.
    pub fn park_cart(&mut self) {
        if !self.cart.lines.is_empty() {
            let current = self.cart.clone();
            self.send_background(Command::Autosave(current));
        }
        self.cart = Cart::new();
        self.reset_payment();
        self.sell.current = None;
        self.autosave = Autosave::default();
        self.clear_marker();
    }
    /// Saves the open bill now (background), e.g. after moving its table.
    pub fn autosave_now(&mut self) {
        let current = self.cart.clone();
        self.send_background(Command::Autosave(current));
    }
    /// Sends the bill's new and cancelled dishes to the kitchen.
    pub fn send_kitchen(&mut self) {
        if !self.busy && (!self.cart.lines.is_empty() || !self.cart.sent.is_empty()) {
            self.send(Command::SendKitchen(self.cart.clone()));
        }
    }
    pub(crate) fn send_background(&mut self, command: Command) -> bool {
        !self.fatal && self.worker.send(command)
    }
    /// Manual and idle locks use the same immediate gate and queue revocation.
    pub fn lock_session(&mut self) {
        if !self.data.staff_enabled {
            return;
        }
        self.session_locked = true;
        self.data.signed_in = None;
        self.worker.revoke();
        self.auto_lock.reset(Instant::now());
        self.staff.clear_pin_drafts();
        self.staff.pin_error = None;
        self.staff.approval = None;
        self.last_command = None;
        if self.payment_review.is_some() {
            self._locked_review = self.payment_review.take();
        }
        self.void_editor = None;
        self.history.refund_editor = None;
        self.held_delete = None;
        self.catalog.cancel_confirmations();
        self.purchasing.cancel_confirmations();
        self.tax.invoice = None;
        self.drawer.pending = None;
        self.drawer.reviewing = false;
        self.sell.confirm_large_cash = false;
        self.sell.large_cash_ok = false;
        self.sell.pay_requested = false;
        self.sell.hold_requested = false;
        self.sell.kitchen_requested = false;
        self.sell.discount_requested = None;
        self.sell.remove_requested = None;
        self.sell.edit_requested = None;
        self.sell.bill_split = None;
        self.recovery.confirm = false;
        self.palette = shell::Palette::default();
        self.display_open = false;
        self.close_confirm = false;
        self.ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
        egui::Popup::close_all(&self.ctx);
        self.ctx.request_repaint();
    }
    /// Call before handling actual input in each native viewport.
    pub fn idle_input(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let due = ctx.input(|input| {
            self.auto_lock.poll(
                now,
                self.data.staff_enabled,
                self.data.signed_in.is_some() && !self.session_locked,
                &input.events,
            )
        });
        if due {
            self.lock_session();
            // The event that encountered expiry belongs to the old session.
            // Do not let it type a PIN or click a lock-screen control.
            ctx.input_mut(|input| {
                input.events.clear();
            });
        }
        if let Some(remaining) = self.auto_lock.remaining(now) {
            ctx.request_repaint_after(remaining);
        }
    }
    pub fn query(&self) -> Query {
        let products = match self.page {
            Page::Products => ProductQuery {
                search: self.catalog.search.clone(),
                category: self.catalog.category.clone(),
                view: self.catalog.view,
                offset: self.catalog.offset,
                limit: 100,
            },
            _ => ProductQuery {
                search: self.search.clone(),
                category: self.sell.category.clone(),
                view: ProductView::All,
                offset: 0,
                limit: self.sell.limit(),
            },
        };
        Query {
            products,
            sale_offset: self.history.offset,
            report_start: self.report.start,
            report_end: self.report.end,
            shift_offset: self.drawer.history_offset,
            movement_shift: self.drawer.selected.as_ref().map(|s| s.id),
            movement_offset: self.drawer.movement_offset,
            stock_product: (self.page == Page::Products)
                .then(|| self.catalog.editor.as_ref().and_then(|e| e.id))
                .flatten(),
        }
    }
    pub fn refresh(&mut self) {
        let query = self.query();
        self.send(Command::Refresh(query));
    }
    /// Line total including option prices, as checkout will compute it.
    pub fn price(&self, line: &CartLine) -> Option<Money> {
        self.known
            .get(&line.product_id)
            .and_then(|p| p.unit_price(&line.options).ok())
            .and_then(|unit| unit.checked_mul(line.quantity).ok())
    }
    /// The whole bill priced as checkout will: options, line and bill discounts.
    pub fn bill_price(&self) -> Result<pos_domain::BillPrice, pos_domain::DomainError> {
        self.price_cart(&self.cart)
    }
    /// Promotions for this bill right now, exactly as checkout applies them.
    pub fn promoted(&self, cart: &Cart) -> Result<pos_domain::Promoted, pos_domain::DomainError> {
        let mut lines = Vec::with_capacity(cart.lines.len());
        for line in &cart.lines {
            let product = self
                .known
                .get(&line.product_id)
                .ok_or(pos_domain::DomainError::MissingProductFields)?;
            lines.push(pos_domain::PromoLine {
                product_id: product.id,
                category: &product.category,
                gross: product
                    .unit_price(&line.options)?
                    .checked_mul(line.quantity)?,
                quantity: line.quantity,
                manual: line.discount,
            });
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        pos_domain::apply_promotions(
            &lines,
            cart.discount,
            &self.data.promotions,
            &pos_domain::LocalTime::bangkok(now),
            &cart.coupons,
        )
    }
    pub fn price_cart(
        &self,
        cart: &Cart,
    ) -> Result<pos_domain::BillPrice, pos_domain::DomainError> {
        let promoted = self.promoted(cart)?;
        let lines = cart
            .lines
            .iter()
            .zip(&promoted.line_discounts)
            .map(|(line, discount)| {
                let product = self
                    .known
                    .get(&line.product_id)
                    .ok_or(pos_domain::DomainError::MissingProductFields)?;
                Ok((
                    product
                        .unit_price(&line.options)?
                        .checked_mul(line.quantity)?,
                    *discount,
                    product.service_charge,
                ))
            })
            .collect::<Result<Vec<_>, pos_domain::DomainError>>()?;
        // Same options checkout uses: rounding only when paying cash.
        let options = pos_domain::PriceOptions::for_bill(
            &self.data.pricing,
            &self.data.tax,
            cart.service_charge,
            // Split payments are exact: no cash rounding.
            self.payment_method == pos_domain::PaymentMethod::Cash && self.sell.split_pay.is_none(),
        );
        pos_domain::price_sale(&lines, promoted.bill, &options)
    }
    pub fn total(&self) -> Result<Money, pos_domain::DomainError> {
        self.bill_price().map(|p| p.total)
    }
    /// Held bills other than the one being edited (which is autosaved as held too).
    pub fn held_bills(&self) -> Vec<Cart> {
        self.data
            .held
            .iter()
            .filter(|c| c.id != self.cart.id)
            .cloned()
            .collect()
    }
    /// Tap / Enter on a product: products with options open the options sheet first.
    pub fn add(&mut self, p: Product) {
        if p.stock == Some(0) || !p.on_sale || !p.active {
            return;
        }
        if !p.options.is_empty() {
            self.sell.sheet = Some(sale_ui::OptionSheet::new(p.clone(), None));
            self.known.insert(p.id, p);
            return;
        }
        self.add_line(p, CartLine::default());
    }
    /// Adds `line` (quantity ≥ 1), merging with an identical existing line.
    pub fn add_line(&mut self, p: Product, mut line: CartLine) {
        self.last_sale = None;
        line.product_id = p.id;
        line.quantity = line.quantity.max(1);
        let index = if let Some(index) = self.cart.lines.iter().position(|l| {
            l.product_id == p.id
                && l.note == line.note
                && l.options == line.options
                && l.discount == line.discount
        }) {
            let existing = &mut self.cart.lines[index];
            existing.quantity = existing.quantity.saturating_add(line.quantity).min(999_999);
            index
        } else {
            self.cart.lines.push(line);
            self.cart.lines.len() - 1
        };
        self.sell.current = Some(index);
        // Adding a dish goes back to ordering.
        self.sell.paying = None;
        self.sell.flash = Some((index, Instant::now()));
        self.sell.scroll_to_current = true;
        self.sell.tab = sale_ui::CartTab::Current;
        self.known.insert(p.id, p);
    }
    pub fn remove_line(&mut self, index: usize) {
        if index >= self.cart.lines.len() {
            return;
        }
        let line = self.cart.lines.remove(index);
        let name = self
            .known
            .get(&line.product_id)
            .map_or_else(String::new, |p| p.name.clone());
        self.sell.current = if self.cart.lines.is_empty() {
            None
        } else {
            Some(index.min(self.cart.lines.len() - 1))
        };
        self.toast_undo(
            format!("{} {name}", self.lang.text("นำออกแล้ว:", "Removed:")),
            Undo::RestoreLine(index, line),
        );
    }
    pub fn reset_payment(&mut self) {
        self.cash.clear();
        self.payment_method = pos_domain::PaymentMethod::Cash;
        self.sell.split_pay = None;
        self.payment_review = None;
    }
    pub fn clear_cart(&mut self) {
        if !self.cart.sent.is_empty() {
            // The kitchen already has these dishes: tell it, then discard.
            let mut empty = self.cart.clone();
            empty.lines.clear();
            self.send(Command::SendKitchen(empty));
            self.cart = Cart::new();
            self.reset_payment();
            self.sell.current = None;
            self.autosave = Autosave::default();
            self.clear_marker();
            return;
        }
        let old = std::mem::take(&mut self.cart);
        self.reset_payment();
        self.sell.current = None;
        if !old.lines.is_empty() {
            self.toast_undo(
                self.lang.text("ล้างบิลแล้ว", "Bill cleared").to_owned(),
                Undo::RestoreCart(old),
            );
        }
    }
    pub fn hold_cart(&mut self) {
        if !self.cart.lines.is_empty() && !self.busy {
            self.send(Command::Hold(self.cart.clone()));
        }
    }
    pub fn resume(&mut self, id: Uuid) {
        // The open bill is already autosaved as a held bill; flush it first so
        // switching bills never loses edits (the worker runs commands in order).
        if !self.cart.lines.is_empty() {
            let current = self.cart.clone();
            self.send_background(Command::Autosave(current));
        }
        self.send(Command::Resume(id));
    }
    fn events(&mut self) {
        self.print_events();
        while let Ok(envelope) = self.worker.rx.try_recv() {
            let stale = envelope.generation != self.worker.generation();
            let event = envelope.event;
            // Background replies never complete a foreground command.
            if !envelope.background {
                self.busy = false;
            }
            if stale
                && matches!(
                    &event,
                    Event::SignedIn(_) | Event::SignedOut | Event::Failed(_)
                )
            {
                continue;
            }
            // A finished restricted command closes its approval dialog.
            if !envelope.background
                && !matches!(
                    event,
                    Event::Quiet | Event::Restored(..) | Event::Failed(_) | Event::Kitchen(_)
                )
            {
                self.staff.approval = None;
            }
            match event {
                Event::Ready(mut data) => {
                    if stale || self.session_locked {
                        data.signed_in = None;
                    }
                    for p in data.products.iter() {
                        self.known.insert(p.id, p.clone());
                    }
                    self.data = *data;
                    self.sell.loading_more = false;
                    if !self.ready {
                        self.ready = true;
                        #[cfg(debug_assertions)]
                        crate::preview::arm(self);
                        if self.demo && self.data.counts.total == 0 {
                            self.send(Command::Demo);
                        }
                        if let Some(id) = self.restore.take() {
                            self.restoring_bill = self.send_background(Command::Restore(id));
                        }
                    }
                    if self.purchase_followup.is_some() {
                        self.send(Command::LoadPurchases(self.purchasing.offset));
                    }
                }
                Event::Quiet => {
                    self.restoring_bill = false;
                    if self.exit_after_save.is_some() && self.autosave.dirty_since.is_none() {
                        self.allow_exit = true;
                    }
                }
                Event::Restored(cart, products) => {
                    self.restoring_bill = false;
                    if self.cart.lines.is_empty() {
                        for p in products {
                            self.known.insert(p.id, p);
                        }
                        self.autosave.saved = Some(cart.id);
                        self.autosave.snapshot = Some(cart.clone());
                        self.cart = cart;
                    }
                }
                Event::ShiftOpened => {
                    self.drawer.opening.clear();
                    self.drawer.open_id = Uuid::new_v4();
                    self.toast(
                        self.lang
                            .text("เปิดกะแล้ว พร้อมรับเงิน", "Shift opened. Ready to take payment."),
                    );
                    self.refresh();
                }
                Event::CashMoved => {
                    self.drawer.amount.clear();
                    self.drawer.reason.clear();
                    self.drawer.move_id = Uuid::new_v4();
                    self.drawer.pending = None;
                    self.toast(self.lang.text("บันทึกเงินสดแล้ว", "Cash movement recorded."));
                    self.refresh();
                }
                Event::ShiftClosed(shift) => {
                    crate::shift_ui::shift_closed(self, &shift);
                    self.refresh();
                }
                Event::Saved(product) => {
                    crate::catalog_ui::saved(self, *product);
                    self.refresh();
                }
                Event::Archived => {
                    self.toast(self.lang.text(
                        "ลบสินค้าออกจากรายการแล้ว ใบเสร็จเดิมไม่เปลี่ยน",
                        "Product removed from the catalog. Past receipts are unchanged.",
                    ));
                    self.refresh();
                }
                Event::StockAdjusted(movement) => {
                    crate::catalog_ui::stock_adjusted(self, &movement);
                    self.refresh();
                }
                Event::Bulk(n) => {
                    crate::catalog_ui::bulk_done(self, n);
                    self.refresh();
                }
                Event::Paid(sale) => {
                    // A table is free again: show the floor for the next one.
                    if self.cart.table.is_some() {
                        self.sell.pane = sale_ui::Pane::Tables;
                    }
                    self.after_payment(&sale);
                    self.cart = Cart::new();
                    self.reset_payment();
                    self.sell.current = None;
                    self.autosave = Autosave::default();
                    self.clear_marker();
                    self.last_sale = Some(sale);
                    self.refresh();
                }
                Event::Voided(cancellation) => {
                    if let Some(record) = &mut self.receipt
                        && record.sale.id == cancellation.sale_id
                    {
                        record.cancellation = Some(cancellation.clone());
                    }
                    if let Some(record) = &mut self.history.selected
                        && record.sale.id == cancellation.sale_id
                    {
                        record.cancellation = Some(cancellation.clone());
                    }
                    if self
                        .last_sale
                        .as_ref()
                        .is_some_and(|s| s.id == cancellation.sale_id)
                    {
                        self.last_sale = None;
                    }
                    self.void_editor = None;
                    self.toast(format!(
                        "{} · {} {}",
                        self.lang
                            .text("ยกเลิกบิลแล้ว บันทึกคืนเงิน", "Sale voided. Refund recorded:"),
                        self.lang.payment_method(cancellation.refund_method),
                        format::baht(cancellation.refund)
                    ));
                    self.refresh();
                }
                Event::Refunded(refund) => {
                    for record in [&mut self.receipt, &mut self.history.selected]
                        .into_iter()
                        .flatten()
                    {
                        if record.sale.id == refund.sale_id
                            && !record.refunds.iter().any(|r| r.id == refund.id)
                        {
                            record.refunds.push(refund.clone());
                        }
                    }
                    self.history.refund_editor = None;
                    self.toast(format!(
                        "{} · {} {}",
                        self.lang.text("คืนสินค้าแล้ว บันทึกคืนเงิน", "Items refunded:"),
                        self.lang.payment_method(refund.refund_method),
                        format::baht(refund.refund)
                    ));
                    self.refresh();
                }
                Event::Split(rest, moved) => {
                    let lang = self.lang;
                    let place = match rest
                        .table
                        .and_then(|id| self.data.tables.iter().find(|t| t.id == id))
                    {
                        Some(t) => format!(
                            "{} {}",
                            lang.text("บิลเดิมยังอยู่ที่โต๊ะ", "the rest stays on"),
                            t.name
                        ),
                        None => lang
                            .text("บิลเดิมอยู่ในแท็บพักไว้", "the rest is in Held")
                            .to_owned(),
                    };
                    self.autosave.saved = Some(moved.id);
                    self.autosave.snapshot = Some(moved.clone());
                    self.cart = moved;
                    self.write_marker();
                    self.sell.tab = sale_ui::CartTab::Current;
                    self.sell.pane = sale_ui::Pane::Products;
                    self.sell.current = None;
                    self.sell.bill_split = None;
                    self.reset_payment();
                    self.last_sale = None;
                    self.toast(format!(
                        "{} · {place}",
                        lang.text(
                            "แยกบิลแล้ว รับเงินบิลนี้ได้เลย",
                            "Bill split. Take payment for this one;"
                        )
                    ));
                    self.refresh();
                }
                Event::Resumed(cart, products) => {
                    for p in products {
                        self.known.insert(p.id, p);
                    }
                    self.autosave.saved = Some(cart.id);
                    self.autosave.snapshot = Some(cart.clone());
                    self.cart = cart;
                    self.write_marker();
                    self.page = Page::Sell;
                    self.sell.tab = sale_ui::CartTab::Current;
                    self.sell.current = None;
                    self.reset_payment();
                    self.last_sale = None;
                    self.refresh();
                }
                Event::Held(id) => {
                    let table = self.cart.table.is_some();
                    if table {
                        self.sell.pane = sale_ui::Pane::Tables;
                    }
                    self.cart = Cart::new();
                    self.reset_payment();
                    self.sell.current = None;
                    self.autosave = Autosave::default();
                    self.clear_marker();
                    self.toast_undo(
                        if table {
                            self.lang.text(
                                "เก็บบิลโต๊ะแล้ว · แตะโต๊ะเพื่อเปิดต่อ",
                                "Table bill kept · tap the table to reopen",
                            )
                        } else {
                            self.lang.text(
                                "พักบิลแล้ว · เปิดคืนได้จากแท็บพักไว้",
                                "Bill held · reopen it from Held",
                            )
                        },
                        Undo::ResumeHeld(id),
                    );
                    self.refresh();
                }
                Event::Found(code, product) => match product {
                    Some(p) if p.on_sale && p.stock != Some(0) => {
                        let name = p.name.clone();
                        self.add(*p);
                        self.search.clear();
                        self.search_changed = Some(Instant::now());
                        self.toast(format!("{} {name}", self.lang.text("เพิ่ม", "Added")));
                    }
                    Some(p) => self.toast(format!(
                        "{} · {}",
                        p.name,
                        if p.on_sale {
                            self.lang.text("สินค้าหมด", "Out of stock")
                        } else {
                            self.lang.text("ปิดขายอยู่", "Not for sale")
                        }
                    )),
                    None => self.toast(format!(
                        "{} “{code}”",
                        self.lang.text("ไม่พบรหัส", "No product with code")
                    )),
                },
                Event::Removed | Event::Demo => self.refresh(),
                Event::Activity(events) => self.activity.events = events,
                Event::ImportPreview(preview) => {
                    if let Some(state) = &mut self.catalog.import {
                        state.preview = Some(preview);
                    }
                }
                Event::ProductsImported(count) => {
                    self.catalog.import = None;
                    self.toast(format!(
                        "{} {count}",
                        self.lang.text("นำเข้าสินค้าแล้ว", "Products imported:")
                    ));
                    self.refresh();
                }
                Event::AutoBackupSaved(settings) => {
                    self.auto_backup.saved(settings);
                    self.toast(
                        self.lang
                            .text("บันทึกการสำรองอัตโนมัติแล้ว", "Automatic backup settings saved."),
                    );
                }
                Event::AutoBackupResult(result) => {
                    let text = match result {
                        Ok(outcome) => {
                            self.auto_backup.latest = outcome
                                .latest
                                .and_then(|snapshot| u64::try_from(snapshot.created_at).ok())
                                .and_then(|seconds| {
                                    std::time::UNIX_EPOCH
                                        .checked_add(std::time::Duration::from_secs(seconds))
                                });
                            if let Some(warning) = outcome.warning {
                                self.error = Some(warning.clone());
                                warning
                            } else if let Some(path) = outcome.created {
                                format!(
                                    "{} {}",
                                    self.lang.text("สำรองอัตโนมัติแล้ว:", "Automatic backup saved:"),
                                    path.display()
                                )
                            } else {
                                self.lang
                                    .text(
                                        "ตรวจแล้ว ไม่มีชุดสำรองใหม่ที่ต้องสร้าง",
                                        "Checked; no new backup was needed.",
                                    )
                                    .into()
                            }
                        }
                        Err(error) => {
                            let message = format!("{}: {error}", self.lang.text("สำรองอัตโนมัติไม่สำเร็จ ข้อมูลที่บันทึกแล้วยังอยู่ ลองใหม่ในตั้งค่า → ข้อมูลและการสำรอง", "Automatic backup failed; saved records are intact. Retry in Settings → Data & backup"));
                            self.error = Some(message.clone());
                            message
                        }
                    };
                    self.auto_backup.last_result = Some(text);
                }
                Event::BackupInspected(review) => self.recovery.review = Some(review),
                Event::RecoveryPending(previous) => {
                    self.recovery.confirm = false;
                    self.recovery.pending = Some(previous);
                    self.fatal = true;
                }
                Event::Backup(path) | Event::Exported(path) => self.toast(format!(
                    "{} {}",
                    self.lang.text("บันทึกไฟล์แล้ว:", "File saved:"),
                    path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned()
                    )
                )),
                Event::SignedIn(me) => {
                    self.session_locked = false;
                    self.auto_lock.reset(Instant::now());
                    self.staff.clear_pin_drafts();
                    self.staff.pin_error = None;
                    self.toast(format!("{} {}", self.lang.text("สวัสดี", "Hello,"), me.name));
                    self.data.signed_in = Some(me);
                    self.refresh();
                }
                Event::SignedOut => {
                    self.data.signed_in = None;
                }
                Event::StaffSaved(member) => {
                    self.staff.editor = None;
                    self.toast(format!(
                        "{} {}",
                        self.lang.text("บันทึกพนักงานแล้ว:", "Staff saved:"),
                        member.name
                    ));
                    self.refresh();
                }
                Event::ProductImage(id, image) => {
                    let ctx = self.ctx.clone();
                    self.image_loaded(&ctx, id, image);
                }
                Event::ImageSaved => {
                    self.toast(self.lang.text("บันทึกรูปแล้ว", "Picture saved."));
                    self.refresh();
                }
                Event::OptionSetsSaved => {
                    self.option_sets.editor = None;
                    self.option_sets.delete = None;
                    self.option_sets.bulk = None;
                    self.toast(self.lang.text("บันทึกชุดตัวเลือกแล้ว", "Option sets saved."));
                    self.refresh();
                }
                Event::CategoriesSaved => {
                    self.categories.rename = None;
                    self.categories.delete = None;
                    self.refresh();
                }
                Event::Purchases {
                    suppliers,
                    orders,
                    offset,
                } => {
                    self.purchasing.suppliers = suppliers;
                    self.purchasing.orders = orders;
                    self.purchasing.offset = offset;
                    self.purchasing.loaded = true;
                    if !crate::staff_ui::locked(self)
                        && let Some(id) = self.purchase_followup.take()
                    {
                        self.send(Command::PurchaseDetail(id, 0));
                    }
                }
                Event::PurchaseProducts(query, products) => {
                    if self.purchasing.product_query == query {
                        self.purchasing.products = products;
                    }
                }
                Event::PurchaseDetail {
                    order,
                    receipts,
                    offset,
                } => self.purchasing.set_detail(order, receipts, offset),
                Event::SupplierSaved(supplier) => {
                    self.purchasing.on_supplier_saved(supplier);
                    self.toast(self.lang.text("บันทึกผู้ขายแล้ว", "Supplier saved."));
                    self.send(Command::LoadPurchases(self.purchasing.offset));
                }
                Event::PurchaseCreated(order) => {
                    let id = order.id;
                    self.purchasing.on_order_created(order);
                    self.toast(self.lang.text("สร้างใบสั่งซื้อแล้ว", "Purchase order created."));
                    self.purchasing.offset = 0;
                    self.purchase_followup = Some(id);
                    self.refresh();
                }
                Event::PurchaseReceived(order) => {
                    let id = order.id;
                    self.purchasing.on_order_updated(order);
                    self.toast(
                        self.lang
                            .text("รับของและเพิ่มสต็อกแล้ว", "Goods received; stock updated."),
                    );
                    self.purchase_followup = Some(id);
                    self.refresh();
                }
                Event::PurchaseCancelled(order) => {
                    let id = order.id;
                    self.purchasing.on_order_updated(order);
                    self.toast(
                        self.lang
                            .text("ยกเลิกยอดค้างรับแล้ว", "Outstanding quantities cancelled."),
                    );
                    self.purchase_followup = Some(id);
                    self.refresh();
                }
                Event::IngredientSaved => {
                    self.ingredients.editor = None;
                    self.ingredients.adjust = None;
                    self.toast(self.lang.text("บันทึกวัตถุดิบแล้ว", "Ingredient saved."));
                    if let Some((id, _)) = &self.ingredients.history {
                        let id = *id;
                        self.send(Command::IngredientHistory(id));
                    }
                    self.refresh();
                }
                Event::IngredientHistory(id, moves) => {
                    if self
                        .ingredients
                        .history
                        .as_ref()
                        .is_some_and(|(open, _)| *open == id)
                    {
                        self.ingredients.history = Some((id, moves));
                    }
                }
                Event::PromotionSaved => {
                    self.promotions.editor = None;
                    self.toast(self.lang.text("บันทึกโปรโมชันแล้ว", "Promotions saved."));
                    self.refresh();
                }
                Event::TableSaved => {
                    self.tables.editor = None;
                    self.toast(self.lang.text("บันทึกโต๊ะแล้ว", "Table saved."));
                    self.refresh();
                }
                Event::Kitchen(tickets) => self.data.kitchen = tickets,
                Event::KitchenSent(cart, ticket) => {
                    let lang = self.lang;
                    match &ticket {
                        Some(t) => {
                            self.toast(format!(
                                "{} #{}",
                                if t.is_cancellation() {
                                    lang.text(
                                        "ส่งยกเลิกเข้าครัวแล้ว",
                                        "Cancellation sent to the kitchen",
                                    )
                                } else {
                                    lang.text("ส่งครัวแล้ว", "Sent to the kitchen")
                                },
                                t.number
                            ));
                            self.print_kitchen(t);
                        }
                        None => self.toast(
                            lang.text("ไม่มีรายการใหม่ที่ต้องส่งครัว", "Nothing new for the kitchen"),
                        ),
                    }
                    if self.cart.id == cart.id {
                        self.cart.sent = cart.sent.clone();
                        // A table's order goes back to the floor once sent.
                        if self.cart.table.is_some()
                            && !self.cart.lines.is_empty()
                            && ticket.is_some()
                        {
                            self.park_cart();
                            self.sell.pane = sale_ui::Pane::Tables;
                        }
                    }
                    self.refresh();
                }
                Event::PricingSaved => {
                    self.pricing_draft = None;
                    self.toast(self.lang.text(
                        "บันทึกการคิดราคาแล้ว ใช้กับบิลถัดไป",
                        "Pricing saved. It applies to the next bills.",
                    ));
                    self.refresh();
                }
                Event::TaxSaved => {
                    self.tax.draft = None;
                    self.tax.rate.clear();
                    self.toast(self.lang.text(
                        "บันทึกข้อมูลภาษีแล้ว ใช้กับบิลถัดไป",
                        "Tax details saved. They apply to the next receipts.",
                    ));
                    self.refresh();
                }
                Event::InvoiceIssued(invoice) => {
                    self.tax.invoice = None;
                    let mut record = None;
                    for r in self
                        .data
                        .sales
                        .iter_mut()
                        .chain(self.history.selected.iter_mut())
                        .chain(self.receipt.iter_mut())
                    {
                        if r.sale.id == invoice.sale_id {
                            r.tax_invoice = Some(invoice.clone());
                            record = Some(r.clone());
                        }
                    }
                    self.toast(format!(
                        "{} TI-{:06}",
                        self.lang.text("ออกใบกำกับภาษีแล้ว", "Tax invoice issued"),
                        invoice.number
                    ));
                    if let Some(record) = record {
                        self.print_tax_invoice(&record, &invoice, false);
                    }
                    self.refresh();
                }
                Event::RoleSaved => {
                    self.staff.role_editor = None;
                    self.toast(self.lang.text("บันทึกบทบาทแล้ว", "Role saved."));
                    self.refresh();
                }
                Event::Failed(pos_core::Error::ApprovalRequired(permission)) if self.ready => {
                    self.recovery.confirm = false;
                    let command = self.last_command.take();
                    crate::staff_ui::request_approval(self, permission, command);
                }
                Event::Failed(
                    error @ (pos_core::Error::WrongPin
                    | pos_core::Error::PinLocked(_)
                    | pos_core::Error::StaffNotFound),
                ) if crate::staff_ui::locked(self) || self.staff.approval.is_some() => {
                    self.staff.pin_error = Some(self.lang.error(&error));
                    if let Some(a) = &mut self.staff.approval {
                        a.pin.clear();
                    }
                }
                Event::Failed(pos_core::Error::SignInRequired) if self.ready => {
                    // The member was switched off or the session ended: lock.
                    self.lock_session();
                    self.refresh();
                }
                Event::Failed(error) => {
                    self.staff.approval = None;
                    if !self.ready {
                        self.fatal = true;
                    }
                    if matches!(
                        error,
                        pos_core::Error::NoOpenShift
                            | pos_core::Error::ShiftChanged
                            | pos_core::Error::ShiftAlreadyOpen
                            | pos_core::Error::ShiftClosed
                            | pos_core::Error::PaymentTotalChanged
                    ) && self.ready
                    {
                        self.drawer.pending = None;
                        self.payment_review = None;
                        self.refresh();
                    }
                    if matches!(error, pos_core::Error::StockChanged) {
                        // The count is stale: close it so the next one starts from fresh stock.
                        self.catalog.stock_dialog = None;
                        self.refresh();
                    }
                    self.error = Some(self.lang.error(&error));
                    eprintln!("POS Heart: {error}");
                }
            }
        }
    }
    fn write_marker(&mut self) {
        let _ = std::fs::write(self.data_dir.join(CURRENT_BILL), self.cart.id.to_string());
    }
    fn clear_marker(&mut self) {
        let _ = std::fs::remove_file(self.data_dir.join(CURRENT_BILL));
    }
    /// Debounced autosave of the open bill; also drops the saved copy once emptied.
    fn autosave(&mut self, ctx: &egui::Context) {
        if self.autosave.snapshot.as_ref() != Some(&self.cart) {
            self.autosave.snapshot = Some(self.cart.clone());
            self.autosave.dirty_since = Some(Instant::now());
        }
        let Some(since) = self.autosave.dirty_since else {
            return;
        };
        if since.elapsed() < Duration::from_millis(400) && self.exit_after_save.is_none() {
            ctx.request_repaint_after(Duration::from_millis(450));
            return;
        }
        if self.cart.lines.is_empty() {
            if let Some(id) = self.autosave.saved
                && self.send_background(Command::Discard(id))
            {
                self.autosave.saved = None;
                self.clear_marker();
            } else if self.autosave.saved.is_none() {
                self.clear_marker();
            }
            self.autosave.dirty_since = None;
        } else if self.cart.validate().is_ok()
            && self.send_background(Command::Autosave(self.cart.clone()))
        {
            if self.autosave.saved != Some(self.cart.id) {
                self.autosave.saved = Some(self.cart.id);
                self.write_marker();
            }
            self.autosave.dirty_since = None;
        }
    }
    pub fn navigate(&mut self, page: Page) {
        if self.page == Page::Products && page != Page::Products && self.purchasing.dirty() {
            self.error = Some(
                self.lang
                    .text(
                        "บันทึกหรือทิ้งร่างจัดซื้อก่อนเปลี่ยนหน้า",
                        "Save or discard the purchasing draft before changing workspace.",
                    )
                    .into(),
            );
            return;
        }
        if self.page == Page::Products && page != Page::Products && self.catalog.dirty() {
            self.catalog.pending_nav = Some(page);
            return;
        }
        // Tables is the floor pane of Sell; Alt 9 again goes back to products.
        let page = if page == Page::Tables {
            let floor = self.page == Page::Sell && self.sell.pane == sale_ui::Pane::Tables;
            self.sell.pane = if floor {
                sale_ui::Pane::Products
            } else {
                sale_ui::Pane::Tables
            };
            self.sell.moving = false;
            Page::Sell
        } else {
            page
        };
        self.page = page;
        self.search.clear();
        self.search_changed = None;
        self.display_open = false;
        self.home_refreshed = Instant::now();
        if page == Page::Sell {
            self.sell.focus_search = true;
        }
        if page == Page::Activity {
            self.send(Command::Activity(self.activity.query.clone()));
        } else {
            self.refresh();
        }
    }
    pub fn force_navigate(&mut self, page: Page) {
        self.catalog.pending_nav = None;
        self.catalog.editor = None;
        self.page = Page::Home;
        self.navigate(page);
    }
    pub fn set_language(&mut self, lang: Language) {
        self.lang = lang;
        if let Err(e) = std::fs::write(
            self.data_dir.join("language.txt"),
            if lang == Language::English {
                "en"
            } else {
                "th"
            },
        ) {
            self.error = Some(e.to_string());
        }
    }
    pub fn set_display(&mut self, look: ds_theme::Look, density: Density) {
        self.look = look;
        self.density = density;
        self.display_dirty = true;
        for (file, value) in [
            ("accent.txt", ds_theme::accent_saved(look.accent)),
            ("shape.txt", ds_theme::shape_saved(look.shape)),
            ("density.txt", ds_theme::density_saved(density)),
        ] {
            if let Err(e) = std::fs::write(self.data_dir.join(file), value) {
                self.error = Some(e.to_string());
            }
        }
    }
    /// Full screen is a per-device preference, applied immediately.
    pub fn set_tile_images(&mut self, on: bool) {
        self.tile_images = on;
        if let Err(e) = std::fs::write(
            self.data_dir.join("product-images.txt"),
            if on { "on" } else { "off" },
        ) {
            self.error = Some(e.to_string());
        }
    }
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if let Err(e) = std::fs::write(
            self.data_dir.join("window.txt"),
            crate::window::fullscreen_saved(fullscreen),
        ) {
            self.error = Some(e.to_string());
        }
    }
    pub fn save_shop(&mut self, next: Option<Page>) {
        if self.shop_readonly {
            return;
        }
        let mut settings = self.shop_draft.clone();
        settings.name = settings.name.trim().to_owned();
        settings.promptpay = match pos_domain::PromptPayId::parse(&settings.promptpay) {
            Ok(id) => id.digits().to_owned(),
            Err(_) => settings.promptpay.trim().to_owned(),
        };
        settings.setup_complete = true;
        match settings.save(&self.data_dir) {
            Ok(()) => {
                self.shop = settings.clone();
                self.shop_draft = settings;
                self.onboarding = false;
                self.toast(self.lang.text("บันทึกข้อมูลร้านแล้ว", "Shop settings saved."));
                self.error = None;
                if let Some(page) = next {
                    self.navigate(page);
                }
            }
            Err(e) => {
                self.error = Some(format!(
                    "{}: {e}",
                    self.lang
                        .text("บันทึกข้อมูลร้านไม่สำเร็จ", "Could not save shop settings")
                ))
            }
        }
    }
    pub fn stamped(&self, prefix: &str, ext: &str) -> PathBuf {
        let now = time::OffsetDateTime::now_utc().to_offset(time::macros::offset!(+7));
        let stamp = now
            .format(time::macros::format_description!(
                "[year][month][day]-[hour][minute][second]"
            ))
            .unwrap_or_else(|_| now.unix_timestamp().to_string());
        self.data_dir.join(format!(
            "{prefix}-{stamp}-{}.{ext}",
            &Uuid::new_v4().simple().to_string()[..6]
        ))
    }
    pub fn backup(&mut self) {
        if self.shop_draft != self.shop
            || self.catalog.dirty()
            || self.purchasing.dirty()
            || self.print.draft != self.print.settings
            || self.pricing_draft.is_some()
            || self.tax.draft.is_some()
            || self.auto_backup.dirty()
        {
            self.error = Some(
                self.lang
                    .text(
                        "บันทึกหรือยกเลิกงานแก้ไขก่อนสำรองข้อมูล",
                        "Save or discard edits before backup.",
                    )
                    .into(),
            );
            return;
        }
        let path = self.stamped("pos-heart-backup", "posbackup");
        self.send(Command::RecoveryBackup(
            self.data_dir.clone(),
            path,
            self.cart.clone(),
            self.autosave.saved,
        ));
    }
    pub fn export_products(&mut self) {
        let path = self.stamped("products", "csv");
        self.send(Command::ExportProducts(path));
    }
    pub fn export_report(&mut self) {
        let name = if self.report.single() {
            format!("report-{}", self.report.start)
        } else {
            format!("report-{}-to-{}", self.report.start, self.report.end)
        };
        let path = self.stamped(&name, "csv");
        self.send(Command::ExportReport(
            path,
            self.report.start,
            self.report.end,
        ));
    }
    fn confirmations(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        if let Some(id) = self.held_delete {
            let r = w::dialog(ctx, "delete-held", 440.0, |ui| {
                w::dialog_title(ui, lang.text("ลบบิลที่พักไว้?", "Delete this held bill?"), true);
                ui.label(lang.text(
                    "บิลนี้ยังไม่ได้ชำระ การลบไม่เปลี่ยนสต็อกและย้อนกลับไม่ได้",
                    "This unpaid bill will be removed. Stock is unchanged. This cannot be undone.",
                ));
                let mut out = None;
                w::dialog_actions(ui, |ui| {
                    if w::button(lang.text("ลบบิล", "Delete bill"))
                        .danger()
                        .enabled(!self.busy)
                        .show(ui)
                        .clicked()
                    {
                        out = Some(true);
                    }
                    if w::button(lang.text("ไม่ลบ", "Keep bill"))
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
                    self.held_delete = None;
                    self.send(Command::RemoveHeld(id));
                }
                Some(false) => self.held_delete = None,
                None if r.should_close() => self.held_delete = None,
                None => {}
            }
        }
        if self.close_confirm {
            let r = w::dialog(ctx, "unsaved-exit", 460.0, |ui| {
                w::dialog_title(
                    ui,
                    lang.text("ปิดแอปทั้งที่ยังไม่บันทึก?", "Close without saving?"),
                    true,
                );
                ui.label(lang.text(
                    "มีข้อมูลร้านหรือสินค้าที่แก้ไขแต่ยังไม่บันทึก บิลที่กำลังขายบันทึกอัตโนมัติแล้ว",
                    "Shop or product edits are not saved. The open bill is saved automatically.",
                ));
                let mut out = None;
                w::dialog_actions(ui, |ui| {
                    if w::button(lang.text("ทิ้งการแก้ไขแล้วปิด", "Discard edits & close"))
                        .danger()
                        .show(ui)
                        .clicked()
                    {
                        out = Some(true);
                    }
                    if w::button(lang.text("กลับไปแก้ต่อ", "Keep editing"))
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
                    self.close_confirm = false;
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Some(false) => self.close_confirm = false,
                None if r.should_close() => self.close_confirm = false,
                None => {}
            }
        }
    }
    fn capture(&mut self, ctx: &egui::Context) {
        if let Some(path) = &self.screenshot {
            let captured = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(image) = captured {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Err(e) = image::save_buffer(
                    path,
                    &bytes,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("Screenshot: {e}");
                }
                self.screenshot = None;
                // Scripted captures exit once the image is written.
                self.allow_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.ready
                && !self.busy
                && self.frames > 12
                && self.frames > self.preview_frame + 20
                && self.preview.is_none()
                && !self.screenshot_requested
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.screenshot_requested = true;
            } else if !self.screenshot_requested {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }
    fn close_requested(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_exit {
            return;
        }
        let unsaved = self.shop_draft != self.shop
            || self.print.draft != self.print.settings
            || self.catalog.dirty()
            || self.purchasing.dirty()
            || self.auto_backup.dirty();
        if unsaved {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_confirm = true;
        } else if self.autosave.dirty_since.is_some() && !self.fatal {
            // Flush the open bill before the process exits (bounded wait).
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.exit_after_save.get_or_insert_with(Instant::now);
        }
    }
}
impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if self.display_dirty {
            ds_theme::apply(&ctx, self.look, self.density);
            self.display_dirty = false;
        }
        self.window.update(&ctx, self.fullscreen);
        self.frames += 1;
        self.idle_input(&ctx);
        self.events();
        // `--page tables` and older links land on the Sell floor.
        if self.page == Page::Tables {
            self.page = Page::Sell;
            self.sell.pane = sale_ui::Pane::Tables;
        }
        #[cfg(debug_assertions)]
        crate::preview::apply(self, &ctx);
        self.capture(&ctx);
        if let Some(previous) = self.recovery.pending.clone() {
            egui::CentralPanel::default().show(root, |ui| {
                w::title(ui, self.lang.text("เตรียมกู้คืนข้อมูลแล้ว", "Recovery is prepared"));
                ui.add(egui::Label::new(self.lang.text("ปิดและเปิดแอปใหม่เพื่อใช้ข้อมูลจากชุดสำรอง ข้อมูลเดิมจะเก็บไว้ที่:", "Close and reopen the app to activate the backup. Original data will be preserved at:")).wrap());
                ui.label(previous.display().to_string());
                if w::button(self.lang.text("ปิดแอป", "Close app")).show(ui).clicked() {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            return;
        }
        self.autosave(&ctx);
        if let Some(since) = self.exit_after_save {
            if self.allow_exit || since.elapsed() > Duration::from_secs(3) {
                self.allow_exit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if matches!(self.page, Page::Home | Page::Shift) && !self.onboarding {
            ctx.request_repaint_after(Duration::from_secs(60));
            if self.home_refreshed.elapsed() >= Duration::from_secs(60) && !self.busy {
                self.home_refreshed = Instant::now();
                self.refresh();
            }
        }
        self.close_requested(&ctx);
        if let Some(changed) = self.search_changed {
            if changed.elapsed() >= Duration::from_millis(250) && !self.busy {
                self.search_changed = None;
                self.refresh();
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        if !crate::staff_ui::locked(self) {
            shell::global_keys(self, &ctx);
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L)) {
                crate::staff_ui::lock(self);
            }
        }
        let t = Theme::current(&ctx);
        let soft = t.soft();
        egui::Panel::top("titlebar")
            .exact_size(t.m.titlebar_h)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.p.bg_chrome)
                    .inner_margin(egui::Margin {
                        left: if soft { 10 } else { 0 },
                        right: 10,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(root, |ui| crate::chrome::title_bar(self, ui));
        egui::Panel::top("toolbar")
            .exact_size(t.m.toolbar_h)
            .show_separator_line(!soft)
            .frame(
                egui::Frame::new()
                    .fill(t.p.bg_chrome_raised)
                    .inner_margin(egui::Margin::symmetric(10, 0)),
            )
            .show(root, |ui| crate::chrome::toolbar(self, ui));
        egui::Panel::bottom("statusbar")
            .exact_size(t.m.statusbar_h)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(t.p.bg_chrome))
            .show(root, |ui| crate::chrome::status_bar(self, ui));
        shell::display_popover(self, &ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t.p.bg_canvas)
                    .inner_margin(if t.soft() {
                        egui::Margin {
                            left: space::PANE_GAP as i8,
                            right: space::PANE_GAP as i8,
                            top: 0,
                            bottom: space::PANE_GAP as i8,
                        }
                    } else {
                        egui::Margin::ZERO
                    }),
            )
            .show(root, |ui| {
                if crate::staff_ui::locked(self) {
                    crate::staff_ui::lock_screen(self, ui);
                    return;
                }
                if let Some(message) = self.error.clone()
                    && self.void_editor.is_none()
                    && self.payment_review.is_none()
                    && self.drawer.pending.is_none()
                    && self.catalog.stock_dialog.is_none()
                    && self.history.refund_editor.is_none()
                    && self.sell.discount.is_none()
                    && self.tax.invoice.is_none()
                    && self.tables.editor.is_none()
                    && self.staff.editor.is_none()
                    && self.staff.role_editor.is_none()
                {
                    if w::banner(
                        ui,
                        w::Tone::Danger,
                        &message,
                        Some(self.lang.text("ปิด", "Dismiss")),
                    ) {
                        self.error = None;
                    }
                    ui.add_space(space::S2);
                }
                if crate::print_ui::banner(self, ui) {
                    ui.add_space(space::S2);
                }
                ui.add_enabled_ui(!self.fatal, |ui| {
                    if self.onboarding {
                        crate::settings_ui::onboard(self, ui);
                        return;
                    }
                    match self.page {
                        Page::Sell => sale_ui::show(self, ui),
                        Page::Products => crate::catalog_ui::show(self, ui),
                        Page::History => crate::history_ui::show(self, ui),
                        Page::Home => crate::home_ui::show(self, ui),
                        Page::Reports => crate::report_ui::show(self, ui),
                        Page::Settings => crate::settings_ui::show(self, ui),
                        // Statement pages read label → amount across a row; on wide
                        // screens they stay centred at a readable width.
                        page => {
                            let full = ui.available_rect_before_wrap();
                            // Square shape has no canvas margin; keep page
                            // headings off the window edge.
                            let full = if t.soft() {
                                full
                            } else {
                                full.shrink(space::S2)
                            };
                            let width = full.width().min(READING_WIDTH);
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(full.center().x - width / 2.0, full.top()),
                                egui::vec2(width, full.height()),
                            );
                            ui.allocate_rect(full, egui::Sense::hover());
                            let mut page_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(rect)
                                    .layout(egui::Layout::top_down(egui::Align::Min)),
                            );
                            match page {
                                Page::Activity => {
                                    if !crate::staff_ui::may(self, pos_domain::Permission::Reports)
                                    {
                                        w::help_text(
                                            &mut page_ui,
                                            self.lang.text(
                                                "บทบาทนี้ดูประวัติกิจกรรมไม่ได้",
                                                "Your role cannot view activity history.",
                                            ),
                                        );
                                        return;
                                    }
                                    let refresh = t
                                        .surface()
                                        .show(&mut page_ui, |ui| {
                                            ui.set_min_size(ui.available_size());
                                            crate::activity_ui::show(
                                                ui,
                                                &t,
                                                self.lang,
                                                &mut self.activity,
                                            )
                                        })
                                        .inner;
                                    if refresh {
                                        self.send(Command::Activity(self.activity.query.clone()));
                                    }
                                }
                                _ => crate::shift_ui::show(self, &mut page_ui),
                            }
                        }
                    }
                });
            });
        // Keep the second window alive but gate its content while locked.
        crate::kitchen_ui::window(self, &ctx);
        if crate::staff_ui::locked(self) {
            return;
        }
        self.confirmations(&ctx);
        sale_ui::payment_dialog(self, &ctx);
        sale_ui::bill_split_dialog(self, &ctx);
        if crate::staff_ui::may(self, pos_domain::Permission::Settings) {
            crate::promo_ui::editor_dialog(self, &ctx);
            crate::ingredients_ui::dialogs(self, &ctx);
            crate::categories_ui::dialogs(self, &ctx);
            crate::option_sets_ui::dialogs(self, &ctx);
        }
        crate::purchasing_ui::dialogs(self, &ctx);
        crate::promo_ui::coupon_dialog(self, &ctx);
        sale_ui::options_sheet(self, &ctx);
        crate::discount_ui::dialog(self, &ctx);
        crate::history_ui::void_dialog(self, &ctx);
        crate::history_ui::refund_dialog(self, &ctx);
        crate::history_ui::receipt_dialog(self, &ctx);
        crate::shift_ui::confirm(self, &ctx);
        crate::catalog_ui::dialogs(self, &ctx);
        if crate::staff_ui::may(self, pos_domain::Permission::ManageStaff) {
            crate::staff_ui::editor_dialog(self, &ctx);
        }
        if crate::staff_ui::may(self, pos_domain::Permission::Settings) {
            crate::tables_ui::editor_dialog(self, &ctx);
        }
        crate::print_ui::preview_dialog(self, &ctx);
        if crate::staff_ui::may(self, pos_domain::Permission::ManageStaff) {
            crate::staff_ui::role_dialog(self, &ctx);
        }
        crate::tax_ui::invoice_dialog(self, &ctx);
        crate::backup_ui::dialog(self, &ctx);
        crate::staff_ui::approval_dialog(self, &ctx);
        shell::palette(self, &ctx);
        shell::toasts(self, &ctx);
    }
}
