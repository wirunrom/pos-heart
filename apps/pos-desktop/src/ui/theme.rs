//! Theme = semantic palette + density metrics + type scale (design/docs/07 §1–2).
//! Widgets read `Theme::of(ui)`; colours and sizes never appear as literals in screens.
use super::tokens::{Density, Metrics, Palette, ThemeMode, radius, space, type_scale};
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText,
    Shadow, Stroke, TextStyle,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
    #[default]
    System,
}
impl Appearance {
    pub fn from_saved(value: &str) -> Self {
        match value.trim() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
    pub fn saved(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }
    fn preference(self) -> egui::ThemePreference {
        match self {
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
            Self::System => egui::ThemePreference::System,
        }
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
fn density_id() -> egui::Id {
    egui::Id::new("pos-density")
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,
    pub density: Density,
    pub p: &'static Palette,
    pub m: Metrics,
}
impl Theme {
    pub fn new(mode: ThemeMode, density: Density) -> Self {
        Self {
            mode,
            density,
            p: Palette::for_mode(mode),
            m: Metrics::for_density(density),
        }
    }
    pub fn current(ctx: &egui::Context) -> Self {
        let mode = match ctx.theme() {
            egui::Theme::Dark => ThemeMode::Dark,
            egui::Theme::Light => ThemeMode::Light,
        };
        let density = ctx
            .data(|d| d.get_temp::<Density>(density_id()))
            .unwrap_or(Density::Compact);
        Self::new(mode, density)
    }
    pub fn of(ui: &egui::Ui) -> Self {
        Self::current(ui.ctx())
    }
    pub fn touch(&self) -> bool {
        self.density == Density::Touch
    }
    /// Size adjusted for density: body, label and money grow in touch mode.
    fn size(&self, s: type_scale::Style) -> f32 {
        let touch = self.touch();
        match (s.size as u32, s.family) {
            (14, type_scale::Family::Ui) if touch => self.m.font_body,
            (13, type_scale::Family::Ui) if touch => self.m.font_label,
            (14, type_scale::Family::Num) if touch => self.m.font_money,
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
        CornerRadius::same(if self.touch() { radius::MD } else { radius::SM })
    }
    pub fn shadow(&self, level: u8) -> Shadow {
        let dark = self.mode == ThemeMode::Dark;
        let (y, blur, alpha) = match level {
            1 => (1, 6, if dark { 120 } else { 14 }),
            2 => (4, 16, if dark { 150 } else { 30 }),
            _ => (16, 48, if dark { 180 } else { 52 }),
        };
        Shadow {
            offset: [0, y],
            blur,
            spread: 0,
            color: Color32::from_black_alpha(alpha),
        }
    }
    /// Dark surfaces use a faint highlight edge instead of relying on shadow.
    fn edge(&self) -> Stroke {
        if self.mode == ThemeMode::Dark {
            Stroke::new(1.0, self.p.border_subtle)
        } else {
            Stroke::NONE
        }
    }
    /// Cart, tables, cards on the canvas.
    pub fn surface(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.p.bg_surface)
            .corner_radius(radius::LG)
            .shadow(self.shadow(1))
            .stroke(self.edge())
            .inner_margin(Margin::same(space::S4 as i8))
    }
    /// Summary blocks and wells inside a surface.
    pub fn sunken(&self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.p.bg_sunken)
            .corner_radius(radius::MD)
            .inner_margin(Margin::same(space::S3 as i8))
    }
    /// Menus, popovers and dialogs.
    pub fn raised(&self, level: u8) -> egui::Frame {
        egui::Frame::new()
            .fill(self.p.bg_raised)
            .corner_radius(if level >= 3 { radius::XL } else { radius::MD })
            .shadow(self.shadow(level))
            .stroke(Stroke::new(1.0, self.p.border_default))
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

/// Applies appearance and density to both egui themes; System follows the OS.
pub fn apply(ctx: &egui::Context, appearance: Appearance, density: Density) {
    ctx.data_mut(|d| d.insert_temp(density_id(), density));
    for (egui_theme, mode) in [
        (egui::Theme::Light, ThemeMode::Light),
        (egui::Theme::Dark, ThemeMode::Dark),
    ] {
        ctx.set_style_of(egui_theme, style(Theme::new(mode, density)));
    }
    ctx.set_theme(appearance.preference());
}

fn style(t: Theme) -> egui::Style {
    let p = t.p;
    let mut style = egui::Style {
        visuals: match t.mode {
            ThemeMode::Light => egui::Visuals::light(),
            ThemeMode::Dark => egui::Visuals::dark(),
        },
        ..Default::default()
    };
    let v = &mut style.visuals;
    v.panel_fill = p.bg_canvas;
    v.window_fill = p.bg_raised;
    v.window_stroke = Stroke::new(1.0, p.border_default);
    v.window_corner_radius = CornerRadius::same(radius::XL);
    v.menu_corner_radius = CornerRadius::same(radius::MD);
    v.window_shadow = t.shadow(3);
    v.popup_shadow = t.shadow(2);
    v.override_text_color = Some(p.fg_default);
    v.weak_text_color = Some(p.fg_muted);
    v.hyperlink_color = p.fg_accent;
    v.error_fg_color = p.danger_fg;
    v.warn_fg_color = p.warning_fg;
    v.selection.bg_fill = p.bg_selected;
    v.selection.stroke = Stroke::new(2.0, p.focus_ring);
    v.text_edit_bg_color = Some(p.bg_surface);
    v.extreme_bg_color = p.bg_surface;
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
        w.bg_fill = p.bg_surface;
        w.weak_bg_fill = p.bg_surface;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
    v.widgets.noninteractive.bg_fill = p.bg_surface;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.fg_default);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.fg_default);
    v.widgets.inactive.bg_fill = p.bg_sunken;
    v.widgets.hovered.weak_bg_fill = p.bg_hover;
    v.widgets.hovered.bg_fill = p.bg_hover;
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
        (TextStyle::Heading, t.font(type_scale::TITLE_LG)),
        (TextStyle::Monospace, t.font(type_scale::MONEY)),
    ]
    .into();
    style.interaction.selectable_labels = false;
    style.animation_time = super::tokens::motion::FAST;
    style
}
