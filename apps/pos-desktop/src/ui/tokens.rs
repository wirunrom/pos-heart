//! POS Design System — design tokens for egui.
//! GENERATED from tokens/tokens.dtcg.json (v0.1). Do not edit by hand: change the tokens and regenerate.
//! Widgets must use `Palette`, `Metrics`, `space`, `radius` and `type_scale` only — never `primitive` directly.
#![allow(dead_code)]

use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode { Light, Dark }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density { Compact, Touch }

/// Tier 1 — raw ramps. Brand-swappable ramp: SAFFRON (see brand.accent-ramp).
pub mod primitive {
    use eframe::egui::Color32;
    pub const STONE_0: Color32 = Color32::from_rgb(255, 255, 255);
    pub const STONE_25: Color32 = Color32::from_rgb(251, 250, 248);
    pub const STONE_50: Color32 = Color32::from_rgb(245, 244, 240);
    pub const STONE_100: Color32 = Color32::from_rgb(235, 233, 227);
    pub const STONE_200: Color32 = Color32::from_rgb(222, 219, 211);
    pub const STONE_300: Color32 = Color32::from_rgb(201, 197, 187);
    pub const STONE_400: Color32 = Color32::from_rgb(168, 163, 152);
    pub const STONE_500: Color32 = Color32::from_rgb(130, 125, 114);
    pub const STONE_600: Color32 = Color32::from_rgb(98, 94, 85);
    pub const STONE_700: Color32 = Color32::from_rgb(71, 68, 62);
    pub const STONE_800: Color32 = Color32::from_rgb(48, 46, 42);
    pub const STONE_850: Color32 = Color32::from_rgb(39, 37, 34);
    pub const STONE_900: Color32 = Color32::from_rgb(31, 30, 27);
    pub const STONE_925: Color32 = Color32::from_rgb(25, 24, 22);
    pub const STONE_950: Color32 = Color32::from_rgb(18, 17, 16);
    pub const STONE_975: Color32 = Color32::from_rgb(11, 11, 10);
    pub const SAFFRON_100: Color32 = Color32::from_rgb(252, 240, 212);
    pub const SAFFRON_200: Color32 = Color32::from_rgb(249, 223, 159);
    pub const SAFFRON_300: Color32 = Color32::from_rgb(245, 200, 89);
    pub const SAFFRON_400: Color32 = Color32::from_rgb(240, 178, 50);
    pub const SAFFRON_500: Color32 = Color32::from_rgb(220, 152, 16);
    pub const SAFFRON_600: Color32 = Color32::from_rgb(180, 122, 6);
    pub const SAFFRON_700: Color32 = Color32::from_rgb(135, 90, 3);
    pub const SAFFRON_800: Color32 = Color32::from_rgb(90, 60, 4);
    pub const SAFFRON_900: Color32 = Color32::from_rgb(58, 40, 6);
    pub const SAFFRON_950: Color32 = Color32::from_rgb(42, 30, 9);
    pub const GREEN_100: Color32 = Color32::from_rgb(221, 243, 230);
    pub const GREEN_300: Color32 = Color32::from_rgb(127, 208, 164);
    pub const GREEN_500: Color32 = Color32::from_rgb(47, 158, 100);
    pub const GREEN_600: Color32 = Color32::from_rgb(31, 127, 78);
    pub const GREEN_700: Color32 = Color32::from_rgb(23, 99, 61);
    pub const GREEN_900: Color32 = Color32::from_rgb(16, 48, 31);
    pub const ORANGE_100: Color32 = Color32::from_rgb(254, 235, 220);
    pub const ORANGE_300: Color32 = Color32::from_rgb(248, 178, 122);
    pub const ORANGE_500: Color32 = Color32::from_rgb(234, 115, 23);
    pub const ORANGE_600: Color32 = Color32::from_rgb(194, 90, 12);
    pub const ORANGE_700: Color32 = Color32::from_rgb(152, 69, 8);
    pub const ORANGE_900: Color32 = Color32::from_rgb(61, 31, 8);
    pub const RED_100: Color32 = Color32::from_rgb(253, 229, 226);
    pub const RED_300: Color32 = Color32::from_rgb(245, 154, 144);
    pub const RED_500: Color32 = Color32::from_rgb(224, 72, 58);
    pub const RED_600: Color32 = Color32::from_rgb(192, 53, 42);
    pub const RED_700: Color32 = Color32::from_rgb(154, 42, 33);
    pub const RED_900: Color32 = Color32::from_rgb(64, 21, 15);
    pub const BLUE_100: Color32 = Color32::from_rgb(226, 237, 253);
    pub const BLUE_300: Color32 = Color32::from_rgb(147, 185, 243);
    pub const BLUE_500: Color32 = Color32::from_rgb(59, 123, 224);
    pub const BLUE_600: Color32 = Color32::from_rgb(37, 99, 199);
    pub const BLUE_700: Color32 = Color32::from_rgb(29, 79, 159);
    pub const BLUE_900: Color32 = Color32::from_rgb(19, 39, 72);
}

/// Tier 2 — semantic colours. One instance per theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// App background behind surfaces.
    pub bg_canvas: Color32,
    /// Panels, cards, cart, tables. Default text ground.
    pub bg_surface: Color32,
    /// Popovers, menus, dialogs, tiles that sit above a surface.
    pub bg_raised: Color32,
    /// Wells inside a surface: summary blocks, inputs groups, disabled tiles.
    pub bg_sunken: Color32,
    /// Hover fill for rows, tabs, ghost buttons.
    pub bg_hover: Color32,
    /// Pressed fill for neutral controls.
    pub bg_pressed: Color32,
    /// Selected row / current cart line.
    pub bg_selected: Color32,
    /// Active segment, tooltips, count badges.
    pub bg_inverse: Color32,
    /// Primary text and all money values.
    pub fg_default: Color32,
    /// Secondary text, labels, metadata. ≥4.5:1 on surface and sunken.
    pub fg_muted: Color32,
    /// Placeholders and disabled text only (exempt from 4.5:1). Never for information.
    pub fg_subtle: Color32,
    /// Text on bg-inverse.
    pub fg_inverse: Color32,
    /// Accent text: notes, modifiers, active tab label.
    pub fg_accent: Color32,
    /// Text/icons on accent-bg (dark on saffron in both themes).
    pub fg_on_accent: Color32,
    /// Text on danger-solid.
    pub fg_on_danger: Color32,
    /// Dividers between rows inside a surface.
    pub border_subtle: Color32,
    /// Card and tile outlines, section dividers.
    pub border_default: Color32,
    /// Input and control boundaries (≥3:1 on surface).
    pub border_control: Color32,
    /// Selected tile / chosen option outline (2px).
    pub border_selected: Color32,
    /// Keyboard focus ring, 2px + 2px offset. Distinct from accent on purpose.
    pub focus_ring: Color32,
    /// THE commit action: take payment, confirm, primary save. One per view.
    pub accent_bg: Color32,
    /// Hover on accent-bg.
    pub accent_bg_hover: Color32,
    /// Pressed on accent-bg.
    pub accent_bg_pressed: Color32,
    /// Soft accent fill: active nav item, selected chip.
    pub accent_soft: Color32,
    /// Success text/icon: paid, synced, balanced.
    pub success_fg: Color32,
    /// Success badge/banner fill.
    pub success_bg: Color32,
    /// Success dot, chart positive.
    pub success_solid: Color32,
    /// Warning text: low stock, unsaved, over/short within tolerance.
    pub warning_fg: Color32,
    /// Warning badge/banner fill.
    pub warning_bg: Color32,
    /// Warning dot.
    pub warning_solid: Color32,
    /// Danger text: out of stock, failed, refund amounts, validation errors.
    pub danger_fg: Color32,
    /// Danger badge/banner fill.
    pub danger_bg: Color32,
    /// Destructive button fill (with fg-on-danger).
    pub danger_solid: Color32,
    /// Info text: in progress, syncing, sent to kitchen.
    pub info_fg: Color32,
    /// Info badge/banner fill.
    pub info_bg: Color32,
    /// Info dot, chart secondary series.
    pub info_solid: Color32,
    /// Chart series 1 (current period / net sales).
    pub chart_1: Color32,
    /// Chart series 2 (comparison period).
    pub chart_2: Color32,
    /// Chart series 3.
    pub chart_3: Color32,
    /// Chart series 4.
    pub chart_4: Color32,
    /// Chart gridlines.
    pub chart_grid: Color32,
    /// Scrim behind dialogs and sheets (premultiplied alpha).
    pub overlay_scrim: Color32,
}

impl Palette {
    pub const LIGHT: Palette = Palette {
        bg_canvas: primitive::STONE_50,
        bg_surface: primitive::STONE_0,
        bg_raised: primitive::STONE_0,
        bg_sunken: primitive::STONE_100,
        bg_hover: primitive::STONE_100,
        bg_pressed: primitive::STONE_200,
        bg_selected: primitive::SAFFRON_100,
        bg_inverse: primitive::STONE_900,
        fg_default: primitive::STONE_900,
        fg_muted: primitive::STONE_600,
        fg_subtle: primitive::STONE_500,
        fg_inverse: primitive::STONE_0,
        fg_accent: primitive::SAFFRON_700,
        fg_on_accent: primitive::STONE_950,
        fg_on_danger: primitive::STONE_0,
        border_subtle: primitive::STONE_100,
        border_default: primitive::STONE_200,
        border_control: primitive::STONE_500,
        border_selected: primitive::STONE_900,
        focus_ring: primitive::BLUE_600,
        accent_bg: primitive::SAFFRON_400,
        accent_bg_hover: primitive::SAFFRON_500,
        accent_bg_pressed: primitive::SAFFRON_600,
        accent_soft: primitive::SAFFRON_100,
        success_fg: primitive::GREEN_700,
        success_bg: primitive::GREEN_100,
        success_solid: primitive::GREEN_600,
        warning_fg: primitive::ORANGE_700,
        warning_bg: primitive::ORANGE_100,
        warning_solid: primitive::ORANGE_600,
        danger_fg: primitive::RED_700,
        danger_bg: primitive::RED_100,
        danger_solid: primitive::RED_600,
        info_fg: primitive::BLUE_700,
        info_bg: primitive::BLUE_100,
        info_solid: primitive::BLUE_600,
        chart_1: primitive::SAFFRON_600,
        chart_2: primitive::STONE_700,
        chart_3: primitive::BLUE_500,
        chart_4: primitive::GREEN_500,
        chart_grid: primitive::STONE_100,
        overlay_scrim: Color32::from_rgba_premultiplied(9, 8, 8, 122),
    };
    pub const DARK: Palette = Palette {
        bg_canvas: primitive::STONE_950,
        bg_surface: primitive::STONE_925,
        bg_raised: primitive::STONE_900,
        bg_sunken: primitive::STONE_975,
        bg_hover: primitive::STONE_850,
        bg_pressed: primitive::STONE_800,
        bg_selected: primitive::SAFFRON_950,
        bg_inverse: primitive::STONE_50,
        fg_default: primitive::STONE_50,
        fg_muted: primitive::STONE_400,
        fg_subtle: primitive::STONE_500,
        fg_inverse: primitive::STONE_925,
        fg_accent: primitive::SAFFRON_300,
        fg_on_accent: primitive::STONE_950,
        fg_on_danger: primitive::STONE_0,
        border_subtle: primitive::STONE_850,
        border_default: primitive::STONE_800,
        border_control: primitive::STONE_500,
        border_selected: primitive::STONE_50,
        focus_ring: primitive::BLUE_300,
        accent_bg: primitive::SAFFRON_400,
        accent_bg_hover: primitive::SAFFRON_300,
        accent_bg_pressed: primitive::SAFFRON_500,
        accent_soft: primitive::SAFFRON_950,
        success_fg: primitive::GREEN_300,
        success_bg: primitive::GREEN_900,
        success_solid: primitive::GREEN_500,
        warning_fg: primitive::ORANGE_300,
        warning_bg: primitive::ORANGE_900,
        warning_solid: primitive::ORANGE_500,
        danger_fg: primitive::RED_300,
        danger_bg: primitive::RED_900,
        danger_solid: primitive::RED_600,
        info_fg: primitive::BLUE_300,
        info_bg: primitive::BLUE_900,
        info_solid: primitive::BLUE_500,
        chart_1: primitive::SAFFRON_400,
        chart_2: primitive::STONE_300,
        chart_3: primitive::BLUE_300,
        chart_4: primitive::GREEN_300,
        chart_grid: primitive::STONE_850,
        overlay_scrim: Color32::from_rgba_premultiplied(0, 0, 0, 163),
    };
    pub fn for_mode(mode: ThemeMode) -> &'static Palette {
        match mode { ThemeMode::Light => &Self::LIGHT, ThemeMode::Dark => &Self::DARK }
    }
}

/// Tier 3 — component tokens (aliases of semantic tokens).
impl Palette {
    /// Primary button fill.
    pub fn button_primary_bg(&self) -> Color32 { self.accent_bg }
    /// Primary button label.
    pub fn button_primary_fg(&self) -> Color32 { self.fg_on_accent }
    /// Product tile already in cart.
    pub fn tile_selected_border(&self) -> Color32 { self.border_selected }
    /// Current cart line (keyboard target).
    pub fn cartline_selected_bg(&self) -> Color32 { self.bg_selected }
    /// Active item in workspace nav.
    pub fn nav_item_active_bg(&self) -> Color32 { self.accent_soft }
}

/// Spacing scale (px, 4px base).
pub mod space {
    /// No gap.
    pub const S0: f32 = 0.0;
    /// Icon–label gap, badge padding.
    pub const S1: f32 = 4.0;
    /// Gap inside controls, between chips and tiles.
    pub const S2: f32 = 8.0;
    /// Row padding (compact), card inner gap.
    pub const S3: f32 = 12.0;
    /// Panel padding, row padding (touch).
    pub const S4: f32 = 16.0;
    /// Section gap inside a panel.
    pub const S5: f32 = 20.0;
    /// Between groups of fields.
    pub const S6: f32 = 24.0;
    /// Between page sections.
    pub const S8: f32 = 32.0;
    /// Empty-state spacing.
    pub const S10: f32 = 40.0;
    /// Screen margins on large displays.
    pub const S12: f32 = 48.0;
}

/// Corner radii (px). egui 0.31+: `CornerRadius::same(radius::LG)`; older: `Rounding::same(radius::LG as f32)`.
pub mod radius {
    /// Badges, kbd hints, count bubbles.
    pub const XS: u8 = 4;
    /// Inputs and buttons (compact).
    pub const SM: u8 = 6;
    /// Buttons (touch), tiles, menus.
    pub const MD: u8 = 8;
    /// Surfaces: cart panel, cards, popovers.
    pub const LG: u8 = 12;
    /// Dialogs and sheets.
    pub const XL: u8 = 16;
    /// Pills, status dots, avatars.
    pub const FULL: u8 = 255;
}

/// Sizes that change with density.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub control_sm: f32, pub control_md: f32, pub control_lg: f32, pub control_xl: f32,
    pub row_height: f32, pub icon: f32, pub topbar: f32, pub tile_min_width: f32, pub tile_min_height: f32,
    pub font_body: f32, pub font_label: f32, pub font_money: f32, pub show_shortcut_hints: bool,
}

impl Metrics {
    pub const COMPACT: Metrics = Metrics { control_sm: 28.0, control_md: 36.0, control_lg: 44.0, control_xl: 52.0, row_height: 40.0, icon: 20.0, topbar: 52.0, tile_min_width: 136.0, tile_min_height: 84.0, font_body: 14.0, font_label: 13.0, font_money: 14.0, show_shortcut_hints: true };
    pub const TOUCH: Metrics = Metrics { control_sm: 40.0, control_md: 48.0, control_lg: 56.0, control_xl: 64.0, row_height: 56.0, icon: 24.0, topbar: 64.0, tile_min_width: 176.0, tile_min_height: 104.0, font_body: 16.0, font_label: 15.0, font_money: 16.0, show_shortcut_hints: false };
    pub fn for_density(d: Density) -> Metrics { match d { Density::Compact => Self::COMPACT, Density::Touch => Self::TOUCH } }
    /// Minimum hit area in touch density.
    pub const TOUCH_TARGET_MIN: f32 = 44.0;
    pub fn cart_width(&self, screen_width: f32) -> f32 {
        if screen_width < 1100.0 { 360.0 } else if self.row_height > 40.0 { 420.0 } else { 392.0 }
    }
}

/// Type scale. `Family::Ui` = IBM Plex Sans Thai + IBM Plex Sans; `Family::Num` = IBM Plex Mono (all money).
pub mod type_scale {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Family { Ui, Num }
    #[derive(Clone, Copy, Debug)]
    pub struct Style { pub size: f32, pub line_height: f32, pub weight: u16, pub family: Family }
    /// One per screen at most: dashboard/summary headline.
    pub const DISPLAY: Style = Style { size: 30.0, line_height: 40.0, weight: 600, family: Family::Ui };
    /// Page title.
    pub const TITLE_LG: Style = Style { size: 22.0, line_height: 32.0, weight: 600, family: Family::Ui };
    /// Panel / dialog / section title, bill name.
    pub const TITLE: Style = Style { size: 17.0, line_height: 26.0, weight: 600, family: Family::Ui };
    /// Default text (compact). Touch: 16/26.
    pub const BODY: Style = Style { size: 14.0, line_height: 22.0, weight: 400, family: Family::Ui };
    /// Emphasis inside body, labels of key values.
    pub const BODY_STRONG: Style = Style { size: 14.0, line_height: 22.0, weight: 600, family: Family::Ui };
    /// Field labels, buttons (compact), tabs.
    pub const LABEL: Style = Style { size: 13.0, line_height: 20.0, weight: 500, family: Family::Ui };
    /// Metadata, helper text. Never below 12px; Thai marks need the line height.
    pub const CAPTION: Style = Style { size: 12.0, line_height: 18.0, weight: 400, family: Family::Ui };
    /// Amount due, change due. One per view.
    pub const MONEY_XL: Style = Style { size: 34.0, line_height: 40.0, weight: 600, family: Family::Num };
    /// Change, cash received, KPI tiles.
    pub const MONEY_LG: Style = Style { size: 22.0, line_height: 28.0, weight: 600, family: Family::Num };
    /// Line amounts, table cells, tile prices. Always right-aligned in columns.
    pub const MONEY: Style = Style { size: 14.0, line_height: 22.0, weight: 500, family: Family::Num };
    /// Unit prices, secondary amounts.
    pub const MONEY_SM: Style = Style { size: 12.0, line_height: 18.0, weight: 500, family: Family::Num };
}

/// Motion (ms). Multiply by 0.0 when reduced motion is on.
pub mod motion {
    /// Selection, focus, typing feedback.
    pub const INSTANT: f32 = 0.0;
    /// Hover/press, small state changes.
    pub const FAST: f32 = 0.12;
    /// Popover, toast enter, row insert highlight.
    pub const BASE: f32 = 0.2;
    /// Sheet/dialog enter. Exit = 0.7× enter.
    pub const SLOW: f32 = 0.28;
}
