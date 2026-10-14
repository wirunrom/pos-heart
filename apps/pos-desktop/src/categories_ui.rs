//! Products → Manage categories: the shop's category order (also Sell's),
//! add, rename (into an existing name merges), and delete with its products
//! moved to another category or left uncategorised.
use crate::{
    app::App,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align};
use pos_domain::Permission;

#[derive(Default)]
pub struct CategoriesState {
    /// The Products workspace shows categories instead of products.
    pub open: bool,
    pub new_name: String,
    /// (old name, typed new name)
    pub rename: Option<(String, String)>,
    /// (category, move its products here; None = uncategorised)
    pub delete: Option<(String, Option<String>)>,
}

pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let may = crate::staff_ui::may(app, Permission::Catalog) && !app.busy;
    let list: Vec<(String, u64)> = app
        .data
        .counts
        .categories
        .iter()
        .filter(|(c, _)| !c.is_empty())
        .cloned()
        .collect();
    ui.spacing_mut().item_spacing.y = space::S2;
    ui.horizontal(|ui| {
        ui.label(t.text(lang.text("หมวดหมู่", "Categories"), type_scale::TITLE));
        ui.label(t.muted(format::count(list.len() as u64), type_scale::BODY));
    });
    w::help_text(
        ui,
        lang.text(
            "ลำดับนี้ใช้ในหน้าขายและหน้าสินค้า เปลี่ยนชื่อแล้วสินค้าทั้งหมดในหมวดเปลี่ยนตาม เปลี่ยนเป็นชื่อหมวดที่มีอยู่แล้ว = รวมหมวด",
            "This order is used in Sell and Products. Renaming moves every product along; renaming to an existing name merges the two.",
        ),
    );
    ui.add_enabled_ui(may, |ui| {
        ui.horizontal(|ui| {
            let r = w::text_input(
                ui,
                &mut app.categories.new_name,
                w::FieldOpts {
                    hint: lang.text("ชื่อหมวดใหม่", "New category name"),
                    limit: 80,
                    width: Some(260.0),
                    ..Default::default()
                },
            );
            let name = app.categories.new_name.trim().to_owned();
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (w::button(lang.text("เพิ่มหมวด", "Add category"))
                .icon(Icon::Plus)
                .enabled(!name.is_empty())
                .show(ui)
                .clicked()
                || enter)
                && !name.is_empty()
            {
                app.send(Command::AddCategory(name));
                app.categories.new_name.clear();
            }
        });
    });
    let mut order: Option<Vec<String>> = None;
    let count = list.len();
    let out = egui::ScrollArea::vertical()
        .id_salt("categories")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, (name, n)) in list.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(may, |ui| {
                        ui.add_enabled_ui(i > 0, |ui| {
                            if w::icon_button(ui, Icon::ChevronUp, lang.text("เลื่อนขึ้น", "Move up"))
                                .clicked()
                            {
                                let mut names: Vec<String> =
                                    list.iter().map(|(c, _)| c.clone()).collect();
                                names.swap(i, i - 1);
                                order = Some(names);
                            }
                        });
                        ui.add_enabled_ui(i + 1 < count, |ui| {
                            if w::icon_button(
                                ui,
                                Icon::ChevronDown,
                                lang.text("เลื่อนลง", "Move down"),
                            )
                            .clicked()
                            {
                                let mut names: Vec<String> =
                                    list.iter().map(|(c, _)| c.clone()).collect();
                                names.swap(i, i + 1);
                                order = Some(names);
                            }
                        });
                    });
                    let text_w = (ui.available_width() - 200.0).max(120.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(text_w, 0.0),
                        egui::Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(text_w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            w::name_label(ui, name, type_scale::BODY_STRONG, t.p.fg_default);
                            ui.label(t.muted(
                                format!(
                                    "{} {}",
                                    format::count(*n),
                                    lang.text("สินค้า", if *n == 1 { "product" } else { "products" })
                                ),
                                type_scale::CAPTION,
                            ));
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_enabled_ui(may, |ui| {
                            if w::button(lang.text("ลบ", "Delete"))
                                .ghost()
                                .size(w::Size::Sm)
                                .show(ui)
                                .clicked()
                            {
                                app.error = None;
                                app.categories.delete = Some((name.clone(), None));
                            }
                            if w::button(lang.text("เปลี่ยนชื่อ", "Rename"))
                                .ghost()
                                .size(w::Size::Sm)
                                .show(ui)
                                .clicked()
                            {
                                app.error = None;
                                app.categories.rename = Some((name.clone(), name.clone()));
                            }
                        });
                    });
                });
                w::divider(ui);
            }
            if count == 0 {
                w::state_view(
                    ui,
                    Icon::Products,
                    lang.text("ยังไม่มีหมวดหมู่", "No categories yet"),
                    lang.text(
                        "เพิ่มหมวดด้านบน หรือใส่หมวดตอนเพิ่มสินค้า",
                        "Add one above, or set a category when adding a product.",
                    ),
                );
            }
        });
    w::scroll_fade(ui, &out, t.p.bg_surface);
    if let Some(names) = order {
        app.send(Command::ReorderCategories(names));
    }
}

pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    rename_dialog(app, ctx);
    delete_dialog(app, ctx);
}
fn rename_dialog(app: &mut App, ctx: &egui::Context) {
    let Some((old, mut new)) = app.categories.rename.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let merge = new.trim() != old
        && app
            .data
            .counts
            .categories
            .iter()
            .any(|(c, _)| c == new.trim());
    let response = w::dialog(ctx, "category-rename", 440.0, |ui| {
        w::dialog_title(ui, lang.text("เปลี่ยนชื่อหมวด", "Rename category"), false);
        w::text_input(
            ui,
            &mut new,
            w::FieldOpts {
                limit: 80,
                ..Default::default()
            },
        );
        if merge {
            w::banner(
                ui,
                w::Tone::Warning,
                &format!(
                    "{} \"{old}\" {} \"{}\"",
                    lang.text("จะรวมสินค้าทั้งหมดใน", "All products in"),
                    lang.text("เข้ากับหมวด", "will merge into"),
                    new.trim()
                ),
                None,
            );
        } else {
            ui.label(t.muted(
                lang.text(
                    "สินค้าในหมวดนี้จะเปลี่ยนตามทั้งหมด",
                    "Every product in it moves along.",
                ),
                type_scale::CAPTION,
            ));
        }
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            let label = if merge {
                lang.text("รวมหมวด", "Merge")
            } else {
                lang.text("บันทึก", "Save")
            };
            if w::button(label)
                .primary()
                .enabled(!new.trim().is_empty() && new.trim() != old && !app.busy)
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
            app.send(Command::RenameCategory(old.clone(), new.trim().to_owned()));
            app.categories.rename = Some((old, new));
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.categories.rename = Some((old, new)),
    }
}
fn delete_dialog(app: &mut App, ctx: &egui::Context) {
    let Some((name, mut into)) = app.categories.delete.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let n = app
        .data
        .counts
        .categories
        .iter()
        .find(|(c, _)| *c == name)
        .map_or(0, |(_, n)| *n);
    let others: Vec<String> = app
        .data
        .counts
        .categories
        .iter()
        .filter(|(c, _)| !c.is_empty() && *c != name)
        .map(|(c, _)| c.clone())
        .collect();
    let response = w::dialog(ctx, "category-delete", 460.0, |ui| {
        w::dialog_title(
            ui,
            &format!("{} \"{name}\"?", lang.text("ลบหมวด", "Delete category")),
            true,
        );
        if n > 0 {
            ui.label(t.text(
                format!(
                    "{} {} {}",
                    lang.text("ย้ายสินค้า", "Move its"),
                    format::count(n),
                    lang.text("รายการไปที่", "products to")
                ),
                type_scale::BODY,
            ));
            ui.horizontal_wrapped(|ui| {
                if w::chip(
                    ui,
                    lang.text("ไม่มีหมวด", "Uncategorised"),
                    into.is_none(),
                    false,
                )
                .clicked()
                {
                    into = None;
                }
                for c in &others {
                    if w::chip(ui, c, into.as_deref() == Some(c.as_str()), false).clicked() {
                        into = Some(c.clone());
                    }
                }
            });
        } else {
            ui.label(t.muted(
                lang.text("หมวดนี้ไม่มีสินค้า", "This category has no products."),
                type_scale::BODY,
            ));
        }
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ลบหมวด", "Delete category"))
                .danger()
                .enabled(!app.busy)
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
            app.send(Command::DeleteCategory(name.clone(), into.clone()));
            app.categories.delete = Some((name, into));
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.categories.delete = Some((name, into)),
    }
}
