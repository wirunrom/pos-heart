//! Theme = semantic palette + density metrics + shape + type scale
//! (design/docs/07 §1–2, v0.2: one theme, graphite chrome + paper panes).
//! Widgets read `Theme::of(ui)`; colours and sizes never appear as literals in screens.
pub use super::tokens::{AccentPreset, Shape};
use super::tokens::{Density, Metrics, Palette, radius, space, type_scale};
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText,
    Shadow, Stroke, TextStyle,
};
use std::sync::{Arc, LazyLock};

/// Device look: brand accent and corner shape (the palette itself is fixed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub accent: AccentPreset,
    pub shape: Shape,
}
impl Default for Look {
    fn default() -> Self {
        Self {
            accent: AccentPreset::Saffron,
            shape: Shape::Soft,
        }
    }
}
pub const ACCENTS: [AccentPreset; 5] = [
    AccentPreset::Saffron,
    AccentPreset::Jade,
    AccentPreset::Ocean,
    AccentPreset::Plum,
    AccentPreset::Ink,
];
pub fn accent_from_saved(value: &str) -> AccentPreset {
    ACCENTS
        .into_iter()
        .find(|a| accent_saved(*a) == value.trim())
        .unwrap_or(AccentPreset::Saffron)
}
pub fn accent_saved(accent: AccentPreset) -> &'static str {
    match accent {
        AccentPreset::Saffron => "saffron",
        AccentPreset::Jade => "jade",
        AccentPreset::Ocean => "ocean",
        AccentPreset::Plum => "plum",
        AccentPreset::Ink => "ink",
    }
}
/// The preset's commit-button colour, for swatches.
pub fn accent_color(accent: AccentPreset) -> Color32 {
    palette(accent).accent_bg
}
/// (Thai, English) names from the design system.
pub fn accent_name(accent: AccentPreset) -> (&'static str, &'static str) {
    match accent {
        AccentPreset::Saffron => ("หญ้าฝรั่น", "Saffron"),
        AccentPreset::Jade => ("หยก", "Jade"),
        AccentPreset::Ocean => ("น้ำทะเล", "Ocean"),
        AccentPreset::Plum => ("พลัม", "Plum"),
        AccentPreset::Ink => ("หมึก", "Ink"),
    }
}
pub fn shape_from_saved(value: &str) -> Shape {
    if value.trim() == "square" {
        Shape::Square
    } else {
        Shape::Soft
    }
}
pub fn shape_saved(shape: Shape) -> &'static str {
    match shape {
        Shape::Soft => "soft",
        Shape::Square => "square",
    }
}
pub fn density_from_saved(value: &str) -> Density {
    if value.trim() == "touch" {
        Density::Touch
    } else {
        Density::Compact
    }
}
pub fn density_saved(density: Density) -> &'static str {
    match density {
        Density::Compact => "compact",
        Density::Touch => "touch",
    }
}

const UI: &str = "ui";
const UI_STRONG: &str = "ui_strong";
const NUM: &str = "num";
const NUM_STRONG: &str = "num_strong";
fn prefs_id() -> egui::Id {
    egui::Id::new("pos-display")
}
/// One palette per accent preset, built once.
static PALETTES: LazyLock<[Palette; 5]> =
    LazyLock::new(|| ACCENTS.map(|a| Palette::DEFAULT.with_accent(a)));
fn palette(accent: AccentPreset) -> &'static Palette {
    let i = ACCENTS.iter().position(|a| *a == accent).unwrap_or(0);
    &PALETTES[i]
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub density: Density,
    pub shape: Shape,
    pub p: &'static Palette,
    pub m: Metrics,
}
impl Theme {
    pub fn new(look: Look, density: Density) -> Self {
        Self {
            density,
            shape: look.shape,
            p: palette(look.accent),
            m: Metrics::for_density(density),
        }
    }
    pub fn current(ctx: &egui::Context) -> Self {
        let (look, density) = ctx
            .data(|d| d.get_temp::<(Look, Density)>(prefs_id()))
            .unwrap_or((Look::default(), Density::Compact));
        Self::new(look, density)
    }
    pub fn of(ui: &egui::Ui) -> Self {
        Self::current(ui.ctx())
    }
    pub fn touch(&self) -> bool {
        self.density == Density::Touch
    }
    pub fn soft(&self) -> bool {
        self.shape == Shape::Soft
    }
    /// A corner radius token for the current shape: square keeps pills
    /// round and caps everything else at 3 px.
    pub fn radius(&self, r: u8) -> u8 {
        if self.soft() || r == radius::FULL {
            r
        } else {
            r.min(3)
        }
    }
    /// Panes (cart, lists, tables): rounded and floating when soft, edge
    /// to edge when square.
    pub fn pane_radius(&self) -> u8 {
        if self.soft() { radius::LG } else { 0 }
    }
    /// Gap between panes (soft: floating sheets; square: 1 px splitters).
    pub fn pane_gap(&self) -> f32 {
        if self.soft() { space::PANE_GAP } else { 1.0 }
    }
    /// Pane header strip height (Navigation: 28 px, touch 38).
    pub fn pane_header_h(&self) -> f32 {
        if self.touch() { 38.0 } else { 28.0 }
    }
    /// Data colour for the i-th category (tile stripe, dot); never a status.
    pub fn cat_color(&self, index: usize) -> Color32 {
        [
            self.p.cat_food,
            self.p.cat_drink,
            self.p.cat_goods,
            self.p.cat_snack,
        ][index % 4]
    }
    /// Cart width for the window width (design/docs/07 §3).
    pub fn cart_width(&self, screen_width: f32) -> f32 {
        if screen_width < 1100.0 {
            360.0
        } else {
            self.m.cart_width
        }
    }
    /// Size adjusted for density: body, label, money and titles grow in touch mode.
    fn size(&self, s: type_scale::Style) -> f32 {
        if !self.touch() {
            return s.size;
        }
        match (s.size as u32, s.family) {
            (16, type_scale::Family::Ui) => 17.0,
            (13, type_scale::Family::Ui) => self.m.font_body,
            (12, type_scale::Family::Ui) => self.m.font_label,
            (13, type_scale::Family::Num) => self.m.font_money,
            (30, type_scale::Family::Num) => self.m.font_money_xl,
            _ => s.size,
        }
    }
    pub fn font(&self, s: type_scale::Style) -> FontId {
        let strong = s.weight >= 550;
        let family = match (s.family, strong) {
            (type_scale::Family::Ui, false) => UI,
            (type_scale::Family::Ui, true) => UI_STRONG,
            (type_scale::Family::Num, false) => NUM,
            (type_scale::Family::Num, true) => NUM_STRONG,
        };
        FontId::new(self.size(s), FontFamily::Name(family.into()))
    }
    pub fn strong_font(&self, s: type_scale::Style) -> FontId {
        self.font(type_scale::Style { weight: 600, ..s })
    }
    pub fn text(&self, text: impl Into<String>, s: type_scale::Style) -> RichText {
        RichText::new(text)
            .font(self.font(s))
            .color(self.p.fg_default)
    }
    pub fn muted(&self, text: impl Into<String>, s: type_scale::Style) -> RichText {
        RichText::new(text)
            .font(self.font(s))
            .color(self.p.fg_muted)
    }
    pub fn control_radius(&self) -> CornerRadius {
        CornerRadius::same(self.radius(radius::MD))
    }
    pub fn shadow(&self, level: u8) -> Shadow {
        let (y, blur, alpha) = match level {
            1 => (1, 6, 14),
            2 => (4, 16, 40),
            _ => (16, 48, 70),
        };
        Shadow {
            offset: [0, y],
            blur,
            spread: 0,
            color: Color32::from_black_alpha(alpha),
        }
    }
    /// A pane on the graphite canvas: cart, lists, tables, settings.
    pub fn surface(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.p.bg_surface)
            .corner_radius(self.pane_radius())
            .inner_margin(Margin::same(space::S4 as i8))
    }
    /// Summary blocks and wells inside a surface.
    pub fn sunken(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.p.bg_sunken)
            .corner_radius(self.radius(radius::MD))
            .inner_margin(Margin::same(space::S3 as i8))
    }
    /// Menus, popovers and dialogs.
    pub fn raised(&self, level: u8) -> egui::Frame {
        egui::Frame::new()
            .fill(self.p.bg_raised)
            .corner_radius(self.radius(if level >= 3 { radius::XL } else { radius::MD }))
            .shadow(self.shadow(level))
            .stroke(Stroke::new(1.0, self.p.border_default))
    }
    /// Text on the graphite chrome/canvas (title bar, status bar, page
    /// headings that sit outside a pane).
    pub fn chrome(&self, text: impl Into<String>, s: type_scale::Style) -> RichText {
        RichText::new(text)
            .font(self.font(s))
            .color(self.p.fg_chrome)
    }
    pub fn chrome_muted(&self, text: impl Into<String>, s: type_scale::Style) -> RichText {
        RichText::new(text)
            .font(self.font(s))
            .color(self.p.fg_chrome_muted)
    }
}

/// Bundled font files, shared by the UI and receipt printing.
pub mod font_bytes {
    pub const PLEX_SANS: &[u8] = include_bytes!("../../../../assets/fonts/IBMPlexSans-Regular.ttf");
    pub const PLEX_SANS_SB: &[u8] =
        include_bytes!("../../../../assets/fonts/IBMPlexSans-SemiBold.ttf");
    pub const PLEX_THAI: &[u8] =
        include_bytes!("../../../../assets/fonts/IBMPlexSansThai-Regular.ttf");
    pub const PLEX_THAI_SB: &[u8] =
        include_bytes!("../../../../assets/fonts/IBMPlexSansThai-SemiBold.ttf");
    pub const PLEX_MONO: &[u8] = include_bytes!("../../../../assets/fonts/IBMPlexMono-Medium.ttf");
    pub const PLEX_MONO_SB: &[u8] =
        include_bytes!("../../../../assets/fonts/IBMPlexMono-SemiBold.ttf");
    pub const NOTO_THAI: &[u8] =
        include_bytes!("../../../../assets/fonts/NotoSansThai-Regular.ttf");
}
/// Fonts are bundled so the app works offline (OFL, assets/fonts/IBM-Plex-OFL.txt).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    add("plex_sans", font_bytes::PLEX_SANS);
    add("plex_sans_sb", font_bytes::PLEX_SANS_SB);
    add("plex_thai", font_bytes::PLEX_THAI);
    add("plex_thai_sb", font_bytes::PLEX_THAI_SB);
    add("plex_mono", font_bytes::PLEX_MONO);
    add("plex_mono_sb", font_bytes::PLEX_MONO_SB);
    // Rare Thai marks missing from Plex (e.g. ฤๅ) fall back to Noto Sans Thai.
    add("noto_thai", font_bytes::NOTO_THAI);
    let chain = |names: &[&str]| -> Vec<String> {
        names
            .iter()
            .map(|n| (*n).to_owned())
            .chain(["Ubuntu-Light".to_owned(), "emoji-icon-font".to_owned()])
            .collect()
    };
    let ui = chain(&["plex_sans", "plex_thai", "noto_thai"]);
    fonts
        .families
        .insert(FontFamily::Name(UI.into()), ui.clone());
    fonts.families.insert(
        FontFamily::Name(UI_STRONG.into()),
        chain(&["plex_sans_sb", "plex_thai_sb", "noto_thai"]),
    );
    fonts.families.insert(
        FontFamily::Name(NUM.into()),
        chain(&["plex_mono", "plex_thai", "noto_thai"]),
    );
    fonts.families.insert(
        FontFamily::Name(NUM_STRONG.into()),
        chain(&["plex_mono_sb", "plex_thai_sb", "noto_thai"]),
    );
    fonts.families.insert(FontFamily::Proportional, ui);
    fonts.families.insert(
        FontFamily::Monospace,
        chain(&["plex_mono", "plex_thai", "noto_thai"]),
    );
    fonts
        .font_data
        .retain(|name, _| name != "Hack" && name != "NotoEmoji-Regular");
    ctx.set_fonts(fonts);
}

/// Applies the look and density; the app always uses egui's light theme.
pub fn apply(ctx: &egui::Context, look: Look, density: Density) {
    ctx.data_mut(|d| d.insert_temp(prefs_id(), (look, density)));
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_style_of(egui::Theme::Light, style(Theme::new(look, density)));
}

fn style(t: Theme) -> egui::Style {
    let p = t.p;
    let mut style = egui::Style {
        visuals: egui::Visuals::light(),
        ..Default::default()
    };
    let v = &mut style.visuals;
    v.panel_fill = p.bg_canvas;
    v.window_fill = p.bg_raised;
    v.window_stroke = Stroke::new(1.0, p.border_default);
    v.window_corner_radius = CornerRadius::same(t.radius(radius::XL));
    v.menu_corner_radius = CornerRadius::same(t.radius(radius::MD));
    v.window_shadow = t.shadow(3);
    v.popup_shadow = t.shadow(2);
    v.override_text_color = Some(p.fg_default);
    v.weak_text_color = Some(p.fg_muted);
    v.hyperlink_color = p.fg_accent;
    v.error_fg_color = p.danger_fg;
    v.warn_fg_color = p.warning_fg;
    v.selection.bg_fill = p.bg_selected;
    v.selection.stroke = Stroke::new(2.0, p.focus_ring);
    v.text_edit_bg_color = Some(p.bg_raised);
    v.extreme_bg_color = p.bg_raised;
    v.faint_bg_color = p.bg_sunken;
    v.code_bg_color = p.bg_sunken;
    v.text_cursor.stroke = Stroke::new(2.0, p.fg_default);
    v.striped = false;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = t.control_radius();
        w.fg_stroke = Stroke::new(1.0, p.fg_default);
        w.bg_stroke = Stroke::new(1.0, p.border_control);
        w.bg_fill = p.bg_raised;
        w.weak_bg_fill = p.bg_raised;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
    v.widgets.noninteractive.bg_fill = p.bg_surface;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.fg_default);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.fg_default);
    v.widgets.inactive.bg_fill = p.bg_sunken;
    v.widgets.hovered.weak_bg_fill = p.bg_sunken;
    v.widgets.hovered.bg_fill = p.bg_sunken;
    v.widgets.active.weak_bg_fill = p.bg_pressed;
    v.widgets.active.bg_fill = p.bg_pressed;
    v.widgets.open.weak_bg_fill = p.bg_hover;

    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(space::S2, space::S2);
    s.button_padding = egui::vec2(if t.touch() { space::S4 } else { space::S3 }, 0.0);
    s.interact_size.y = t.m.control_md;
    s.icon_width = if t.touch() { 22.0 } else { 18.0 };
    s.icon_width_inner = if t.touch() { 12.0 } else { 10.0 };
    s.text_edit_width = 200.0;
    s.window_margin = Margin::same(space::S6 as i8);
    s.menu_margin = Margin::same(space::S1 as i8);
    s.combo_height = 320.0;
    s.scroll = egui::style::ScrollStyle::floating();

    style.text_styles = [
        (TextStyle::Body, t.font(type_scale::BODY)),
        (TextStyle::Button, t.font(type_scale::BODY_STRONG)),
        (TextStyle::Small, t.font(type_scale::CAPTION)),
        (TextStyle::Heading, t.font(type_scale::TITLE)),
        (TextStyle::Monospace, t.font(type_scale::MONEY)),
    ]
    .into();
    style.interaction.selectable_labels = false;
    style.animation_time = super::tokens::motion::FAST;
    style
}
