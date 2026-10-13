//! Kitchen screen: a second window (for a second monitor) listing open
//! kitchen tickets oldest first. Tap a line to tick it cooked; "Done" bumps
//! the whole ticket; recently bumped tickets can be recalled.
use crate::{
    app::App,
    ui::{Theme, space, type_scale, w},
    worker::Command,
};
use eframe::egui::{self, Align, RichText, Sense, Stroke, vec2};
use pos_domain::KitchenTicket;

#[derive(Default)]
pub struct KitchenState {
    pub show_done: bool,
}

/// Minutes waited, and whether it is late (10 min) or very late (20 min).
fn age(ticket: &KitchenTicket, now: i64) -> (i64, u8) {
    let min = (now - ticket.created_at).max(0) / 60;
    (
        min,
        if min >= 20 {
            2
        } else if min >= 10 {
            1
        } else {
            0
        },
    )
}

pub fn window(app: &mut App, ctx: &egui::Context) {
    if !app.kitchen_open {
        return;
    }
    let title = format!("{} · {}", app.shop.name, app.lang.text("จอครัว", "Kitchen"));
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("kitchen-screen"),
        egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([640.0, 480.0]),
        |ui, _class| {
            if ui.input(|i| i.viewport().close_requested()) {
                app.kitchen_open = false;
            }
            // Waiting times tick even when nothing else happens.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(20));
            let t = Theme::of(ui);
            egui::Frame::new()
                .fill(t.p.bg_canvas)
                .inner_margin(egui::Margin::same(space::S3 as i8))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    board(app, ui);
                });
        },
    );
}

fn board(app: &mut App, ui: &mut egui::Ui) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let open: Vec<KitchenTicket> = app
        .data
        .kitchen
        .iter()
        .filter(|k| k.bumped_at.is_none())
        .cloned()
        .collect();
    ui.horizontal(|ui| {
        w::page_title(ui, lang.text("ครัว", "Kitchen"));
        ui.label(t.muted(
            format!("{} {}", open.len(), lang.text("ใบที่รอ", "waiting")),
            type_scale::BODY,
        ));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            let label = if app.kitchen.show_done {
                lang.text("กลับไปใบที่รอ", "Back to waiting")
            } else {
                lang.text("ใบที่เสร็จล่าสุด", "Recently done")
            };
            if w::button(label).ghost().show(ui).clicked() {
                app.kitchen.show_done = !app.kitchen.show_done;
            }
        });
    });
    ui.add_space(space::S2);
    let tickets: Vec<KitchenTicket> = if app.kitchen.show_done {
        app.data
            .kitchen
            .iter()
            .filter(|k| k.bumped_at.is_some())
            .cloned()
            .collect()
    } else {
        open
    };
    if tickets.is_empty() {
        w::state_view(
            ui,
            crate::ui::Icon::CheckCircle,
            if app.kitchen.show_done {
                lang.text("ยังไม่มีใบที่เสร็จ", "Nothing done yet")
            } else {
                lang.text("ไม่มีออเดอร์ค้าง", "No orders waiting")
            },
            "",
        );
        return;
    }
    let card_w = if t.touch() { 300.0 } else { 270.0 };
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Top-aligned rows: cards of different heights must not stair-step.
            ui.with_layout(
                egui::Layout::left_to_right(Align::Min).with_main_wrap(true),
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(space::S3, space::S3);
                    for ticket in tickets {
                        ui.allocate_ui_with_layout(
                            vec2(card_w, 0.0),
                            egui::Layout::top_down(Align::Min),
                            |ui| card(app, ui, &ticket, now),
                        );
                    }
                },
            );
        });
}

fn card(app: &mut App, ui: &mut egui::Ui, ticket: &KitchenTicket, now: i64) {
    let t = Theme::of(ui);
    let lang = app.lang;
    let (minutes, late) = age(ticket, now);
    let (edge, head_bg) = match late {
        2 => (t.p.danger_solid, t.p.danger_bg),
        1 => (t.p.warning_solid, t.p.warning_bg),
        _ => (t.p.border_default, t.p.bg_sunken),
    };
    egui::Frame::new()
        .fill(t.p.bg_surface)
        .stroke(Stroke::new(if late > 0 { 2.0 } else { 1.0 }, edge))
        .corner_radius(t.control_radius())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = space::S1;
            egui::Frame::new()
                .fill(head_bg)
                .inner_margin(egui::Margin::same(space::S2 as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let name = if ticket.table.is_empty() {
                            format!("#{}", ticket.number)
                        } else {
                            ticket.table.clone()
                        };
                        ui.label(t.text(name, type_scale::TITLE));
                        if ticket.guests > 0 {
                            ui.label(t.muted(
                                format!("{} {}", ticket.guests, lang.text("คน", "guests")),
                                type_scale::LABEL,
                            ));
                        }
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{minutes} {}", lang.text("นาที", "min")))
                                    .font(t.strong_font(type_scale::MONEY))
                                    .color(if late > 0 { edge } else { t.p.fg_muted }),
                            );
                        });
                    });
                    ui.label(t.muted(
                        format!(
                            "#{} · {}{}",
                            ticket.number,
                            crate::ui::format::clock(ticket.created_at),
                            ticket
                                .staff
                                .by
                                .as_ref()
                                .map_or(String::new(), |b| format!(" · {}", b.name))
                        ),
                        type_scale::CAPTION,
                    ));
                });
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(space::S2 as i8, space::S1 as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (i, line) in ticket.lines.iter().enumerate() {
                        let cancel = line.quantity < 0;
                        let mut text = format!("{} × {}", line.quantity.abs(), line.name);
                        if cancel {
                            text = format!("{} {text}", lang.text("ยกเลิก", "CANCEL"));
                        }
                        let mut detail: Vec<String> = line.options.clone();
                        if !line.note.is_empty() {
                            detail.push(line.note.clone());
                        }
                        let color = if cancel {
                            t.p.danger_fg
                        } else if line.done {
                            t.p.fg_subtle
                        } else {
                            t.p.fg_default
                        };
                        let r = ui
                            .scope(|ui| {
                                let mut rt = RichText::new(text)
                                    .font(t.strong_font(type_scale::BODY))
                                    .color(color);
                                if line.done {
                                    rt = rt.strikethrough();
                                }
                                ui.add(egui::Label::new(rt).wrap().sense(Sense::click()));
                                if !detail.is_empty() {
                                    ui.label(
                                        RichText::new(detail.join(" · "))
                                            .font(t.strong_font(type_scale::LABEL))
                                            .color(if cancel {
                                                t.p.danger_fg
                                            } else {
                                                t.p.fg_accent
                                            }),
                                    );
                                }
                            })
                            .response
                            .interact(Sense::click());
                        if r.clicked() && !cancel {
                            app.kitchen_command(Command::TicketLine(ticket.id, i, !line.done));
                        }
                    }
                });
            egui::Frame::new()
                .inner_margin(egui::Margin::same(space::S2 as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let (label, at) = if ticket.bumped_at.is_some() {
                        (lang.text("เรียกคืน", "Recall"), None)
                    } else {
                        (
                            lang.text("เสร็จทั้งใบ", "Done"),
                            Some(time::OffsetDateTime::now_utc().unix_timestamp()),
                        )
                    };
                    let r = w::button(label).fill().show(ui);
                    if r.clicked() {
                        app.kitchen_command(Command::BumpTicket(ticket.id, at));
                    }
                });
        });
}
