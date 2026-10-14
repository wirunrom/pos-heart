//! POS Design System v0.2 — design tokens for egui (single theme: graphite chrome + paper panes).
//! GENERATED from tokens/tokens.dtcg.json. Do not edit by hand.
//! Widgets use `Palette`, `Metrics`, `space`, `radius`, `type_scale`, `motion` — never `primitive`.
#![allow(dead_code)]

use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density { Compact, Touch }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape { Soft, Square }

pub mod primitive {
    use eframe::egui::Color32;
    pub const GRAPHITE_950: Color32 = Color32::from_rgb(24, 25, 27);
    pub const GRAPHITE_900: Color32 = Color32::from_rgb(31, 32, 35);
    pub const GRAPHITE_850: Color32 = Color32::from_rgb(42, 43, 47);
    pub const GRAPHITE_800: Color32 = Color32::from_rgb(53, 55, 59);
    pub const GRAPHITE_700: Color32 = Color32::from_rgb(70, 72, 77);
    pub const GRAPHITE_400: Color32 = Color32::from_rgb(166, 167, 162);
    pub const GRAPHITE_50: Color32 = Color32::from_rgb(236, 235, 231);
    pub const PAPER_0: Color32 = Color32::from_rgb(255, 255, 255);
    pub const PAPER_50: Color32 = Color32::from_rgb(242, 242, 239);
    pub const PAPER_100: Color32 = Color32::from_rgb(233, 233, 229);
    pub const PAPER_150: Color32 = Color32::from_rgb(224, 224, 219);
    pub const PAPER_200: Color32 = Color32::from_rgb(205, 205, 199);
    pub const PAPER_400: Color32 = Color32::from_rgb(134, 135, 130);
    pub const PAPER_600: Color32 = Color32::from_rgb(92, 94, 90);
    pub const PAPER_900: Color32 = Color32::from_rgb(28, 29, 31);
    pub const SLATE_50: Color32 = Color32::from_rgb(238, 241, 245);
    pub const SLATE_100: Color32 = Color32::from_rgb(220, 227, 237);
    pub const SLATE_150: Color32 = Color32::from_rgb(226, 230, 236);
    pub const SLATE_500: Color32 = Color32::from_rgb(79, 117, 170);
    pub const SAFFRON_100: Color32 = Color32::from_rgb(251, 231, 181);
    pub const SAFFRON_400: Color32 = Color32::from_rgb(240, 178, 50);
    pub const SAFFRON_500: Color32 = Color32::from_rgb(220, 152, 16);
    pub const SAFFRON_600: Color32 = Color32::from_rgb(196, 139, 16);
    pub const SAFFRON_700: Color32 = Color32::from_rgb(176, 124, 12);
    pub const SAFFRON_800: Color32 = Color32::from_rgb(126, 83, 3);
    pub const SAFFRON_INK: Color32 = Color32::from_rgb(20, 19, 16);
    pub const GREEN_100: Color32 = Color32::from_rgb(220, 239, 228);
    pub const GREEN_600: Color32 = Color32::from_rgb(46, 139, 87);
    pub const GREEN_700: Color32 = Color32::from_rgb(26, 110, 69);
    pub const ORANGE_100: Color32 = Color32::from_rgb(248, 231, 210);
    pub const ORANGE_600: Color32 = Color32::from_rgb(199, 106, 10);
    pub const ORANGE_700: Color32 = Color32::from_rgb(154, 83, 6);
    pub const RED_100: Color32 = Color32::from_rgb(247, 223, 220);
    pub const RED_600: Color32 = Color32::from_rgb(179, 51, 43);
    pub const RED_700: Color32 = Color32::from_rgb(168, 48, 42);
    pub const BLUE_100: Color32 = Color32::from_rgb(221, 232, 246);
    pub const BLUE_600: Color32 = Color32::from_rgb(47, 111, 208);
    pub const BLUE_700: Color32 = Color32::from_rgb(40, 92, 158);
    pub const BLUE_500: Color32 = Color32::from_rgb(61, 111, 181);
    pub const CAT_FOOD: Color32 = Color32::from_rgb(201, 120, 26);
    pub const CAT_DRINK: Color32 = Color32::from_rgb(63, 120, 181);
    pub const CAT_GOODS: Color32 = Color32::from_rgb(95, 143, 90);
    pub const CAT_SNACK: Color32 = Color32::from_rgb(160, 86, 154);
    pub const CHART_GREY: Color32 = Color32::from_rgb(85, 87, 92);
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Title bar (window controls + workspace tabs) and status bar.
    pub bg_chrome: Color32,
    /// Toolbar and the canvas the panes float on.
    pub bg_chrome_raised: Color32,
    /// Hover/active fill for chrome buttons and the selected workspace tab.
    pub bg_chrome_hover: Color32,
    /// Separators inside chrome (toolbar groups, status items).
    pub border_chrome: Color32,
    /// Text and icons on chrome.
    pub fg_chrome: Color32,
    /// Secondary text on chrome, shortcut hints. ≥5.8:1 on chrome.
    pub fg_chrome_muted: Color32,
    /// Behind panes (same as toolbar so panes read as floating sheets).
    pub bg_canvas: Color32,
    /// Pane background (cart, lists, settings).
    pub bg_surface: Color32,
    /// Wells, tiles, table body, inputs, cards inside a pane.
    pub bg_raised: Color32,
    /// Secondary fills: disabled tiles, segmented tracks, summary blocks.
    pub bg_sunken: Color32,
    /// Hover on rows and tiles (pointer only).
    pub bg_hover: Color32,
    /// Pressed rows.
    pub bg_pressed: Color32,
    /// Selected row/line/category (2px inset bar in square shape).
    pub bg_selected: Color32,
    /// Count badges, active chips, toasts.
    pub bg_inverse: Color32,
    /// Primary text and all money values.
    pub fg_default: Color32,
    /// Labels, metadata. ≥4.5:1 on surface, raised, sunken, selected.
    pub fg_muted: Color32,
    /// Placeholders, disabled text, decorative numbers only.
    pub fg_subtle: Color32,
    /// Text on bg-inverse.
    pub fg_inverse: Color32,
    /// Modifiers and notes on cart lines.
    pub fg_accent: Color32,
    /// Text on accent-bg.
    pub fg_on_accent: Color32,
    /// Text on danger-solid.
    pub fg_on_danger: Color32,
    /// Row dividers inside a pane.
    pub border_subtle: Color32,
    /// Tile, card, input-group outlines.
    pub border_default: Color32,
    /// Input and icon-button boundaries (≥3:1 on raised).
    pub border_control: Color32,
    /// Tile already in the bill; selected-row bar.
    pub border_selected: Color32,
    /// Keyboard focus ring 2px.
    pub focus_ring: Color32,
    /// THE commit action — one per view: ชำระเงิน, ยืนยันรับเงิน, บันทึก. Also the active-tab underline.
    pub accent_bg: Color32,
    /// Hover on accent-bg.
    pub accent_bg_hover: Color32,
    /// Pressed on accent-bg.
    pub accent_bg_pressed: Color32,
    /// “Just added” flash on a cart line.
    pub accent_soft: Color32,
    /// Paid, balanced, ready.
    pub success_fg: Color32,
    /// Success badge/banner fill.
    pub success_bg: Color32,
    /// Success dot, switch on.
    pub success_solid: Color32,
    /// Low stock, unsaved, needs attention.
    pub warning_fg: Color32,
    /// Warning badge/banner fill.
    pub warning_bg: Color32,
    /// Warning dot/stripe.
    pub warning_solid: Color32,
    /// Out of stock, failed, refunds, validation errors.
    pub danger_fg: Color32,
    /// Danger badge/banner fill.
    pub danger_bg: Color32,
    /// Destructive button fill.
    pub danger_solid: Color32,
    /// In progress, sent to kitchen, links.
    pub info_fg: Color32,
    /// Info badge fill.
    pub info_bg: Color32,
    /// Info dot/stripe, seated tables.
    pub info_solid: Color32,
    /// Category color: food (tile stripe, dot). Data color, never a status.
    pub cat_food: Color32,
    /// Category color: drinks.
    pub cat_drink: Color32,
    /// Category color: household goods.
    pub cat_goods: Color32,
    /// Category color: snacks.
    pub cat_snack: Color32,
    /// Chart series 1 (current period).
    pub chart_1: Color32,
    /// Chart series 2 (comparison period).
    pub chart_2: Color32,
    /// Chart series 3.
    pub chart_3: Color32,
    /// Chart series 4.
    pub chart_4: Color32,
    /// Gridlines.
    pub chart_grid: Color32,
    /// Scrim behind dialogs (premultiplied).
    pub overlay_scrim: Color32,
}

impl Palette {
    pub const DEFAULT: Palette = Palette {
        bg_chrome: primitive::GRAPHITE_900,
        bg_chrome_raised: primitive::GRAPHITE_850,
        bg_chrome_hover: primitive::GRAPHITE_800,
        border_chrome: primitive::GRAPHITE_700,
        fg_chrome: primitive::GRAPHITE_50,
        fg_chrome_muted: primitive::GRAPHITE_400,
        bg_canvas: primitive::GRAPHITE_850,
        bg_surface: primitive::PAPER_50,
        bg_raised: primitive::PAPER_0,
        bg_sunken: primitive::PAPER_100,
        bg_hover: primitive::SLATE_50,
        bg_pressed: primitive::SLATE_150,
        bg_selected: primitive::SLATE_100,
        bg_inverse: primitive::GRAPHITE_850,
        fg_default: primitive::PAPER_900,
        fg_muted: primitive::PAPER_600,
        fg_subtle: primitive::PAPER_400,
        fg_inverse: primitive::GRAPHITE_50,
        fg_accent: primitive::SAFFRON_800,
        fg_on_accent: primitive::SAFFRON_INK,
        fg_on_danger: primitive::PAPER_0,
        border_subtle: primitive::PAPER_150,
        border_default: primitive::PAPER_200,
        border_control: primitive::PAPER_400,
        border_selected: primitive::SLATE_500,
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
        info_solid: primitive::BLUE_500,
        cat_food: primitive::CAT_FOOD,
        cat_drink: primitive::CAT_DRINK,
        cat_goods: primitive::CAT_GOODS,
        cat_snack: primitive::CAT_SNACK,
        chart_1: primitive::SAFFRON_700,
        chart_2: primitive::CHART_GREY,
        chart_3: primitive::SLATE_500,
        chart_4: primitive::GREEN_600,
        chart_grid: primitive::PAPER_150,
        overlay_scrim: Color32::from_rgba_premultiplied(4, 4, 5, 115),
    };
    /// Primary button fill.
    pub fn button_primary_bg(&self) -> Color32 { self.accent_bg }
    /// Primary button label.
    pub fn button_primary_fg(&self) -> Color32 { self.fg_on_accent }
    /// Tile already in the bill.
    pub fn tile_selected_border(&self) -> Color32 { self.border_selected }
    /// Current cart line.
    pub fn cartline_selected_bg(&self) -> Color32 { self.bg_selected }
    /// Selected workspace tab.
    pub fn nav_item_active_bg(&self) -> Color32 { self.bg_chrome_hover }
    /// Apply a brand accent preset (only accent-* , fg-on-accent, accent-soft, chart-1 change).
    pub fn with_accent(mut self, a: AccentPreset) -> Self { let (bg, hover, pressed, on, soft, chart) = a.colors(); self.accent_bg = bg; self.accent_bg_hover = hover; self.accent_bg_pressed = pressed; self.fg_on_accent = on; self.accent_soft = soft; self.chart_1 = chart; self }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccentPreset { Saffron, Jade, Ocean, Plum, Ink }
impl AccentPreset {
    /// (bg, hover, pressed, on_accent, soft, chart_1)
    pub const fn colors(self) -> (Color32, Color32, Color32, Color32, Color32, Color32) {
        match self {
            AccentPreset::Saffron => (Color32::from_rgb(240, 178, 50), Color32::from_rgb(220, 152, 16), Color32::from_rgb(196, 139, 16), Color32::from_rgb(20, 19, 16), Color32::from_rgb(251, 231, 181), Color32::from_rgb(176, 124, 12)),
            AccentPreset::Jade => (Color32::from_rgb(15, 122, 95), Color32::from_rgb(12, 101, 80), Color32::from_rgb(10, 84, 67), Color32::from_rgb(255, 255, 255), Color32::from_rgb(207, 234, 223), Color32::from_rgb(15, 122, 95)),
            AccentPreset::Ocean => (Color32::from_rgb(29, 106, 140), Color32::from_rgb(24, 90, 119), Color32::from_rgb(19, 74, 98), Color32::from_rgb(255, 255, 255), Color32::from_rgb(207, 227, 238), Color32::from_rgb(29, 106, 140)),
            AccentPreset::Plum => (Color32::from_rgb(123, 60, 143), Color32::from_rgb(104, 50, 122), Color32::from_rgb(86, 41, 102), Color32::from_rgb(255, 255, 255), Color32::from_rgb(234, 215, 241), Color32::from_rgb(123, 60, 143)),
            AccentPreset::Ink => (Color32::from_rgb(44, 45, 49), Color32::from_rgb(31, 32, 35), Color32::from_rgb(17, 18, 20), Color32::from_rgb(255, 255, 255), Color32::from_rgb(226, 226, 222), Color32::from_rgb(69, 71, 76)),
        }
    }
}

pub mod space {
    /// None.
    pub const S0: f32 = 0.0;
    /// Icon–label gap.
    pub const S1: f32 = 4.0;
    /// Control gaps, tile gaps.
    pub const S2: f32 = 8.0;
    /// Row padding (compact), pane padding.
    pub const S3: f32 = 12.0;
    /// Row padding (touch), section padding.
    pub const S4: f32 = 16.0;
    /// Settings page padding.
    pub const S5: f32 = 20.0;
    /// Between field groups.
    pub const S6: f32 = 24.0;
    /// Between page sections.
    pub const S8: f32 = 32.0;
    /// Gap between floating panes and around them.
    pub const PANE_GAP: f32 = 6.0;
}

pub mod radius {
    /// Kbd hints.
    pub const XS: u8 = 4;
    /// Badges, count bubbles, small chips inside rows.
    pub const SM: u8 = 6;
    /// Buttons, inputs, rows (selection), tabs, toolbar buttons.
    pub const MD: u8 = 8;
    /// Panes, tiles, cards, quick-cash keys.
    pub const LG: u8 = 10;
    /// Dialogs, command palette, PIN pad.
    pub const XL: u8 = 14;
    /// App window corners (when the OS allows).
    pub const WINDOW: u8 = 12;
    /// Pills, chips, dots, avatars.
    pub const FULL: u8 = 255;
}

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub titlebar_h: f32, pub toolbar_h: f32, pub statusbar_h: f32,
    pub control_sm: f32, pub control_md: f32, pub control_lg: f32, pub row_height: f32, pub icon: f32,
    pub cart_width: f32, pub category_width: f32, pub details_width: f32, pub tile_min_width: f32, pub tile_min_height: f32,
    pub font_body: f32, pub font_label: f32, pub font_money: f32, pub font_money_xl: f32,
    pub show_shortcut_hints: bool, pub categories_as_chips: bool,
}
impl Metrics {
    pub const COMPACT: Metrics = Metrics { titlebar_h: 40.0, toolbar_h: 40.0, statusbar_h: 24.0, control_sm: 26.0, control_md: 30.0, control_lg: 44.0, row_height: 30.0, icon: 16.0, cart_width: 392.0, category_width: 168.0, details_width: 360.0, tile_min_width: 150.0, tile_min_height: 78.0, font_body: 13.0, font_label: 12.0, font_money: 13.0, font_money_xl: 30.0, show_shortcut_hints: true, categories_as_chips: false };
    pub const TOUCH: Metrics = Metrics { titlebar_h: 48.0, toolbar_h: 58.0, statusbar_h: 32.0, control_sm: 44.0, control_md: 44.0, control_lg: 62.0, row_height: 52.0, icon: 20.0, cart_width: 430.0, category_width: 0.0, details_width: 380.0, tile_min_width: 172.0, tile_min_height: 104.0, font_body: 15.0, font_label: 13.5, font_money: 16.0, font_money_xl: 34.0, show_shortcut_hints: false, categories_as_chips: true };
    pub const TOUCH_TARGET_MIN: f32 = 44.0;
    pub fn for_density(d: Density) -> Metrics { match d { Density::Compact => Self::COMPACT, Density::Touch => Self::TOUCH } }
}

pub mod type_scale {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Family { Ui, Num }
    #[derive(Clone, Copy, Debug)]
    pub struct Style { pub size: f32, pub line_height: f32, pub weight: u16, pub family: Family }
    /// Pane titles, bill name, inspector title. Touch 17.
    pub const TITLE: Style = Style { size: 16.0, line_height: 24.0, weight: 600, family: Family::Ui };
    /// Default text (compact). Touch 15/22.
    pub const BODY: Style = Style { size: 13.0, line_height: 20.0, weight: 400, family: Family::Ui };
    /// Emphasis, labels of key values.
    pub const BODY_STRONG: Style = Style { size: 13.0, line_height: 20.0, weight: 600, family: Family::Ui };
    /// Pane headers, field labels, toolbar (12.5). Touch 13–14.
    pub const LABEL: Style = Style { size: 12.0, line_height: 18.0, weight: 600, family: Family::Ui };
    /// Metadata, status bar. Never below 11px; Thai needs the line height.
    pub const CAPTION: Style = Style { size: 11.5, line_height: 16.0, weight: 400, family: Family::Ui };
    /// Amount due. Touch 34.
    pub const MONEY_XL: Style = Style { size: 30.0, line_height: 36.0, weight: 600, family: Family::Num };
    /// KPIs, change, counted cash.
    pub const MONEY_LG: Style = Style { size: 22.0, line_height: 28.0, weight: 600, family: Family::Num };
    /// Line amounts, table cells, tile prices (600). Always right-aligned.
    pub const MONEY: Style = Style { size: 13.0, line_height: 20.0, weight: 500, family: Family::Num };
    /// Unit prices.
    pub const MONEY_SM: Style = Style { size: 11.5, line_height: 16.0, weight: 500, family: Family::Num };
}

/// Seconds. Use 0 when reduced motion is on.
pub mod motion {
    /// Typing, selection.
    pub const INSTANT: f32 = 0.0;
    /// Hover, press, count bump.
    pub const FAST: f32 = 0.12;
    /// Toast, pane switch, payment step.
    pub const BASE: f32 = 0.2;
    /// “Just added” line flash fade.
    pub const FLASH: f32 = 0.6;
}
