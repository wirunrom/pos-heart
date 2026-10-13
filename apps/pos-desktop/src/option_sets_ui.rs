//! Products → Manage → Option sets: groups such as sweetness kept once and
//! used by many products. Editing a set changes every product using it;
//! the bulk bar links a set to the selected products.
use crate::{
    app::App,
    catalog_ui::GroupDraft,
    ui::{Icon, Theme, format, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align};
use pos_domain::{OptionGroup, Permission};
use uuid::Uuid;

#[derive(Default)]
pub struct OptionSetsState {
    /// The Products workspace shows option sets instead of products.
    pub open: bool,
    /// The set being edited (always exactly one group).
    pub editor: Option<Vec<GroupDraft>>,
    pub show_errors: bool,
    pub delete: Option<OptionGroup>,
    /// Bulk: (set, link) for the selected products.
    pub bulk: Option<(Option<Uuid>, bool)>,
}

fn summary(set: &OptionGroup, lang: crate::i18n::Language) -> String {
    let names: Vec<String> = set
        .choices
        .iter()
        .map(|c| {
            if c.price.satang() > 0 {
                format!("{} +{}", c.name, format::money(c.price))
            } else {
                c.name.clone()
            }
        })
        .collect();
    let mut rule = Vec::new();
    if set.required {
        rule.push(lang.text("ต้องเลือก", "required"));
    }
    if set.multiple {
        rule.push(lang.text("เลือกได้หลายอย่าง", "several"));
    }
    if rule.is_empty() {
        names.join(" · ")
    } else {
        format!("{} ({})", names.join(" · "), rule.join(", "))
    }
}

pub fn panel(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let may = crate::staff_ui::may(app, Permission::Catalog) && !app.busy;
    let sets = app.data.option_sets.clone();
    ui.spacing_mut().item_spacing.y = space::S2;
    ui.horizontal(|ui| {
        ui.label(t.text(lang.text("ชุดตัวเลือก", "Option sets"), type_scale::TITLE_LG));
        ui.label(t.muted(format::count(sets.len() as u64), type_scale::BODY));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if may
                && w::button(lang.text("เพิ่มชุดตัวเลือก", "Add option set"))
                    .icon(Icon::Plus)
                    .primary()
                    .show(ui)
                    .clicked()
            {
                app.error = None;
                app.option_sets.show_errors = false;
                app.option_sets.editor = Some(vec![GroupDraft::new()]);
            }
        });
    });
    w::help_text(
        ui,
        lang.text(
            "สร้างครั้งเดียวใช้ได้หลายสินค้า เช่น ความหวาน ท็อปปิ้ง ขนาดแก้ว แก้ที่นี่แล้วทุกสินค้าที่ใช้ชุดนี้เปลี่ยนตาม ผูกกับสินค้าได้ในหน้าแก้ไขสินค้า หรือเลือกหลายสินค้าแล้วกด \"ชุดตัวเลือก\"",
            "Made once, used by many products (sweetness, toppings, cup size). Edits here change every product using the set. Link it in the product editor, or select several products and use Option set.",
        ),
    );
    if sets.is_empty() {
        w::state_view(
            ui,
            Icon::Options,
            lang.text("ยังไม่มีชุดตัวเลือก", "No option sets yet"),
            lang.text(
                "เช่น ความหวาน (หวานปกติ หวานน้อย ไม่หวาน) ที่เครื่องดื่มหลายอย่างใช้ร่วมกัน",
                "e.g. Sweetness (normal, less, none) shared by many drinks.",
            ),
        );
        return;
    }
    let out = egui::ScrollArea::vertical()
        .id_salt("option-sets")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (set, used) in &sets {
                ui.horizontal(|ui| {
                    let text_w = (ui.available_width() - 160.0).max(140.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(text_w, 0.0),
                        egui::Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(text_w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.horizontal(|ui| {
                                w::name_label(
                                    ui,
                                    &set.name,
                                    type_scale::BODY_STRONG,
                                    t.p.fg_default,
                                );
                                ui.label(t.muted(
                                    format!(
                                        "· {} {}",
                                        format::count(*used),
                                        lang.text(
                                            "สินค้าใช้",
                                            if *used == 1 { "product" } else { "products" }
                                        )
                                    ),
                                    type_scale::CAPTION,
                                ));
                            });
                            ui.add(
                                egui::Label::new(t.muted(summary(set, lang), type_scale::CAPTION))
                                    .wrap(),
                            );
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
                                app.option_sets.delete = Some(set.clone());
                            }
                            if w::button(lang.text("แก้ไข", "Edit"))
                                .ghost()
                                .size(w::Size::Sm)
                                .show(ui)
                                .clicked()
                            {
                                app.error = None;
                                app.option_sets.show_errors = false;
                                app.option_sets.editor = Some(vec![GroupDraft::from(set)]);
                            }
                        });
                    });
                });
                w::divider(ui);
            }
        });
    w::scroll_fade(ui, &out, t.p.bg_surface);
}

pub fn dialogs(app: &mut App, ctx: &egui::Context) {
    editor_dialog(app, ctx);
    delete_dialog(app, ctx);
    bulk_dialog(app, ctx);
}
fn editor_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(mut groups) = app.option_sets.editor.take() else {
        return;
    };
    let lang = app.lang;
    let exists = groups
        .first()
        .is_some_and(|g| app.data.option_sets.iter().any(|(s, _)| s.id == g.id()));
    let built = groups
        .first()
        .and_then(GroupDraft::group)
        .filter(|g| pos_domain::validate_groups(std::slice::from_ref(g)).is_ok());
    let errors = app.option_sets.show_errors;
    let response = w::dialog(ctx, "option-set-editor", 520.0, |ui| {
        w::dialog_title(
            ui,
            if exists {
                lang.text("แก้ไขชุดตัวเลือก", "Edit option set")
            } else {
                lang.text("เพิ่มชุดตัวเลือก", "Add option set")
            },
            false,
        );
        egui::ScrollArea::vertical()
            .max_height((ctx.content_rect().height() - 240.0).clamp(200.0, 560.0))
            .show(ui, |ui| {
                crate::catalog_ui::options_editor(
                    ui,
                    &Theme::current(ctx),
                    lang,
                    &mut groups,
                    errors,
                    true,
                );
            });
        if exists {
            w::help_text(
                ui,
                lang.text(
                    "ทุกสินค้าที่ใช้ชุดนี้เปลี่ยนตามทันที ใบเสร็จเก่าไม่เปลี่ยน",
                    "Every product using this set changes at once; past receipts do not.",
                ),
            );
        }
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("บันทึก", "Save"))
                .primary()
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
            match built {
                Some(set) => app.send(Command::SaveOptionSet(set)),
                None => app.option_sets.show_errors = true,
            }
            app.option_sets.editor = Some(groups);
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.option_sets.editor = Some(groups),
    }
}
fn delete_dialog(app: &mut App, ctx: &egui::Context) {
    let Some(set) = app.option_sets.delete.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let used = app
        .data
        .option_sets
        .iter()
        .find(|(s, _)| s.id == set.id)
        .map_or(0, |(_, n)| *n);
    let response = w::dialog(ctx, "option-set-delete", 440.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} \"{}\"?",
                lang.text("ลบชุดตัวเลือก", "Delete option set"),
                set.name
            ),
            true,
        );
        ui.add(
            egui::Label::new(t.text(
                format!(
                    "{} {} {}",
                    lang.text("สินค้า", "It is removed from"),
                    format::count(used),
                    lang.text(
                        "รายการจะไม่มีตัวเลือกชุดนี้อีก ใบเสร็จเก่าไม่เปลี่ยน",
                        "products; past receipts do not change."
                    )
                ),
                type_scale::BODY,
            ))
            .wrap(),
        );
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ลบ", "Delete"))
                .danger()
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
    match response.inner {
        Some(true) => {
            app.send(Command::DeleteOptionSet(set.id));
            app.option_sets.delete = Some(set);
        }
        Some(false) => {}
        None if response.should_close() => {}
        None => app.option_sets.delete = Some(set),
    }
}
/// Bulk bar → Option set: link or unlink one set on the selected products.
fn bulk_dialog(app: &mut App, ctx: &egui::Context) {
    let Some((mut chosen, mut link)) = app.option_sets.bulk.take() else {
        return;
    };
    let lang = app.lang;
    let t = Theme::current(ctx);
    let ids: Vec<Uuid> = app.catalog.selected.iter().copied().collect();
    let response = w::dialog(ctx, "option-set-bulk", 460.0, |ui| {
        w::dialog_title(
            ui,
            &format!(
                "{} · {} {}",
                lang.text("ชุดตัวเลือก", "Option set"),
                ids.len(),
                lang.text("สินค้า", "products")
            ),
            false,
        );
        if app.data.option_sets.is_empty() {
            ui.label(t.muted(
                lang.text(
                    "สร้างชุดตัวเลือกก่อนที่ สินค้า → จัดการ → ชุดตัวเลือก",
                    "Create a set first in Products → Manage → Option sets.",
                ),
                type_scale::BODY,
            ));
        }
        ui.horizontal_wrapped(|ui| {
            for (set, _) in &app.data.option_sets {
                if w::chip(ui, &set.name, chosen == Some(set.id), false).clicked() {
                    chosen = Some(set.id);
                }
            }
        });
        w::segmented(
            ui,
            &mut link,
            &[
                (true, lang.text("ใส่ชุดนี้", "Add this set")),
                (false, lang.text("เอาชุดนี้ออก", "Remove this set")),
            ],
        );
        if let Some(e) = &app.error {
            w::inline_error(ui, e);
        }
        let mut out = None;
        w::dialog_actions(ui, |ui| {
            if w::button(lang.text("ทำกับสินค้าที่เลือก", "Apply to selected"))
                .primary()
                .enabled(chosen.is_some() && !ids.is_empty() && !app.busy)
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
            if let Some(set) = chosen {
                app.send(Command::LinkOptionSet(set, ids, link));
            }
            app.option_sets.bulk = Some((chosen, link));
        }
        Some(false) => app.error = None,
        None if response.should_close() => {}
        None => app.option_sets.bulk = Some((chosen, link)),
    }
}
