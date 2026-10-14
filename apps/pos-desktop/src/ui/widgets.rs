//! Stateless design-system primitives (design/docs/03 Components).
//! Each widget reads `Theme::of(ui)`, returns a `Response` (or a changed flag),
//! and leaves data changes to the calling screen.
use super::{
    icons::{self, Icon},
    theme::Theme,
    tokens::{Metrics, radius, space, type_scale},
};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Galley, Id, Pos2, Rect, Response, RichText, Sense,
    Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};
use std::sync::Arc;

const DISABLED: f32 = 0.45;

fn faded(color: Color32, enabled: bool) -> Color32 {
    if enabled {
        color
    } else {
        color.gamma_multiply(DISABLED)
    }
}
/// Blue ring outside the control, keyboard focus only (Tab navigation).
pub fn focus_ring(ui: &Ui, rect: Rect, rounding: CornerRadius, response: &Response) {
    if response.has_focus() {
        let t = Theme::of(ui);
        ui.painter().rect_stroke(
            rect.expand(3.0),
            rounding + 3,
            Stroke::new(2.0, t.p.focus_ring),
            StrokeKind::Middle,
        );
    }
}
fn galley(ui: &Ui, text: impl Into<String>, font: FontId, color: Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.into(), font, color)
}
fn galley_wrapped(
    ui: &Ui,
    text: impl Into<String>,
    font: FontId,
    color: Color32,
    width: f32,
) -> Arc<Galley> {
    super::text::wrapped(ui.painter(), &text.into(), font, color, width, 0).0
}

// ───────────────────────────── Button ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Ghost,
    Danger,
    DangerQuiet,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Sm,
    Md,
    Lg,
    Xl,
}
pub struct Button {
    label: String,
    money: Option<String>,
    icon: Option<Icon>,
    kind: Kind,
    size: Size,
    shortcut: Option<&'static str>,
    enabled: bool,
    loading: bool,
    fill: bool,
    tooltip: Option<String>,
}
pub fn button(label: impl Into<String>) -> Button {
    Button {
        label: label.into(),
        money: None,
        icon: None,
        kind: Kind::Secondary,
        size: Size::Md,
        shortcut: None,
        enabled: true,
        loading: false,
        fill: false,
        tooltip: None,
    }
}
impl Button {
    pub fn primary(mut self) -> Self {
        self.kind = Kind::Primary;
        self
    }
    pub fn ghost(mut self) -> Self {
        self.kind = Kind::Ghost;
        self
    }
    pub fn danger(mut self) -> Self {
        self.kind = Kind::Danger;
        self
    }
    pub fn danger_quiet(mut self) -> Self {
        self.kind = Kind::DangerQuiet;
        self
    }
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn shortcut(mut self, key: &'static str) -> Self {
        self.shortcut = Some(key);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }
    /// Stretch to the available width (commit buttons at the bottom of a panel).
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }
    /// Full amount appended in the money font: `รับเงินสด ฿1,323.50`.
    pub fn money(mut self, amount: impl Into<String>) -> Self {
        self.money = Some(amount.into());
        self
    }
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
    pub fn show(self, ui: &mut Ui) -> Response {
        let t = Theme::of(ui);
        let p = t.p;
        let enabled = self.enabled && ui.is_enabled() && !self.loading;
        let height = match self.size {
            Size::Sm => t.m.control_sm,
            Size::Md => t.m.control_md,
            Size::Lg => t.m.control_lg,
            Size::Xl => t.m.control_lg,
        };
        let height = if t.touch() {
            height.max(Metrics::TOUCH_TARGET_MIN)
        } else {
            height
        };
        let style = match self.size {
            Size::Xl => type_scale::TITLE,
            Size::Sm => type_scale::LABEL,
            _ => type_scale::BODY,
        };
        // A disabled commit/destructive button is grey, never a faded
        // accent (design/docs/02: disabled states).
        let solid = matches!(self.kind, Kind::Primary | Kind::Danger);
        let (bg, fg, stroke) = match self.kind {
            _ if solid && !enabled => (p.bg_sunken, p.fg_subtle, Stroke::NONE),
            Kind::Primary => (p.accent_bg, p.fg_on_accent, Stroke::NONE),
            Kind::Secondary => (
                p.bg_raised,
                p.fg_default,
                Stroke::new(1.0, p.border_control),
            ),
            Kind::Ghost => (Color32::TRANSPARENT, p.fg_default, Stroke::NONE),
            Kind::Danger => (p.danger_solid, p.fg_on_danger, Stroke::NONE),
            Kind::DangerQuiet => (
                Color32::TRANSPARENT,
                p.danger_fg,
                Stroke::new(1.0, p.border_default),
            ),
        };
        let mut job = LayoutJob::default();
        job.append(
            &self.label,
            0.0,
            egui::TextFormat::simple(t.strong_font(style), fg),
        );
        if let Some(money) = &self.money {
            let money_style = if self.size == Size::Xl {
                type_scale::MONEY_LG
            } else {
                type_scale::MONEY
            };
            job.append(
                money,
                if self.label.is_empty() { 0.0 } else { 8.0 },
                egui::TextFormat::simple(t.strong_font(money_style), fg),
            );
        }
        let text = ui.painter().layout_job(job);
        let icon_size = if self.size == Size::Sm {
            16.0
        } else {
            t.m.icon
        };
        let hint = self
            .shortcut
            .filter(|_| t.m.show_shortcut_hints)
            .map(|k| galley(ui, k, t.font(type_scale::MONEY_SM), fg));
        let pad = if self.size == Size::Sm {
            space::S2 + 2.0
        } else if t.touch() {
            space::S4 + 4.0
        } else {
            space::S4
        };
        let gap = space::S2;
        let mut content = text.size().x;
        if self.icon.is_some() {
            content += icon_size + if text.size().x > 0.0 { gap } else { 0.0 };
        }
        if let Some(h) = &hint {
            content += gap + 4.0 + h.size().x + 8.0;
        }
        let mut width = (content + pad * 2.0).max(height);
        if self.fill {
            width = width.max(ui.available_width());
        }
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
        let a11y = match &self.money {
            Some(m) => format!("{} {m}", self.label),
            None => self.label.clone(),
        };
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &a11y));
        if ui.is_rect_visible(rect) {
            let rounding = t.control_radius();
            let bg = if !enabled {
                bg
            } else if response.is_pointer_button_down_on() {
                match self.kind {
                    Kind::Primary => p.accent_bg_pressed,
                    Kind::Danger => p.danger_fg,
                    _ => p.bg_pressed,
                }
            } else if response.hovered() {
                match self.kind {
                    Kind::Primary => p.accent_bg_hover,
                    Kind::Danger => p.danger_solid.gamma_multiply(0.9),
                    _ => p.bg_sunken,
                }
            } else {
                bg
            };
            let fade = enabled || solid;
            ui.painter().rect(
                rect,
                rounding,
                faded(bg, fade),
                Stroke::new(stroke.width, faded(stroke.color, fade)),
                StrokeKind::Inside,
            );
            let fg = faded(fg, fade);
            let mut x = rect.center().x - content / 2.0;
            if self.loading {
                spinner(ui, rect.center(), height * 0.36, fg);
            } else {
                if let Some(icon) = self.icon {
                    icons::paint_sized(
                        ui.painter(),
                        icon,
                        pos2(x + icon_size / 2.0, rect.center().y),
                        icon_size,
                        fg,
                    );
                    x += icon_size + if text.size().x > 0.0 { gap } else { 0.0 };
                }
                let text_size = text.size();
                ui.painter().galley_with_override_text_color(
                    pos2(x, rect.center().y - text_size.y / 2.0),
                    text,
                    fg,
                );
                x += text_size.x;
                if let Some(h) = hint {
                    let box_rect = Rect::from_min_size(
                        pos2(x + gap + 2.0, rect.center().y - h.size().y / 2.0 - 2.0),
                        h.size() + vec2(8.0, 4.0),
                    );
                    ui.painter().rect_stroke(
                        box_rect,
                        t.radius(radius::XS),
                        Stroke::new(1.0, fg.gamma_multiply(0.55)),
                        StrokeKind::Inside,
                    );
                    ui.painter().galley_with_override_text_color(
                        box_rect.min + vec2(4.0, 2.0),
                        h,
                        fg,
                    );
                }
            }
            focus_ring(ui, rect, rounding, &response);
        }
        let response = if enabled {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        };
        match self.tooltip {
            Some(tip) => response.on_hover_text(tip),
            None => response,
        }
    }
}
/// Square icon-only action. Always has an accessible label and tooltip.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    icon_button_sized(ui, icon, label, None, true)
}
pub fn icon_button_sized(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    size: Option<f32>,
    bordered: bool,
) -> Response {
    let t = Theme::of(ui);
    let side = size.unwrap_or(t.m.control_md);
    let side = if t.touch() {
        side.max(Metrics::TOUCH_TARGET_MIN)
    } else {
        side
    };
    let enabled = ui.is_enabled();
    let (rect, response) = ui.allocate_exact_size(
        vec2(side, side),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    let bg = if !enabled {
        Color32::TRANSPARENT
    } else if response.is_pointer_button_down_on() {
        t.p.bg_pressed
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        Color32::TRANSPARENT
    };
    let rounding = t.control_radius();
    ui.painter().rect(
        rect,
        rounding,
        bg,
        if bordered {
            Stroke::new(1.0, faded(t.p.border_control, enabled))
        } else {
            Stroke::NONE
        },
        StrokeKind::Inside,
    );
    icons::paint_sized(
        ui.painter(),
        icon,
        rect.center(),
        if side < 32.0 { 16.0 } else { t.m.icon },
        faded(t.p.fg_default, enabled),
    );
    focus_ring(ui, rect, rounding, &response);
    response.on_hover_text(label)
}
pub fn spinner(ui: &Ui, center: Pos2, r: f32, color: Color32) {
    let time = ui.input(|i| i.time) as f32;
    let start = time * 5.0;
    let points: Vec<Pos2> = (0..=20)
        .map(|i| {
            let a = start + i as f32 / 20.0 * 4.5;
            center + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.0, color)));
    ui.ctx().request_repaint();
}

// ───────────────────────────── Text fields ─────────────────────────────

pub fn field_label(ui: &mut Ui, label: &str, optional: Option<&str>) {
    let t = Theme::of(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            t.text(label, type_scale::LABEL)
                .font(t.strong_font(type_scale::LABEL)),
        );
        if let Some(note) = optional {
            ui.label(t.muted(note, type_scale::LABEL));
        }
    });
}
pub struct FieldOpts<'a> {
    pub hint: &'a str,
    pub limit: usize,
    pub width: Option<f32>,
    pub invalid: bool,
    pub id: Option<Id>,
    pub enabled: bool,
}
impl Default for FieldOpts<'_> {
    fn default() -> Self {
        Self {
            hint: "",
            limit: 160,
            width: None,
            invalid: false,
            id: None,
            enabled: true,
        }
    }
}
/// Single-line text input at control height.
pub fn text_input(ui: &mut Ui, value: &mut String, opts: FieldOpts<'_>) -> Response {
    let t = Theme::of(ui);
    let width = opts.width.unwrap_or_else(|| ui.available_width());
    let mut edit = egui::TextEdit::singleline(value)
        .hint_text(RichText::new(opts.hint).color(t.p.fg_subtle))
        .font(t.font(type_scale::BODY))
        .margin(egui::Margin::symmetric(space::S3 as i8, 0))
        .vertical_align(Align::Center)
        .min_size(vec2(width, t.m.control_md))
        .desired_width(width - 2.0 * space::S3)
        .char_limit(opts.limit);
    if let Some(id) = opts.id {
        edit = edit.id(id);
    }
    let response = ui.add_enabled(opts.enabled, edit);
    if opts.invalid {
        ui.painter().rect_stroke(
            response.rect,
            t.control_radius(),
            Stroke::new(2.0, t.p.danger_fg),
            StrokeKind::Inside,
        );
    }
    response
}
/// Multi-line text that wraps (long product names, reasons).
pub fn text_area(
    ui: &mut Ui,
    value: &mut String,
    hint: &str,
    limit: usize,
    rows: usize,
) -> Response {
    let t = Theme::of(ui);
    ui.add(
        egui::TextEdit::multiline(value)
            .hint_text(RichText::new(hint).color(t.p.fg_subtle))
            .font(t.font(type_scale::BODY))
            .margin(egui::Margin::symmetric(space::S3 as i8, space::S2 as i8))
            .desired_rows(rows)
            .desired_width(f32::INFINITY)
            .char_limit(limit),
    )
}
pub fn inline_error(ui: &mut Ui, message: &str) {
    let t = Theme::of(ui);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        icons::show(ui, Icon::Alert, 16.0, t.p.danger_fg);
        ui.label(t.text(message, type_scale::LABEL).color(t.p.danger_fg));
    });
}
pub fn help_text(ui: &mut Ui, text: &str) {
    let t = Theme::of(ui);
    ui.add(egui::Label::new(t.muted(text, type_scale::CAPTION)).wrap());
}

pub struct MoneyOpts {
    pub id: Id,
    pub primary: bool,
    pub width: Option<f32>,
    pub large: bool,
    pub invalid: bool,
    pub enabled: bool,
}
/// Right-aligned money in the number font. Selects everything on focus and
/// formats as `1,500.00` on blur; never rounds silently (parse happens upstream).
pub fn money_input(ui: &mut Ui, value: &mut String, opts: MoneyOpts) -> Response {
    let t = Theme::of(ui);
    let width = opts.width.unwrap_or_else(|| ui.available_width());
    let style = if opts.large {
        type_scale::MONEY_LG
    } else {
        type_scale::MONEY
    };
    let height = if opts.large {
        t.m.control_lg
    } else {
        t.m.control_md
    };
    let output = ui
        .add_enabled_ui(opts.enabled, |ui| {
            egui::TextEdit::singleline(value)
                .id(opts.id)
                .hint_text(RichText::new("0.00").color(t.p.fg_subtle))
                .font(t.strong_font(style))
                .horizontal_align(Align::Max)
                .vertical_align(Align::Center)
                .margin(egui::Margin::symmetric(space::S3 as i8, 0))
                .min_size(vec2(width, height))
                .desired_width(width - 2.0 * space::S3)
                .char_limit(20)
                .show(ui)
        })
        .inner;
    let mut state = output.state;
    let response = output.response.response;
    if response.gained_focus() {
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::select_all(&output.galley)));
        state.store(ui.ctx(), response.id);
    }
    if response.changed() {
        value.retain(|c| c.is_ascii_digit() || c == '.' || c == ',');
    }
    if response.lost_focus()
        && let Ok(m) = super::format::parse_money(value)
    {
        *value = super::format::money(m);
    }
    let stroke = if opts.invalid {
        Some(Stroke::new(2.0, t.p.danger_fg))
    } else if opts.primary && !response.has_focus() {
        Some(Stroke::new(2.0, t.p.fg_default))
    } else {
        None
    };
    if let Some(stroke) = stroke {
        ui.painter().rect_stroke(
            response.rect,
            t.control_radius(),
            stroke,
            StrokeKind::Inside,
        );
    }
    response
}

/// Search box with icon and shortcut hint. `Esc` clears it.
pub fn search_input(
    ui: &mut Ui,
    value: &mut String,
    hint: &str,
    shortcut: Option<&str>,
    id: Id,
) -> Response {
    let t = Theme::of(ui);
    let width = ui.available_width();
    let height = t.m.control_md;
    let (outer, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let rounding = t.control_radius();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(outer)
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    let icon_w = 36.0;
    let hint_w = shortcut
        .filter(|_| t.m.show_shortcut_hints)
        .map_or(0.0, |k| {
            galley(&child, k, t.font(type_scale::MONEY_SM), t.p.fg_muted)
                .size()
                .x
                + 22.0
        });
    child.add_space(icon_w);
    let response = child.add(
        egui::TextEdit::singleline(value)
            .id(id)
            .frame(egui::Frame::NONE)
            .hint_text(RichText::new(hint).color(t.p.fg_subtle))
            .font(t.font(type_scale::BODY))
            .vertical_align(Align::Center)
            .min_size(vec2(width - icon_w - hint_w - 4.0, height))
            .desired_width(width - icon_w - hint_w - 4.0)
            .char_limit(120),
    );
    let focused = response.has_focus();
    // The sell search keeps focus for scanners, so focus is a firm border,
    // not the blue keyboard ring that would stay lit all day.
    ui.painter().rect_stroke(
        outer,
        rounding,
        if focused {
            Stroke::new(1.5, t.p.border_selected)
        } else {
            Stroke::new(1.0, t.p.border_control)
        },
        StrokeKind::Inside,
    );
    icons::paint_sized(
        ui.painter(),
        Icon::Search,
        pos2(outer.left() + 18.0, outer.center().y),
        18.0,
        t.p.fg_muted,
    );
    if let Some(key) = shortcut.filter(|_| t.m.show_shortcut_hints) {
        let g = galley(ui, key, t.font(type_scale::MONEY_SM), t.p.fg_muted);
        let r = Rect::from_min_size(
            pos2(
                outer.right() - g.size().x - 18.0,
                outer.center().y - g.size().y / 2.0 - 2.0,
            ),
            g.size() + vec2(8.0, 4.0),
        );
        ui.painter().rect_stroke(
            r,
            radius::XS,
            Stroke::new(1.0, t.p.border_default),
            StrokeKind::Inside,
        );
        ui.painter().galley(r.min + vec2(4.0, 2.0), g, t.p.fg_muted);
    }
    if focused && ui.input(|i| i.key_pressed(egui::Key::Escape)) && !value.is_empty() {
        value.clear();
    }
    response
}

// ───────────────────────────── Selection ─────────────────────────────

/// 2–5 options with an immediate effect. Returns true when the value changed.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let t = Theme::of(ui);
    let height = t.m.control_md;
    let font = t.strong_font(type_scale::LABEL);
    let pad = if t.touch() { 16.0 } else { 12.0 };
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, label)| galley(ui, *label, font.clone(), t.p.fg_default))
        .collect();
    let width: f32 = galleys.iter().map(|g| g.size().x + pad * 2.0).sum::<f32>() + 8.0;
    let (outer, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter()
        .rect_filled(outer, t.control_radius(), t.p.bg_sunken);
    let mut x = outer.left() + 4.0;
    let mut changed = false;
    for ((option, label), g) in options.iter().zip(galleys) {
        let w = g.size().x + pad * 2.0;
        let rect = Rect::from_min_size(pos2(x, outer.top() + 4.0), vec2(w, height - 8.0));
        let id = ui.id().with(("segment", label));
        // Paint the inset segment, but hit-test the full control height (≥ 44 in touch).
        let hit = Rect::from_min_size(pos2(x, outer.top()), vec2(w, height));
        let response = ui.interact(hit, id, Sense::click());
        let active = *value == *option;
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::RadioButton, ui.is_enabled(), active, *label)
        });
        if active {
            ui.painter().rect(
                rect,
                CornerRadius::same(if t.touch() { 6 } else { 4 }),
                t.p.bg_raised,
                Stroke::new(1.0, t.p.border_default),
                StrokeKind::Inside,
            );
        } else if response.hovered() {
            ui.painter().rect_filled(rect, t.radius(4), t.p.bg_hover);
        }
        let color = if active { t.p.fg_default } else { t.p.fg_muted };
        ui.painter()
            .galley_with_override_text_color(rect.center() - g.size() / 2.0, g, color);
        focus_ring(ui, rect, CornerRadius::same(t.radius(4)), &response);
        if response.clicked() && !active {
            *value = *option;
            changed = true;
        }
        x += w;
    }
    changed
}
/// Pill chip; `selected` fills it with the inverse colour.
pub fn chip(ui: &mut Ui, label: &str, selected: bool, money: bool) -> Response {
    let t = Theme::of(ui);
    let font = if money {
        t.strong_font(type_scale::MONEY)
    } else {
        t.strong_font(type_scale::LABEL)
    };
    let fg = if selected {
        t.p.fg_inverse
    } else {
        t.p.fg_default
    };
    let g = galley(ui, label, font, fg);
    let height = if t.touch() {
        Metrics::TOUCH_TARGET_MIN
    } else {
        t.m.control_sm.max(32.0)
    };
    let (rect, response) = ui.allocate_exact_size(vec2(g.size().x + 28.0, height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
    let bg = if selected {
        t.p.bg_inverse
    } else if response.hovered() {
        t.p.bg_hover
    } else {
        t.p.bg_raised
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(radius::FULL),
        bg,
        if selected {
            Stroke::NONE
        } else {
            Stroke::new(1.0, t.p.border_control)
        },
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
    focus_ring(ui, rect, CornerRadius::same(radius::FULL), &response);
    response
}
/// Chip with an optional category dot (touch category row).
pub fn chip_dot(ui: &mut Ui, label: &str, selected: bool, dot: Option<Color32>) -> Response {
    let t = Theme::of(ui);
    let fg = if selected {
        t.p.fg_chrome
    } else {
        t.p.fg_default
    };
    let g = galley(ui, label, t.strong_font(type_scale::LABEL), fg);
    let height = if t.touch() { 40.0 } else { 28.0 };
    let pad = if t.touch() { 14.0 } else { 10.0 };
    let dot_w = if dot.is_some() { 8.0 + 6.0 } else { 0.0 };
    let (rect, response) =
        ui.allocate_exact_size(vec2(g.size().x + pad * 2.0 + dot_w, height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, label));
    let (bg, stroke) = if selected {
        (t.p.bg_chrome_raised, Stroke::NONE)
    } else if response.hovered() {
        (t.p.bg_hover, Stroke::new(1.0, t.p.border_default))
    } else {
        (t.p.bg_raised, Stroke::new(1.0, t.p.border_default))
    };
    let r = CornerRadius::same(t.radius(radius::FULL));
    ui.painter().rect(rect, r, bg, stroke, StrokeKind::Inside);
    if let Some(c) = dot {
        ui.painter()
            .circle_filled(pos2(rect.left() + pad + 4.0, rect.center().y), 4.0, c);
    }
    ui.painter().galley(
        pos2(
            rect.left() + pad + dot_w,
            rect.center().y - g.size().y / 2.0,
        ),
        g,
        fg,
    );
    focus_ring(ui, rect, r, &response);
    response
}
/// Brand accent presets as colour swatches with names; true when changed.
pub fn accent_picker(
    ui: &mut Ui,
    value: &mut crate::ui::theme::AccentPreset,
    name: impl Fn(crate::ui::theme::AccentPreset) -> String,
) -> bool {
    let t = Theme::of(ui);
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = space::S2;
        for accent in crate::ui::theme::ACCENTS {
            let label = name(accent);
            let selected = *value == accent;
            let g = galley(ui, &label, t.strong_font(type_scale::LABEL), t.p.fg_default);
            let dot = if t.touch() { 18.0 } else { 14.0 };
            let height = if t.touch() {
                Metrics::TOUCH_TARGET_MIN
            } else {
                t.m.control_md
            };
            let width = space::S3 + dot + space::S2 + g.size().x + space::S3;
            let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::RadioButton, true, selected, &label)
            });
            let r = CornerRadius::same(t.radius(radius::MD));
            let (bg, stroke) = if selected {
                (t.p.bg_selected, Stroke::new(2.0, t.p.border_selected))
            } else if response.hovered() {
                (t.p.bg_hover, Stroke::new(1.0, t.p.border_default))
            } else {
                (t.p.bg_raised, Stroke::new(1.0, t.p.border_default))
            };
            ui.painter().rect(rect, r, bg, stroke, StrokeKind::Inside);
            let c = egui::pos2(rect.left() + space::S3 + dot / 2.0, rect.center().y);
            ui.painter().circle(
                c,
                dot / 2.0,
                crate::ui::theme::accent_color(accent),
                Stroke::new(1.0, t.p.border_default),
            );
            ui.painter().galley(
                egui::pos2(
                    c.x + dot / 2.0 + space::S2,
                    rect.center().y - g.size().y / 2.0,
                ),
                g,
                t.p.fg_default,
            );
            focus_ring(ui, rect, r, &response);
            if response.clicked() && !selected {
                *value = accent;
                changed = true;
            }
        }
    });
    changed
}
/// Chips that share the row width equally (quick cash).
pub fn chip_row(ui: &mut Ui, labels: &[(String, bool)]) -> Option<usize> {
    let t = Theme::of(ui);
    let gap = space::S2;
    let n = labels.len().max(1) as f32;
    let width = ((ui.available_width() - gap * (n - 1.0)) / n).floor();
    let height = if t.touch() { 44.0 } else { 34.0 };
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (i, (label, selected)) in labels.iter().enumerate() {
            let fg = if *selected {
                t.p.fg_inverse
            } else {
                t.p.fg_default
            };
            let g = galley(ui, label.clone(), t.strong_font(type_scale::MONEY), fg);
            let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
            response
                .widget_info(|| WidgetInfo::selected(WidgetType::Button, true, *selected, label));
            let bg = if *selected {
                t.p.bg_inverse
            } else if response.hovered() {
                t.p.bg_hover
            } else {
                t.p.bg_surface
            };
            ui.painter().rect(
                rect,
                CornerRadius::same(radius::FULL),
                bg,
                if *selected {
                    Stroke::NONE
                } else {
                    Stroke::new(1.0, t.p.border_control)
                },
                StrokeKind::Inside,
            );
            ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
            focus_ring(ui, rect, CornerRadius::same(radius::FULL), &response);
            if response.clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}
/// Immediate on/off setting.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let t = Theme::of(ui);
    let size = vec2(44.0, 26.0);
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(size.x, t.m.control_md.max(size.y)), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, *on, label));
    let track = Rect::from_center_size(rect.center(), size);
    let how = ui.ctx().animate_bool_responsive(response.id, *on);
    ui.painter().rect(
        track,
        CornerRadius::same(radius::FULL),
        if *on {
            t.p.success_solid
        } else {
            t.p.bg_pressed
        },
        Stroke::new(
            1.0,
            if *on {
                t.p.success_solid
            } else {
                t.p.border_control
            },
        ),
        StrokeKind::Inside,
    );
    let x = egui::lerp(track.left() + 13.0..=track.right() - 13.0, how);
    ui.painter()
        .circle_filled(pos2(x, track.center().y), 10.0, t.p.bg_surface);
    focus_ring(ui, track, CornerRadius::same(radius::FULL), &response);
    response.on_hover_text(label)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Off,
    On,
    Mixed,
}
/// Square checkbox drawn to spec; `Mixed` for a partially selected header.
pub fn checkbox(ui: &mut Ui, state: Check, label: &str) -> Response {
    let t = Theme::of(ui);
    let side = if t.touch() { 44.0 } else { 28.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, true, state != Check::Off, label)
    });
    let r = Rect::from_center_size(rect.center(), vec2(18.0, 18.0));
    let filled = state != Check::Off;
    ui.painter().rect(
        r,
        CornerRadius::same(t.radius(radius::XS)),
        if filled {
            t.p.bg_inverse
        } else {
            t.p.bg_surface
        },
        if filled {
            Stroke::NONE
        } else {
            Stroke::new(1.5, t.p.border_control)
        },
        StrokeKind::Inside,
    );
    match state {
        Check::On => {
            icons::paint_sized(ui.painter(), Icon::Check, r.center(), 16.0, t.p.fg_inverse)
        }
        Check::Mixed => {
            icons::paint_sized(ui.painter(), Icon::Minus, r.center(), 16.0, t.p.fg_inverse)
        }
        Check::Off => {}
    }
    focus_ring(ui, r, CornerRadius::same(t.radius(radius::XS)), &response);
    response.on_hover_text(label)
}
/// Checkbox followed by a clickable text label.
pub fn check_label(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let t = Theme::of(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut r = checkbox(ui, if *value { Check::On } else { Check::Off }, label);
        let text = ui.add(
            egui::Label::new(t.text(label, type_scale::BODY))
                .wrap()
                .sense(Sense::click()),
        );
        if r.clicked() || text.clicked() {
            *value = !*value;
            r.mark_changed();
        }
        r
    })
    .inner
}

// ───────────────────────────── Status ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
    Outline,
}
impl Tone {
    pub fn colors(self, t: &Theme) -> (Color32, Color32) {
        let p = t.p;
        match self {
            Self::Neutral => (p.bg_sunken, p.fg_muted),
            Self::Success => (p.success_bg, p.success_fg),
            Self::Warning => (p.warning_bg, p.warning_fg),
            Self::Danger => (p.danger_bg, p.danger_fg),
            Self::Info => (p.info_bg, p.info_fg),
            Self::Outline => (Color32::TRANSPARENT, p.fg_default),
        }
    }
}
/// Short status text, 1–3 words. Never a button.
pub fn badge(ui: &mut Ui, text: &str, tone: Tone) -> Response {
    let t = Theme::of(ui);
    let (bg, fg) = tone.colors(&t);
    let g = galley(ui, text, t.strong_font(type_scale::CAPTION), fg);
    let (rect, response) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 22.0), Sense::hover());
    ui.painter().rect(
        rect,
        radius::XS,
        bg,
        if tone == Tone::Outline {
            Stroke::new(1.0, t.p.border_control)
        } else {
            Stroke::NONE
        },
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
    response
}
pub fn paint_count(ui: &Ui, center: Pos2, n: u32, inverted: bool) -> Rect {
    let t = Theme::of(ui);
    let (bg, fg) = if inverted {
        (t.p.bg_surface, t.p.fg_default)
    } else {
        (t.p.bg_inverse, t.p.fg_inverse)
    };
    let g = galley(ui, n.to_string(), t.strong_font(type_scale::MONEY_SM), fg);
    let rect = Rect::from_center_size(center, vec2((g.size().x + 10.0).max(22.0), 22.0));
    ui.painter().rect(
        rect,
        radius::XS,
        bg,
        if inverted {
            Stroke::new(1.0, t.p.border_default)
        } else {
            Stroke::NONE
        },
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
    rect
}
// ───────────────────────────── Layout helpers ─────────────────────────────

/// A pane at `rect` (Navigation: bg-surface, header strip, own scrolling).
/// `header` draws into the strip (left to right, centred); `body` gets the
/// rest with no padding.
pub fn pane_at<R>(
    ui: &mut Ui,
    rect: Rect,
    header: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let (mut head_ui, mut body_ui) = pane_uis(ui, rect);
    header(&mut head_ui);
    body(&mut body_ui)
}
/// The pane's header strip and body as separate child Uis.
pub fn pane_uis(ui: &mut Ui, rect: Rect) -> (Ui, Ui) {
    let t = Theme::of(ui);
    let r = CornerRadius::same(t.pane_radius());
    ui.painter().rect_filled(rect, r, t.p.bg_surface);
    let h = t.pane_header_h();
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), h));
    if !t.soft() {
        ui.painter().rect_filled(head, 0, t.p.border_subtle);
    }
    ui.painter().hline(
        head.x_range(),
        head.bottom() - 0.5,
        Stroke::new(
            1.0,
            if t.soft() {
                t.p.border_subtle
            } else {
                t.p.border_default
            },
        ),
    );
    let mut head_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(head.shrink2(vec2(if t.soft() { 6.0 } else { 0.0 }, 0.0)))
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    head_ui.set_clip_rect(head.intersect(ui.clip_rect()));
    head_ui.spacing_mut().item_spacing.x = space::S2;
    let body_rect = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
    let mut body_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(body_rect)
            .layout(egui::Layout::top_down(Align::Min)),
    );
    body_ui.set_clip_rect(body_rect.intersect(ui.clip_rect()));
    (head_ui, body_ui)
}
/// Pane header title (soft: ink; square: muted, like the reference).
pub fn pane_title(ui: &mut Ui, text: &str) {
    let t = Theme::of(ui);
    ui.add_space(if t.soft() { space::S1 } else { 10.0 });
    ui.label(
        RichText::new(text)
            .font(t.strong_font(type_scale::LABEL))
            .color(if t.soft() {
                t.p.fg_default
            } else {
                t.p.fg_muted
            }),
    );
}
/// Right-aligned figure in a pane header (counts, totals).
pub fn pane_count(ui: &mut Ui, text: &str) {
    let t = Theme::of(ui);
    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(if t.soft() { space::S1 } else { 10.0 });
        ui.label(
            RichText::new(text)
                .font(t.font(type_scale::MONEY_SM))
                .color(t.p.fg_muted),
        );
    });
}
/// Tabs inside a pane header (soft: pills; square: flat cells).
pub fn pane_tabs<T: Copy + PartialEq>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str, Option<usize>)],
) -> bool {
    let t = Theme::of(ui);
    let mut changed = false;
    let full = ui.available_height();
    for (v, label, count) in options {
        let on = *value == *v;
        let text = match count {
            Some(n) if *n > 0 => format!("{label} · {n}"),
            _ => (*label).to_owned(),
        };
        let g = galley(
            ui,
            &text,
            t.strong_font(type_scale::LABEL),
            if on { t.p.fg_default } else { t.p.fg_muted },
        );
        let pad = if t.touch() { 18.0 } else { 10.0 };
        let height = if t.soft() {
            (full - 4.0).min(if t.touch() { 30.0 } else { 24.0 })
        } else {
            full
        };
        let (rect, response) =
            ui.allocate_exact_size(vec2(g.size().x + pad * 2.0, height), Sense::click());
        response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, &text));
        if t.soft() {
            if on {
                ui.painter().rect_filled(rect, t.radius(7), t.p.bg_sunken);
            } else if response.hovered() {
                ui.painter().rect_filled(rect, t.radius(7), t.p.bg_hover);
            }
        } else {
            if on {
                ui.painter()
                    .rect_filled(rect.expand2(vec2(0.0, 0.5)), 0, t.p.bg_surface);
            }
            ui.painter().vline(
                rect.right(),
                rect.y_range(),
                Stroke::new(1.0, t.p.border_default),
            );
        }
        ui.painter().galley(
            rect.center() - g.size() / 2.0,
            g,
            if on { t.p.fg_default } else { t.p.fg_muted },
        );
        focus_ring(ui, rect, CornerRadius::same(t.radius(7)), &response);
        if response.clicked() && !on {
            *value = *v;
            changed = true;
        }
    }
    changed
}

pub fn title(ui: &mut Ui, text: &str) -> Response {
    let t = Theme::of(ui);
    ui.add(egui::Label::new(t.text(text, type_scale::TITLE)).wrap())
}
pub fn page_title(ui: &mut Ui, text: &str) -> Response {
    let t = Theme::of(ui);
    ui.add(egui::Label::new(t.text(text, type_scale::TITLE)).wrap())
}
/// A page heading drawn straight on the graphite canvas (outside a pane).
pub fn canvas_title(ui: &mut Ui, text: &str) -> Response {
    let t = Theme::of(ui);
    ui.add(egui::Label::new(t.chrome(text, type_scale::TITLE)).wrap())
}
/// Small uppercase-like section label (`หมวดหมู่`, `มุมมอง`).
pub fn eyebrow(ui: &mut Ui, text: &str) -> Response {
    let t = Theme::of(ui);
    ui.label(
        RichText::new(text)
            .font(t.strong_font(type_scale::CAPTION))
            .color(t.p.fg_muted),
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Plain,
    Strong,
    Danger,
}
/// Label left (muted) · value right in the number font (`ds-kv`).
pub fn kv(ui: &mut Ui, label: &str, value: &str, kind: Value) {
    kv_styled(ui, label, value, kind, type_scale::MONEY);
}
pub fn kv_styled(ui: &mut Ui, label: &str, value: &str, kind: Value, style: type_scale::Style) {
    let t = Theme::of(ui);
    let width = ui.available_width();
    let color = match kind {
        Value::Danger => t.p.danger_fg,
        _ => t.p.fg_default,
    };
    let font = if matches!(kind, Value::Strong) {
        t.strong_font(style)
    } else {
        t.font(style)
    };
    let label_font = if matches!(kind, Value::Strong) {
        t.strong_font(type_scale::BODY)
    } else {
        t.font(type_scale::BODY)
    };
    let value_g = galley(ui, value, font, color);
    let label_g = galley_wrapped(
        ui,
        label,
        label_font,
        if matches!(kind, Value::Strong) {
            t.p.fg_default
        } else {
            t.p.fg_muted
        },
        (width - value_g.size().x - space::S4).max(60.0),
    );
    let height = label_g.size().y.max(value_g.size().y) + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter().galley(
        pos2(rect.left(), rect.center().y - label_g.size().y / 2.0),
        label_g,
        t.p.fg_muted,
    );
    ui.painter().galley(
        pos2(
            rect.right() - value_g.size().x,
            rect.center().y - value_g.size().y / 2.0,
        ),
        value_g,
        color,
    );
}
pub fn divider(ui: &mut Ui) {
    let t = Theme::of(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, t.p.border_subtle),
    );
}
pub fn dashed(ui: &mut Ui) {
    let t = Theme::of(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 9.0), Sense::hover());
    let y = rect.center().y;
    let mut x = rect.left();
    while x < rect.right() {
        ui.painter().line_segment(
            [pos2(x, y), pos2((x + 4.0).min(rect.right()), y)],
            Stroke::new(1.0, t.p.border_control),
        );
        x += 7.0;
    }
}
pub fn steps_width(ui: &Ui, labels: &[&str]) -> f32 {
    let t = Theme::of(ui);
    let text: f32 = labels
        .iter()
        .map(|l| text_width(ui, l, t.strong_font(type_scale::LABEL)))
        .sum();
    let n = labels.len() as f32;
    text + n * 24.0 + (n - 1.0) * 20.0 + (n * 3.0 - 1.0) * space::S2 + 4.0
}
/// Step indicator for 3–5 ordered stages (`✓ นับเงิน — ② ตรวจยอด — ③ ยืนยัน`).
pub fn steps(ui: &mut Ui, labels: &[&str], current: usize) {
    let t = Theme::of(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S2;
        for (i, label) in labels.iter().enumerate() {
            let done = i < current;
            let active = i == current;
            let (rect, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
            if done {
                ui.painter()
                    .circle_filled(rect.center(), 11.0, t.p.bg_inverse);
                icons::paint_sized(
                    ui.painter(),
                    Icon::Check,
                    rect.center(),
                    14.0,
                    t.p.fg_inverse,
                );
            } else {
                ui.painter().circle_stroke(
                    rect.center(),
                    10.5,
                    Stroke::new(
                        if active { 2.0 } else { 1.2 },
                        if active { t.p.fg_default } else { t.p.fg_muted },
                    ),
                );
                let g = galley(
                    ui,
                    (i + 1).to_string(),
                    t.strong_font(type_scale::CAPTION),
                    t.p.fg_default,
                );
                ui.painter()
                    .galley(rect.center() - g.size() / 2.0, g, t.p.fg_default);
            }
            let text = if active {
                t.text(*label, type_scale::LABEL)
                    .font(t.strong_font(type_scale::LABEL))
            } else {
                t.muted(*label, type_scale::LABEL)
            };
            ui.label(text);
            if i + 1 < labels.len() {
                let (r, _) = ui.allocate_exact_size(vec2(20.0, 24.0), Sense::hover());
                ui.painter().line_segment(
                    [r.left_center(), r.right_center()],
                    Stroke::new(1.0, t.p.border_control),
                );
            }
        }
    });
}

// ───────────────────────────── Feedback ─────────────────────────────

/// Persistent problem at the top of the area it concerns.
pub fn banner(ui: &mut Ui, tone: Tone, text: &str, action: Option<&str>) -> bool {
    let t = Theme::of(ui);
    let (bg, fg) = tone.colors(&t);
    let icon = match tone {
        Tone::Danger | Tone::Warning => Icon::Alert,
        Tone::Success => Icon::CheckCircle,
        _ => Icon::Info,
    };
    let mut clicked = false;
    egui::Frame::new()
        .fill(bg)
        .corner_radius(t.radius(radius::MD))
        .inner_margin(egui::Margin::symmetric(space::S3 as i8, space::S2 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    icons::show(ui, icon, 18.0, fg);
                });
                let reserve = action.map_or(0.0, |a| {
                    text_width(ui, a, t.strong_font(type_scale::LABEL)) + space::S6 + space::S2
                });
                ui.allocate_ui_with_layout(
                    vec2((ui.available_width() - reserve).max(80.0), 0.0),
                    egui::Layout::top_down(Align::Min),
                    |ui| {
                        ui.add(egui::Label::new(t.text(text, type_scale::BODY).color(fg)).wrap());
                    },
                );
                if let Some(action) = action {
                    // A fixed-width column: a right-to-left layout here would
                    // claim all remaining height and stretch the banner.
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), 0.0),
                        egui::Layout::right_to_left(Align::Min),
                        |ui| {
                            clicked = button(action).ghost().size(Size::Sm).show(ui).clicked();
                        },
                    );
                }
            });
        });
    clicked
}
/// Empty / no results / error state inside a content area.
pub fn state_view(ui: &mut Ui, icon: Icon, title: &str, body: &str) {
    let t = Theme::of(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(space::S8);
        let (rect, _) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, t.radius(radius::MD), t.p.bg_sunken);
        icons::paint_sized(ui.painter(), icon, rect.center(), 24.0, t.p.fg_muted);
        ui.add_space(space::S3);
        ui.label(t.text(title, type_scale::TITLE));
        if !body.is_empty() {
            ui.add(egui::Label::new(t.muted(body, type_scale::BODY)).wrap());
        }
        ui.add_space(space::S3);
    });
}

/// KPI tile: label, money value, and a line that explains the number.
pub fn stat_tile(
    ui: &mut Ui,
    label: &str,
    value: &str,
    danger: bool,
    sub: &[(String, Option<Tone>)],
) {
    let t = Theme::of(ui);
    let width = ui.available_width();
    t.surface().show(ui, |ui| {
        ui.set_width(width - space::S4 * 2.0);
        ui.set_min_height(76.0);
        ui.spacing_mut().item_spacing.y = space::S1;
        ui.add(egui::Label::new(t.muted(label, type_scale::LABEL)).truncate());
        ui.add(
            egui::Label::new(
                RichText::new(value)
                    .font(t.strong_font(type_scale::MONEY_LG))
                    .color(if danger {
                        t.p.danger_fg
                    } else {
                        t.p.fg_default
                    }),
            )
            .wrap_mode(egui::TextWrapMode::Extend),
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (text, tone) in sub {
                let color = match tone {
                    Some(Tone::Success) => t.p.success_fg,
                    Some(Tone::Danger) => t.p.danger_fg,
                    _ => t.p.fg_muted,
                };
                let font = if tone.is_some() {
                    t.strong_font(type_scale::CAPTION)
                } else {
                    t.font(type_scale::CAPTION)
                };
                ui.label(RichText::new(text).font(font).color(color));
            }
        });
    });
}

// ───────────────────────────── Overlays ─────────────────────────────

/// Blocking dialog (≤ 480 wide). Esc / scrim click report `should_close`.
pub fn dialog<R>(
    ctx: &egui::Context,
    id: &str,
    width: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> egui::ModalResponse<R> {
    let t = Theme::current(ctx);
    egui::Modal::new(Id::new(id))
        .backdrop_color(t.p.overlay_scrim)
        .frame(
            t.raised(3)
                .inner_margin(egui::Margin::same(space::S6 as i8)),
        )
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).min(width));
            ui.spacing_mut().item_spacing.y = space::S3;
            content(ui)
        })
}
/// Dialog heading with optional danger icon.
pub fn dialog_title(ui: &mut Ui, text: &str, danger: bool) {
    let t = Theme::of(ui);
    ui.horizontal(|ui| {
        if danger {
            let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 18.0, t.p.danger_bg);
            icons::paint_sized(
                ui.painter(),
                Icon::Alert,
                rect.center(),
                20.0,
                t.p.danger_fg,
            );
        }
        ui.add(egui::Label::new(t.text(text, type_scale::TITLE)).wrap());
    });
}
/// Footer row: safe action left, commit action right (ActionGroup order).
pub fn dialog_actions(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let t = Theme::of(ui);
    ui.add_space(space::S2);
    // A fixed row height: an unbounded right-to-left layout would take all
    // remaining height of the modal and centre the buttons in it.
    let height = t.m.control_lg.max(Metrics::TOUCH_TARGET_MIN);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        egui::Layout::right_to_left(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = space::S2;
            add(ui);
        },
    );
}
pub fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    galley(ui, text, font, Color32::WHITE).size().x
}
/// Chevron for `egui::ComboBox::icon` so selects match the line icon set.
pub fn chevron(ui: &Ui, rect: Rect, visuals: &egui::style::WidgetVisuals, open: bool) {
    let _ = open;
    icons::paint_sized(
        ui.painter(),
        Icon::ChevronDown,
        rect.center(),
        16.0,
        visuals.fg_stroke.color,
    );
}
/// A product, bill or category name wrapped with Thai-safe breaks.
pub fn name_label(ui: &mut Ui, text: &str, style: type_scale::Style, color: Color32) -> Response {
    let t = Theme::of(ui);
    let (g, _) = super::text::wrapped(
        ui.painter(),
        text,
        t.font(style),
        color,
        ui.available_width(),
        0,
    );
    ui.add(egui::Label::new(g))
}
/// Fades the bottom edge of a scroll area while more content sits below it.
pub fn scroll_fade<R>(ui: &Ui, out: &egui::scroll_area::ScrollAreaOutput<R>, color: Color32) {
    let rect = out.inner_rect;
    let hidden = out.content_size.y - out.state.offset.y - rect.height();
    if hidden <= 1.0 {
        return;
    }
    let height = 28.0_f32.min(hidden + 8.0);
    let steps = 8;
    for i in 0..steps {
        let f = i as f32 / steps as f32;
        let band = Rect::from_min_max(
            pos2(rect.left(), rect.bottom() - height * (1.0 - f)),
            pos2(
                rect.right(),
                rect.bottom() - height * (1.0 - (i + 1) as f32 / steps as f32),
            ),
        );
        ui.painter().rect_filled(
            band,
            0,
            color.gamma_multiply((f + 0.5 / steps as f32).min(1.0)),
        );
    }
}
/// Equal-width columns like `Ui::columns`, but with a non-justified layout.
/// egui justifies wrapped labels inside `top_down_justified` layouts by
/// stretching the gaps between glyphs, which detaches Thai vowels and tone
/// marks from their consonants.
pub fn columns<R>(ui: &mut Ui, count: usize, add: impl FnOnce(&mut [Ui]) -> R) -> R {
    let spacing = ui.spacing().item_spacing.x;
    let total_spacing = spacing * (count as f32 - 1.0);
    let width = (ui.available_width() - total_spacing) / count as f32;
    let top_left = ui.cursor().min;
    let bottom = ui.max_rect().bottom();
    let mut cols: Vec<Ui> = (0..count)
        .map(|i| {
            let x = top_left.x + i as f32 * (width + spacing);
            let rect = Rect::from_min_max(pos2(x, top_left.y), pos2(x + width, bottom));
            let mut col = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(Align::Min)),
            );
            col.set_width(width);
            col
        })
        .collect();
    let result = add(&mut cols);
    let height = cols.iter().map(|c| c.min_size().y).fold(0.0, f32::max);
    let size = vec2(ui.available_width(), height);
    ui.advance_cursor_after_rect(Rect::from_min_size(top_left, size));
    result
}
