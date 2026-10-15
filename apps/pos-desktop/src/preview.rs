//! Debug builds only: `--preview NAME` opens an overlay or state once data is
//! loaded so native layouts can be captured with `--screenshot` (AGENTS.md
//! asks for runtime inspection at 1000×700 and 1280×820 in both palettes).
use crate::{app::App, sale_ui, shift_ui, ui::format};
use eframe::egui;
use pos_domain::{PaymentMethod, PaymentRequest};

pub fn arm(_app: &mut App) {}

pub fn apply(app: &mut App, ctx: &egui::Context) {
    if !app.ready || app.busy || app.restoring_bill || app.preview.is_none() {
        return;
    }
    // Wait for the restored bill (background) before applying bill presets.
    if app.cart.lines.is_empty()
        && app
            .preview
            .as_deref()
            .is_some_and(|p| p.starts_with("sell"))
    {
        return;
    }
    let Some(mut name) = app.preview.take() else {
        return;
    };
    // "as-cashier:PRESET" / "as-owner:PRESET" sign in a seeded member first.
    for (prefix, who, pin) in [
        ("as-cashier:", "นิด", "1111"),
        ("as-owner:", "สมชาย", "9999"),
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            if app.data.signed_in.is_none() {
                if let Some(id) = app.data.staff.iter().find(|s| s.name == who).map(|s| s.id) {
                    app.send(crate::worker::Command::SignIn(id, pin.into()));
                    app.preview = Some(name);
                }
                return;
            }
            name = rest.to_owned();
            break;
        }
    }
    // Keep capture presets clean after the synthetic owner sign-in. The
    // explicit undo-toast preset still demonstrates its transient feedback.
    if name != "sell-toast" {
        app.toasts.clear();
    }
    if matches!(
        name.as_str(),
        "purchasing"
            | "suppliers"
            | "purchase-create"
            | "purchase-create-confirm"
            | "purchase-receive"
            | "purchase-receive-confirm"
            | "purchase-cancel"
            | "supplier-editor"
            | "purchase-detail"
    ) {
        app.page = crate::app::Page::Products;
        if !app.purchasing.loaded {
            app.send(crate::worker::Command::LoadPurchases(0));
            app.preview = Some(name);
            return;
        }
        if name == "purchase-detail"
            && app.purchasing.selected.is_none()
            && let Some(id) = app.purchasing.orders.first().map(|o| o.id)
        {
            app.send(crate::worker::Command::PurchaseDetail(id, 0));
            app.preview = Some(name);
            return;
        }
        crate::purchasing_ui::preview(app, &name);
        app.preview_frame = app.frames;
        return;
    }
    app.preview_frame = app.frames;
    let total = app.total().ok();
    match name.as_str() {
        "page" => {}
        "report-categories" => app.report_group = crate::report_ui::Group::Categories,
        "automatic-backups" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Data;
            app.auto_backup.draft.enabled = true;
            app.recovery.focus = true;
        }
        "idle-settings" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Staff;
        }
        "idle-lock" => {
            app.auto_lock.settings.timeout_minutes = 1;
            app.auto_lock
                .reset(std::time::Instant::now() - std::time::Duration::from_secs(61));
        }
        "palette" => {
            app.palette.open();
            app.palette.query = "ชา".into();
        }
        "palette-empty" => app.palette.open(),
        "display" => app.display_open = true,
        "sell-pay" => app.sell.paying = Some(app.cart.id),
        // Settings → Devices with the hub running on a free port and a
        // pairing code on screen (nothing is saved to hub.json).
        "settings-devices-pairing" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Devices;
            app.hub.settings = crate::hub_ui::Settings {
                enabled: true,
                port: 0,
                loopback: true,
            };
            crate::hub_ui::start(app);
            if let Some(hub) = &app.hub.hub {
                app.hub.pairing = Some((hub.open_pairing(), std::time::Instant::now()));
            }
        }
        "sell-cash" => {
            app.sell.paying = Some(app.cart.id);
            if let Some(total) = total {
                app.cash = format::money(sale_ui::quick_cash(total)[0]);
            }
            app.sell.current = Some(1);
        }
        "sell-short" => {
            app.sell.paying = Some(app.cart.id);
            app.cash = "100.00".into();
        }
        "sell-note" => {
            app.sell.current = Some(0);
            app.sell.note_edit = Some(0);
        }
        "sell-review" => {
            app.sell.paying = Some(app.cart.id);
            app.payment_method = PaymentMethod::Transfer;
            if let (Some(total), Some(shift)) = (total, app.data.shift.as_ref()) {
                app.payment_review = Some(sale_ui::PaymentReview::new(
                    app.cart.clone(),
                    PaymentRequest {
                        method: PaymentMethod::Transfer,
                        amount: total,
                    },
                    shift.id,
                ));
            }
        }
        "sell-split" | "sell-split-review" => {
            app.sell.paying = Some(app.cart.id);
            app.sell.split_pay = Some(sale_ui::SplitDraft {
                transfer: "100.00".into(),
                card: String::new(),
            });
            app.cash = "500.00".into();
            if name == "sell-split-review"
                && let (Some(total), Some(shift)) = (app.total().ok(), app.data.shift.as_ref())
                && let Ok(cash) = format::parse_money("500.00")
                && let Ok(hundred) = format::parse_money("100.00")
            {
                let rest = pos_domain::Money::from_satang(total.satang() - hundred.satang())
                    .unwrap_or_default();
                let cash = sale_ui::quick_cash(rest)
                    .get(1)
                    .copied()
                    .unwrap_or(cash)
                    .max(rest);
                app.cash = format::money(cash);
                app.payment_review = Some(sale_ui::PaymentReview::split(
                    app.cart.clone(),
                    total,
                    pos_domain::SplitPayment {
                        tenders: vec![
                            pos_domain::Tender {
                                method: PaymentMethod::Cash,
                                amount: rest,
                            },
                            pos_domain::Tender {
                                method: PaymentMethod::Transfer,
                                amount: hundred,
                            },
                        ],
                        cash_given: cash,
                    },
                    shift.id,
                ));
            }
        }
        "bill-split" => app.sell.bill_split = Some(sale_ui::BillSplit::preview(&app.cart)),
        "sell-large" => {
            app.sell.paying = Some(app.cart.id);
            app.cash = "99999.00".into();
            app.sell.confirm_large_cash = true;
        }
        "sell-discount" | "sell-discounted" => {
            if let Some(line) = app.cart.lines.first_mut() {
                line.discount = Some(pos_domain::Discount::Percent(1_000));
            }
            app.cart.discount = Some(pos_domain::Discount::Amount(
                pos_domain::Money::from_satang(5_000).unwrap_or_default(),
            ));
            app.cart.discount_reason = "สมาชิก".into();
            app.sell.current = Some(0);
            if name == "sell-discount" {
                crate::discount_ui::open(app, Some(0));
            }
        }
        "backup-review" | "backup-confirm" | "backup-pending" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Data;
            app.cart = pos_domain::Cart::new();
            let review = crate::backup::Review {
                source: app.data_dir.join("sample.posbackup"),
                shop: "ร้านหัวใจ / Heart Shop".into(),
                manifest: crate::backup::Manifest {
                    version: 1,
                    created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    info: pos_core::BackupInfo {
                        schema: i64::from(pos_core::SCHEMA_VERSION),
                        sales: 156,
                        products: 42,
                        held: 3,
                    },
                    files: vec![],
                },
            };
            app.recovery.focus = true;
            app.recovery.source = review.source.display().to_string();
            app.recovery.review = Some(review);
            app.recovery.confirm = name == "backup-confirm";
            if name == "backup-pending" {
                app.recovery.pending = Some(app.data_dir.with_file_name(".sample-before-recovery"));
            }
        }
        "activity" => {
            app.page = crate::app::Page::Activity;
            app.send(crate::worker::Command::Activity(app.activity.query.clone()));
        }
        "settings-vat" | "settings-roles" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = if name == "settings-vat" {
                crate::settings_ui::Section::Pricing
            } else {
                crate::settings_ui::Section::Staff
            };
            app.preview_scroll_bottom = true;
        }
        "settings-printer" => {
            app.preview_scroll_bottom = true;
            app.settings_section = crate::settings_ui::Section::Printing;
            app.print.draft.connection = crate::print::Connection::Network;
            app.print.draft.address = "192.168.1.80".into();
            app.print.draft.paper = crate::print::Paper::Mm58;
            app.print.draft.auto_print = true;
            app.print.failed = Some("พิมพ์ไม่สำเร็จ #000056 · ข้อมูลบันทึกแล้ว ตรวจกระดาษ สาย และการตั้งค่าเครื่องพิมพ์ (Connection refused)".into());
        }
        "print-preview" => {
            // Writes receipt and shift summary images into the data folder.
            if let Some(record) = app.data.sales.first() {
                let path = app.data_dir.join("print-preview");
                let lang = app.lang;
                let mut settings = app.print.settings.clone();
                for (paper, name) in [
                    (crate::print::Paper::Mm58, "58"),
                    (crate::print::Paper::Mm80, "80"),
                ] {
                    settings.paper = paper;
                    let mut t = app.shop.templates.receipt.clone();
                    if t.footer.is_empty() {
                        t.footer = "ขอบคุณที่อุดหนุน".into();
                    }
                    let doc = app.receipt_doc(&t, record, true);
                    let _ = crate::print::save_preview(
                        &settings,
                        &doc,
                        &path.with_extension(format!("receipt{name}.png")),
                    );
                }
                if let Some(r) = app.data.sales.iter().find(|r| r.tax_invoice.is_some())
                    && let Some(invoice) = &r.tax_invoice
                    && let Some(doc) = crate::print::docs::tax_invoice(r, invoice, lang, false)
                {
                    let _ = crate::print::save_preview(
                        &settings,
                        &doc,
                        &path.with_extension("invoice.png"),
                    );
                }
                if let Some(ticket) = app.data.kitchen.first() {
                    let doc = crate::print::docs::kitchen_ticket(
                        ticket,
                        &app.shop.templates.kitchen,
                        lang,
                    );
                    let _ = crate::print::save_preview(
                        &settings,
                        &doc,
                        &path.with_extension("kitchen.png"),
                    );
                }
                if let Some(shift) = app.data.shifts.first() {
                    let doc =
                        crate::print::docs::shift_summary(shift, Some(1), &app.shop.name, lang);
                    let _ = crate::print::save_preview(
                        &settings,
                        &doc,
                        &path.with_extension("shift.png"),
                    );
                }
            }
        }
        "approval" => crate::staff_ui::request_approval(
            app,
            pos_domain::Permission::Refund,
            Some(crate::worker::Command::Demo),
        ),
        "invoice" => {
            if let Some(r) = app.data.sales.iter().find(|r| {
                r.sale.tax.is_some()
                    && r.tax_invoice.is_none()
                    && r.refunds.is_empty()
                    && r.cancellation.is_none()
            }) {
                app.tax.invoice = Some(crate::tax_ui::InvoiceEditor {
                    sale_id: r.sale.id,
                    sale_number: r.sale.number,
                    name: "บริษัท ลูกค้าตัวอย่าง จำกัด".into(),
                    tax_id: "0105555987650".into(),
                    head_office: false,
                    branch: "1".into(),
                    address: "1 ถนนตัวอย่าง เชียงใหม่ 50000".into(),
                });
            }
        }
        "history-invoiced" => {
            app.history.selected = app
                .data
                .sales
                .iter()
                .find(|r| r.tax_invoice.is_some())
                .cloned()
        }
        "role-editor" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Staff;
            crate::staff_ui::preview_role_editor(app);
        }
        "report-profit" => {
            app.data.summary.profit = pos_domain::ProfitSummary {
                covered_revenue: pos_domain::SignedMoney::from_satang(1825035),
                cost_of_goods: pos_domain::SignedMoney::from_satang(1020050),
                gross_profit: pos_domain::SignedMoney::from_satang(804985),
                unknown_revenue: pos_domain::SignedMoney::from_satang(250050),
                known_sold_units: 152,
                unknown_sold_units: 12,
                known_refunded_units: 3,
                unknown_refunded_units: 1,
            };
        }
        "product-cost" => {
            app.catalog.inspector_tab = crate::catalog_ui::InspectorTab::Price;
            if let Some(p) = app.data.products.first().cloned() {
                let mut editor = crate::catalog_ui::Editor::from(p);
                editor.cost = "12.35".into();
                app.catalog.editor = Some(editor);
            }
        }
        "kitchen" => {
            // Captures show the kitchen screen inside the main window.
            ctx.set_embed_viewports(true);
            app.kitchen_open = true;
        }
        "table-bill" => {
            if let Some(id) = app
                .data
                .held
                .iter()
                .find(|c| c.table.is_some() && c.lines.len() > 4)
                .map(|c| c.id)
            {
                app.resume(id);
            }
        }
        "table-pay" => {
            if let Some(cart) = app.data.held.iter().find(|c| c.table.is_some()).cloned() {
                app.sell.paying = Some(cart.id);
                app.resume(cart.id);
            }
        }
        "table-move" => {
            if let Some(id) = app
                .data
                .held
                .iter()
                .find(|c| c.table.is_some())
                .map(|c| c.id)
            {
                app.resume(id);
            }
            app.sell.moving = true;
            app.sell.pane = crate::sale_ui::Pane::Tables;
        }
        "table-editor" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Tables;
            if let Some(t) = app.data.tables.first().cloned() {
                app.tables.editor = Some(t);
            }
        }
        "templates" | "templates-bill" | "templates-kitchen" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Printing;
            app.templates.tab = match name.as_str() {
                "templates-bill" => crate::templates_ui::Tab::Bill,
                "templates-kitchen" => crate::templates_ui::Tab::Kitchen,
                _ => crate::templates_ui::Tab::Receipt,
            };
            let mut t = app.shop.templates.clone();
            t.receipt.header = "99 ถนนตัวอย่าง กรุงเทพฯ\nโทร 02-000-0000 · LINE @heart".into();
            t.receipt.footer = "ขอบคุณที่อุดหนุน\nWi-Fi: heart-guest".into();
            t.receipt.logo = app.logo.is_some();
            app.templates.draft = Some(t);
        }
        "ingredients" | "ingredient-history" => {
            app.page = crate::app::Page::Products;
            app.ingredients.open = true;
            if name == "ingredient-history"
                && let Some(id) = app.data.ingredients.first().map(|i| i.id)
            {
                app.ingredients.history = Some((id, Vec::new()));
                app.send(crate::worker::Command::IngredientHistory(id));
            }
        }
        "option-sets" | "option-set-editor" => {
            app.page = crate::app::Page::Products;
            app.option_sets.open = true;
            if name == "option-set-editor"
                && let Some((set, _)) = app.data.option_sets.first()
            {
                app.option_sets.editor = Some(vec![crate::catalog_ui::GroupDraft::from(set)]);
            }
        }
        "option-set-bulk" => {
            app.page = crate::app::Page::Products;
            app.catalog.selected = app.data.products.iter().take(3).map(|p| p.id).collect();
            app.option_sets.bulk = Some((app.data.option_sets.first().map(|(s, _)| s.id), true));
        }
        "categories" | "category-rename" => {
            app.page = crate::app::Page::Products;
            app.categories.open = true;
            if name == "category-rename"
                && let Some((c, _)) = app.data.counts.categories.first().cloned()
            {
                app.categories.rename = Some((
                    c.clone(),
                    app.data.counts.categories.get(1).map_or(c, |x| x.0.clone()),
                ));
            }
        }
        "product-recipe" => {
            app.catalog.inspector_tab = crate::catalog_ui::InspectorTab::Options;
            app.page = crate::app::Page::Products;
            if let Some(p) = app
                .data
                .products
                .iter()
                .find(|p| !p.recipe.is_empty())
                .cloned()
            {
                let mut editor = crate::catalog_ui::Editor::from(p);
                editor.scroll_to_recipe = true;
                app.catalog.editor = Some(editor);
            }
        }
        "promo-editor" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Promotions;
            if let Some(p) = app
                .data
                .promotions
                .iter()
                .find(|p| p.from_minute != p.to_minute)
                .cloned()
            {
                app.promotions.editor = Some(crate::promo_ui::Editor::new(p));
            }
        }
        "sell-coupon" => app.promotions.coupon = Some("welcome".into()),
        "credit-note" => {
            if let Some(note) = app
                .data
                .sales
                .iter()
                .find_map(|r| r.credit_notes.first())
                .cloned()
            {
                app.preview_credit_note(&note);
            }
        }
        "receipt-preview" => {
            if let Some(r) = app.data.sales.first().cloned() {
                app.preview_receipt(&r);
            }
        }
        "check-bill" => app.preview_check_bill(),
        "report-staff" => app.report_group = crate::report_ui::Group::Staff,
        "report-promos" => app.report_group = crate::report_ui::Group::Promotions,
        "report-ingredients" => app.report_group = crate::report_ui::Group::Ingredients,
        "staff-editor" => {
            app.page = crate::app::Page::Settings;
            app.settings_section = crate::settings_ui::Section::Staff;
        }
        "sell-held" => app.sell.tab = sale_ui::CartTab::Held,
        "sell-options" => {
            if let Some(p) = app
                .data
                .products
                .iter()
                .find(|p| !p.options.is_empty())
                .cloned()
            {
                app.add(p);
            }
        }
        "sell-toast" => app.remove_line(0),
        "paid" => {
            if let Some(record) = app.data.sales.first() {
                app.last_sale = Some(record.sale.clone());
            }
            app.cart.lines.clear();
        }
        "receipt" => app.receipt = app.data.sales.first().cloned(),
        "void" => {
            if let Some(record) = app.data.sales.iter().find(|r| r.cancellation.is_none()) {
                app.void_editor = Some(crate::history_ui::VoidEditor::new(record.sale.clone()));
            }
        }
        "refund" => {
            if let Some(record) = app
                .data
                .sales
                .iter()
                .find(|r| r.cancellation.is_none() && r.sale.lines.len() > 1)
            {
                let mut editor = crate::history_ui::RefundEditor::new(record.clone());
                editor.preview_pick();
                app.history.refund_editor = Some(editor);
            }
        }
        "history-detail" => app.history.selected = app.data.sales.get(1).cloned(),
        "history-refunded" => {
            app.history.selected = app
                .data
                .sales
                .iter()
                .find(|r| !r.refunds.is_empty())
                .cloned()
        }
        "close" | "close-confirm" => {
            app.drawer.tab = shift_ui::Tab::Close;
            if let Some(id) = app.data.shift.as_ref().map(|s| s.id) {
                shift_ui::load_draft(app, id);
            }
            app.drawer.counts = ["2", "1", "8", "4", "6", "", "3", "", "5"]
                .map(String::from)
                .to_vec();
            app.drawer.reviewing = true;
            if name == "close-confirm" {
                app.drawer.close_reason = "ทอนเกินบิล #000012".into();
                shift_ui::preview_confirm(app);
            }
        }
        "report-week" | "report-month" => {
            let today = crate::worker::today();
            let days = if name == "report-week" { 6 } else { 29 };
            app.report = crate::report_ui::Period {
                start: today
                    .checked_sub(time::Duration::days(days))
                    .unwrap_or(today),
                end: today,
            };
            if name == "report-month" {
                app.report_group = crate::report_ui::Group::Categories;
            }
            app.refresh();
        }
        "shift-history" => app.drawer.tab = shift_ui::Tab::History,
        "product" => {
            if let Some(p) = app.data.products.get(3).cloned() {
                let mut editor = crate::catalog_ui::Editor::from(p);
                editor.price = "1,049.00".into();
                app.catalog.editor = Some(editor);
            }
        }
        "stock-count" | "stock-remove" => {
            if let Some(p) = app
                .data
                .products
                .iter()
                .find(|p| p.stock.is_some_and(|n| n > 5))
                .cloned()
            {
                let kind = if name == "stock-count" {
                    pos_domain::StockKind::Count
                } else {
                    pos_domain::StockKind::Remove
                };
                let current = p.stock.unwrap_or(0);
                app.catalog.stock_dialog = Some(crate::catalog_ui::StockDialog::new(
                    p.id,
                    p.name.clone(),
                    kind,
                    current,
                ));
                app.catalog.editor = Some(crate::catalog_ui::Editor::from(p));
                app.refresh();
            }
        }
        "csv-import" | "csv-import-errors" => {
            app.page = crate::app::Page::Products;
            let product = pos_domain::ProductInput {
                sku: "NEW-001".into(),
                name: "ชาไทยสูตรพิเศษ หวานน้อย เพิ่มไข่มุก / Special Thai tea with pearls".into(),
                price: pos_domain::Money::from_satang(125_050).unwrap_or_default(),
                cost: Some(pos_domain::Money::from_satang(35_025).unwrap_or_default()),
                stock: Some(24),
                ..Default::default()
            };
            let issues = if name == "csv-import-errors" {
                vec![
                    pos_core::ImportIssue {
                        row: 3,
                        message: "รหัสสินค้าซ้ำในไฟล์ / Duplicate SKU in file".into(),
                    },
                    pos_core::ImportIssue {
                        row: 4,
                        message: "price: ราคาไม่ถูกต้อง (ทศนิยมไม่เกิน 2 ตำแหน่ง) / Invalid price".into(),
                    },
                    pos_core::ImportIssue {
                        row: 5,
                        message: "บาร์โค้ดมีอยู่แล้ว / Barcode already exists".into(),
                    },
                ]
            } else {
                Vec::new()
            };
            app.catalog.import = Some(crate::import_ui::ImportState {
                path: "/tmp/products.csv".into(),
                preview: Some(pos_core::ImportPreview {
                    source:
                        "sku,name,price,cost,stock\nNEW-001,Special Thai tea,1250.50,350.25,24\n"
                            .into(),
                    rows: vec![pos_core::ImportRow { row: 2, product }],
                    issues,
                }),
            });
        }
        "product-new" => app.catalog.editor = Some(crate::catalog_ui::Editor::default()),
        "bulk" => {
            app.catalog.selected = app.data.products.iter().take(2).map(|p| p.id).collect();
        }
        other => match other
            .strip_prefix("settings-")
            .and_then(crate::settings_ui::Section::from_name)
        {
            Some(section) => {
                app.page = crate::app::Page::Settings;
                app.settings_section = section;
            }
            None => eprintln!("Unknown preview: {name}"),
        },
    }
}
