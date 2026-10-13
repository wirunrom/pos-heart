---
name: POS Heart
description: A calm, offline counter workspace for Thai and English sales, in light and dark, compact and touch.
colors:
  light-canvas: "#F5F4F0"
  light-surface: "#FFFFFF"
  light-raised: "#FFFFFF"
  light-sunken: "#EBE9E3"
  light-hover: "#EBE9E3"
  light-pressed: "#DEDBD3"
  light-selected: "#FCF0D4"
  light-inverse: "#1F1E1B"
  light-ink: "#1F1E1B"
  light-muted: "#625E55"
  light-subtle: "#827D72"
  light-on-inverse: "#FFFFFF"
  light-accent-text: "#875A03"
  light-on-accent: "#121110"
  light-on-danger: "#FFFFFF"
  light-border-subtle: "#EBE9E3"
  light-border: "#DEDBD3"
  light-border-control: "#827D72"
  light-border-selected: "#1F1E1B"
  light-focus-ring: "#2563C7"
  light-accent: "#F0B232"
  light-accent-hover: "#DC9810"
  light-accent-pressed: "#B47A06"
  light-accent-soft: "#FCF0D4"
  light-success: "#17633D"
  light-success-bg: "#DDF3E6"
  light-success-solid: "#1F7F4E"
  light-warning: "#984508"
  light-warning-bg: "#FEEBDC"
  light-warning-solid: "#C25A0C"
  light-danger: "#9A2A21"
  light-danger-bg: "#FDE5E2"
  light-danger-solid: "#C0352A"
  light-info: "#1D4F9F"
  light-info-bg: "#E2EDFD"
  light-chart-1: "#B47A06"
  light-chart-2: "#47443E"
  light-chart-grid: "#EBE9E3"
  light-scrim: "rgba(18, 17, 16, 0.48)"
  dark-canvas: "#121110"
  dark-surface: "#191816"
  dark-raised: "#1F1E1B"
  dark-sunken: "#0B0B0A"
  dark-hover: "#272522"
  dark-pressed: "#302E2A"
  dark-selected: "#2A1E09"
  dark-inverse: "#F5F4F0"
  dark-ink: "#F5F4F0"
  dark-muted: "#A8A398"
  dark-subtle: "#827D72"
  dark-on-inverse: "#191816"
  dark-accent-text: "#F5C859"
  dark-on-accent: "#121110"
  dark-on-danger: "#FFFFFF"
  dark-border-subtle: "#272522"
  dark-border: "#302E2A"
  dark-border-control: "#827D72"
  dark-border-selected: "#F5F4F0"
  dark-focus-ring: "#93B9F3"
  dark-accent: "#F0B232"
  dark-accent-hover: "#F5C859"
  dark-accent-pressed: "#DC9810"
  dark-accent-soft: "#2A1E09"
  dark-success: "#7FD0A4"
  dark-success-bg: "#10301F"
  dark-success-solid: "#2F9E64"
  dark-warning: "#F8B27A"
  dark-warning-bg: "#3D1F08"
  dark-warning-solid: "#EA7317"
  dark-danger: "#F59A90"
  dark-danger-bg: "#40150F"
  dark-danger-solid: "#C0352A"
  dark-info: "#93B9F3"
  dark-info-bg: "#132748"
  dark-chart-1: "#F0B232"
  dark-chart-2: "#C9C5BB"
  dark-chart-grid: "#272522"
  dark-scrim: "rgba(0, 0, 0, 0.64)"
typography:
  display:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "30px"
    fontWeight: 600
  title-lg:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "22px"
    fontWeight: 600
  title:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "17px"
    fontWeight: 600
  body:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "14px"
    fontWeight: 400
  body-touch:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "16px"
    fontWeight: 400
  body-strong:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "14px"
    fontWeight: 600
  label:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "13px"
    fontWeight: 400
  label-touch:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "15px"
    fontWeight: 400
  caption:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "12px"
    fontWeight: 400
  money-xl:
    fontFamily: "IBM Plex Mono"
    fontSize: "34px"
    fontWeight: 600
  money-lg:
    fontFamily: "IBM Plex Mono"
    fontSize: "22px"
    fontWeight: 600
  money:
    fontFamily: "IBM Plex Mono"
    fontSize: "14px"
    fontWeight: 500
  money-touch:
    fontFamily: "IBM Plex Mono"
    fontSize: "16px"
    fontWeight: 500
  money-sm:
    fontFamily: "IBM Plex Mono"
    fontSize: "12px"
    fontWeight: 500
rounded:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
  full: "9999px"
spacing:
  s1: "4px"
  s2: "8px"
  s3: "12px"
  s4: "16px"
  s5: "20px"
  s6: "24px"
  s8: "32px"
  s10: "40px"
  s12: "48px"
components:
  light-button-primary:
    backgroundColor: "{colors.light-accent}"
    textColor: "{colors.light-on-accent}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "36px"
  light-button-primary-hover:
    backgroundColor: "{colors.light-accent-hover}"
    textColor: "{colors.light-on-accent}"
  light-button-primary-pressed:
    backgroundColor: "{colors.light-accent-pressed}"
    textColor: "{colors.light-on-accent}"
  light-button-secondary:
    backgroundColor: "{colors.light-surface}"
    textColor: "{colors.light-ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "36px"
  light-button-ghost-hover:
    backgroundColor: "{colors.light-hover}"
    textColor: "{colors.light-ink}"
    rounded: "{rounded.sm}"
    height: "36px"
  light-button-danger:
    backgroundColor: "{colors.light-danger-solid}"
    textColor: "{colors.light-on-danger}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "36px"
  light-input-search:
    backgroundColor: "{colors.light-surface}"
    textColor: "{colors.light-ink}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "0 12px 0 36px"
    height: "36px"
  light-input-money:
    backgroundColor: "{colors.light-surface}"
    textColor: "{colors.light-ink}"
    typography: "{typography.money-lg}"
    rounded: "{rounded.sm}"
    height: "44px"
  light-chip:
    backgroundColor: "{colors.light-surface}"
    textColor: "{colors.light-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.full}"
    padding: "0 14px"
    height: "32px"
  light-chip-selected:
    backgroundColor: "{colors.light-inverse}"
    textColor: "{colors.light-on-inverse}"
    rounded: "{rounded.full}"
  light-product-tile:
    backgroundColor: "{colors.light-raised}"
    textColor: "{colors.light-ink}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
    width: "136px"
    height: "84px"
  light-product-tile-unavailable:
    backgroundColor: "{colors.light-sunken}"
    textColor: "{colors.light-muted}"
    rounded: "{rounded.md}"
  light-cart-line-current:
    backgroundColor: "{colors.light-selected}"
    textColor: "{colors.light-ink}"
    rounded: "{rounded.sm}"
  light-nav-item-active:
    backgroundColor: "{colors.light-accent-soft}"
    textColor: "{colors.light-ink}"
    rounded: "{rounded.sm}"
    height: "36px"
  light-surface-panel:
    backgroundColor: "{colors.light-surface}"
    textColor: "{colors.light-ink}"
    rounded: "{rounded.lg}"
    padding: "16px"
  light-sunken-well:
    backgroundColor: "{colors.light-sunken}"
    textColor: "{colors.light-ink}"
    rounded: "{rounded.md}"
    padding: "12px"
  light-dialog:
    backgroundColor: "{colors.light-raised}"
    textColor: "{colors.light-ink}"
    rounded: "{rounded.xl}"
    padding: "24px"
    width: "480px"
  light-toast:
    backgroundColor: "{colors.light-inverse}"
    textColor: "{colors.light-on-inverse}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
  dark-button-primary:
    backgroundColor: "{colors.dark-accent}"
    textColor: "{colors.dark-on-accent}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "36px"
  dark-button-primary-hover:
    backgroundColor: "{colors.dark-accent-hover}"
    textColor: "{colors.dark-on-accent}"
  dark-button-primary-pressed:
    backgroundColor: "{colors.dark-accent-pressed}"
    textColor: "{colors.dark-on-accent}"
  dark-button-secondary:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "36px"
  dark-button-ghost-hover:
    backgroundColor: "{colors.dark-hover}"
    textColor: "{colors.dark-ink}"
    rounded: "{rounded.sm}"
    height: "36px"
  dark-button-danger:
    backgroundColor: "{colors.dark-danger-solid}"
    textColor: "{colors.dark-on-danger}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "36px"
  dark-input-search:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-ink}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "0 12px 0 36px"
    height: "36px"
  dark-input-money:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-ink}"
    typography: "{typography.money-lg}"
    rounded: "{rounded.sm}"
    height: "44px"
  dark-chip:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.full}"
    padding: "0 14px"
    height: "32px"
  dark-chip-selected:
    backgroundColor: "{colors.dark-inverse}"
    textColor: "{colors.dark-on-inverse}"
    rounded: "{rounded.full}"
  dark-product-tile:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-ink}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
    width: "136px"
    height: "84px"
  dark-product-tile-unavailable:
    backgroundColor: "{colors.dark-sunken}"
    textColor: "{colors.dark-muted}"
    rounded: "{rounded.md}"
  dark-cart-line-current:
    backgroundColor: "{colors.dark-selected}"
    textColor: "{colors.dark-ink}"
    rounded: "{rounded.sm}"
  dark-nav-item-active:
    backgroundColor: "{colors.dark-accent-soft}"
    textColor: "{colors.dark-ink}"
    rounded: "{rounded.sm}"
    height: "36px"
  dark-surface-panel:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-ink}"
    rounded: "{rounded.lg}"
    padding: "16px"
  dark-sunken-well:
    backgroundColor: "{colors.dark-sunken}"
    textColor: "{colors.dark-ink}"
    rounded: "{rounded.md}"
    padding: "12px"
  dark-dialog:
    backgroundColor: "{colors.dark-raised}"
    textColor: "{colors.dark-ink}"
    rounded: "{rounded.xl}"
    padding: "24px"
    width: "480px"
  dark-toast:
    backgroundColor: "{colors.dark-inverse}"
    textColor: "{colors.dark-on-inverse}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
---

# Design System: POS Heart

> **Source of truth.** The design system is **POS Design System v0.1**, committed in `design/` (tokens: `design/tokens/tokens.dtcg.json`, generated `design/rust/tokens.rs`; docs: `design/docs/00–07`). This file records what the native Rust/egui app actually implements (`apps/pos-desktop/src/ui/tokens.rs`, `theme.rs`, `widgets.rs`, `icons.rs`, `shell.rs` and the screen modules). When the two disagree, change `design/` first, regenerate tokens, then update the app and this record. Never hand-edit `ui/tokens.rs`.

## Overview

**Creative North Star: "Calm counter"**

A warm-paper workspace: stone neutrals, dark ink, thin hairlines instead of heavy frames, and a single saffron signal that marks the action that moves money. The screen stays quiet so the cashier always knows which button comes next. Money is set in a fixed-width face, shown in full to two decimals, and right-aligned everywhere it appears.

The world draws from three explored directions. From Studio: layered surfaces, a neutral frame with one signal color, the workspace switcher, `Ctrl K`, switchable density, held bills as tabs, offline and shift status always in the top bar. From Ledger: IBM Plex Mono money, right-aligned columns, F-key hints on desktop, hairlines instead of thick frames. From Counter: a real touch density (48–64 px controls), quick-cash chips, cart steppers, and an amount due readable from a metre away.

Two independent axes shape every screen. Appearance is Light, Dark or System (follows the OS). Density is Compact (keyboard and mouse, shortcut hints visible) or Touch (larger controls and type, hints hidden). Every screen supports both input methods in both densities.

**Key Characteristics:**
- Warm stone neutrals in both themes; saffron is the only brand hue.
- One saffron primary per view, reserved for commit actions such as take payment, confirm and save.
- IBM Plex Sans + IBM Plex Sans Thai for UI; IBM Plex Mono for every amount.
- 4 pt spacing grid, five radius tiers, three elevation levels.
- Every action has a visible control and a keyboard shortcut.
- Built-in line icons painted as vectors, always paired with a text label or tooltip.

## Colors

A warm stone frame with a single saffron signal, plus four status hues that only appear when something needs attention.

### Primary
- **Saffron** (light-accent / dark-accent): fill of the one commit button per view (take payment `F9`, confirm close, save product, add product), always with dark on-accent text in both themes. Hover deepens in light (light-accent-hover) and brightens in dark (dark-accent-hover); press uses the accent-pressed step.
- **Saffron Wash** (light-accent-soft / dark-accent-soft): active workspace in the switcher menu and the brief flash on a newly added cart line.
- **Saffron Ink** (light-accent-text / dark-accent-text): line options and notes in the cart, links, accent text in receipts. Text only, never a fill.
- **Selected Saffron** (light-selected / dark-selected): the current cart line (keyboard target), selected table rows in history and catalog, and text selection.

### Neutral
- **Stone Canvas** (light-canvas / dark-canvas): the app background behind panels.
- **Surface** (light-surface / dark-surface): cart panel, tables, cards, inputs, secondary buttons.
- **Raised** (light-raised / dark-raised): product tiles, menus, popovers, dialogs, sheets. In light this is the same white as Surface and is separated by shadow and border; in dark it is one step lighter.
- **Sunken Well** (light-sunken / dark-sunken): order summary block, KPI wells, unavailable product tiles.
- **Hover / Pressed** (light-hover, light-pressed / dark-): row, tab, ghost and secondary feedback fills.
- **Inverse** (light-inverse / dark-inverse) with on-inverse text: selected chips, count badges, toasts, tooltips, the completed-step dot.
- **Ink** (light-ink / dark-ink): primary text and all money values.
- **Muted** (light-muted / dark-muted): labels, metadata and secondary text. Meets 4.5:1 on surface and sunken.
- **Subtle** (light-subtle / dark-subtle): placeholders and disabled text only. Never for information.
- **Hairline / Border / Control border** (border-subtle, border, border-control): row dividers; card and tile outlines; input and control boundaries (at least 3:1 on surface).
- **Selected border** (light-border-selected / dark-border-selected): 2 px outline on product tiles already in the cart, and the firm border of the focused sell search.
- **Focus Blue** (light-focus-ring / dark-focus-ring): the 2 px keyboard-focus ring drawn 3 px outside a control. Deliberately not saffron.

### Status
- **Success** (success, success-bg, success-solid): paid, balanced, the open-shift dot, "on sale" badges.
- **Warning** (warning, warning-bg, warning-solid): low stock, unsaved, over/short within tolerance.
- **Danger** (danger, danger-bg, danger-solid with on-danger): out of stock, failed, refund amounts, validation errors; danger-solid fills destructive buttons only.
- **Info** (info, info-bg): in progress, informational banners.
- **Charts**: chart-1 (saffron, current period), chart-2 (stone, comparison), chart-grid for gridlines.
- **Scrim** (light-scrim / dark-scrim): the backdrop behind dialogs and sheets.

### Named Rules
**The One Saffron Rule.** At most one saffron primary button per view, and only for an action that commits or moves money (take payment, confirm, save). Everything else is secondary, ghost or danger.

**The Semantic Only Rule.** Screens read colors from the active `Palette` through role names. They never use `primitive` ramps or hex literals, and they never mix light and dark roles in one control.

**The Not By Color Alone Rule.** Status always pairs a hue with text (badge label, "Out · 0", "Offline"). Money stays readable without its color.

## Typography

**UI Font:** IBM Plex Sans (Latin) chained to IBM Plex Sans Thai, Regular and SemiBold, with Noto Sans Thai as the fallback for rare Thai marks (e.g. ฤๅ).
**Money Font:** IBM Plex Mono Medium and SemiBold, with Thai falling back to Plex Sans Thai.

**Character:** A single humanist superfamily. Thai and Latin share stroke and proportions, so bilingual product names sit naturally on one line, and the mono face makes digit columns line up. All fonts are bundled (OFL) so the app renders the same offline.

### Hierarchy
- **Display** (600, 30 px): one dashboard headline per screen at most.
- **Title Large** (600, 22 px): page titles ("All products", "รายงานประจำวัน").
- **Title** (600, 17 px): panel, dialog and section titles; bill name; extra-large button labels.
- **Body** (400, 14 px compact / 16 px touch): default text, product names, table cells.
- **Body Strong** (600, 14 / 16 px): button labels and emphasised values.
- **Label** (13 px compact / 15 px touch): field labels, tabs, segmented controls, chips. Rendered in Regular, or SemiBold for tabs, segments and chips.
- **Caption** (400, 12 px): metadata, helper text, SKU lines, side-nav group labels. Never smaller than 12 px, because Thai marks need the room.
- **Money XL** (Mono 600, 34 px): the amount due on the sell screen. One per view.
- **Money Large** (Mono 600, 22 px): change, cash received, KPI tiles, the extra-large pay button amount.
- **Money** (Mono 500, 14 compact / 16 touch): line amounts, table cells, tile prices. Always right-aligned in columns.
- **Money Small** (Mono 500, 12 px): unit prices, secondary amounts, shortcut key hints.

egui lays out lines from font metrics, so the line heights in `design/` (e.g. body 22, caption 18) are honored through row heights and spacing rather than set on each run.

### Named Rules
**The Whole Baht Rule.** Money is always IBM Plex Mono, shows every digit with two decimals (`1,019.00`), and is right-aligned. Never abbreviate, truncate or round it for display.

**The Density Type Rule.** Only body, label and money grow with Touch density (14→16, 13→15, 14→16). Titles, captions and the large money sizes stay fixed.

## Layout

**Spacing.** A 4 pt grid (s1 4 through s12 48). Widgets space by 8 px; panels pad 16 px; dialogs pad 24 px; field groups separate by 24 px.

**Density metrics.**

| | Compact | Touch |
|---|---|---|
| Controls sm / md / lg / xl | 28 / 36 / 44 / 52 | 40 / 48 / 56 / 64 |
| Row height | 40 | 56 |
| Icon | 20 | 24 |
| Top bar | 52 | 64 |
| Product tile minimum | 136 × 84 | 176 × 104 |
| Shortcut hints | shown | hidden |

The minimum hit area in Touch is 44 px.

**Shell.** The top bar has the workspace switcher on the left (current workspace with icon and chevron; a menu lists every workspace with `Alt 1–9`; Tables is not listed: it is the floor pane of Sell, still `Alt 9`), the shop name next to it, a centred command field (`Ctrl K`), and on the right the status pills (Offline when there is no network; shift open with time) and the display-settings popover (appearance, density, language). Toasts sit bottom-centre above the content.

**Sell.** The cart is a fixed column on the left: 360 px when the window is narrower than 1100, otherwise 392 compact or 420 touch. It holds the Current/Held tabs, bill name, cart lines, the sunken order summary with Money XL amount due, the cash / transfer / card segmented control, received and change, quick-cash chips, and the full-width saffron pay button at the bottom. On the right, a category list (with counts) sits beside a search field and a grid of product tiles. The grid takes as many columns as the tile minimum allows, and each row's height grows to fit its tallest tile.

**Settings sections.** Settings uses the same SideNav rows as Products (no counts) with seven sections, one shown at a time beside a scrolling column capped at 760 px; a warning-coloured dot marks sections holding unsaved drafts. Checkbox groups inside cards (e.g. Printed documents → Show) use equal-width columns (1–3 by width) rather than `horizontal_wrapped`, which squeezes wrapping labels to one letter per line.

**Management (Products, History, Shift, Reports, Settings).** A side nav or filter column (176 px, 208 px at 1100 and wider) is followed by the main table or form. A detail inspector docks on the right when the window allows it: product editor 340 px at 1180 and wider, receipt detail 400 px. On narrower windows the same content opens as a sheet over a scrim. Reports use a row of KPI stat tiles above chart and table cards.

### Keyboard map
| Key | Action |
|---|---|
| `F2` or `/` | Focus product search |
| `↑` `↓` | Move the current cart line |
| `+` `−` | Change the current line's quantity (when search is empty) |
| `F3` | Edit quantity of the current line |
| `F6` | Options / note for the current line |
| `Del` | Remove the current line (undoable) |
| `F4` | Hold bill |
| `F10` | Send to kitchen (when tables exist) |
| `F9` | Pay |
| `Ctrl K` | Command palette |
| `Alt 1–9` | Switch workspace |
| `Ctrl Z` | Undo last undoable action |
| `Ctrl S` | Save in the product editor |
| `Esc` | Clear search / close dialog, sheet or popover |

### Named Rules
**The Both Hands Rule.** Every action has a visible control and a key. In Compact, buttons and fields show their key hint (`F9`, `F2`, `Ctrl K`) as a bordered mono chip; in Touch the hint is hidden but the key still works.

## Elevation & Depth

A hybrid system: tonal layers carry most of the depth, and soft ambient shadows lift only what sits above the canvas. There are three planes. **Surface** (cart, tables, cards; lg radius; shadow 1) sits on the canvas. **Sunken** wells (md radius, no shadow) recess summaries inside a surface. **Raised** (tiles, menus at shadow 2, dialogs and sheets at shadow 3 with the scrim) floats above. In dark mode shadows fade into the background, so surfaces also get a 1 px border-subtle highlight edge.

### Shadow Vocabulary
- **Shadow 1** (`0 1px 6px`, black 5.5 % light / 47 % dark): surfaces on the canvas.
- **Shadow 2** (`0 4px 16px`, black 12 % light / 59 % dark): menus, popovers, toasts.
- **Shadow 3** (`0 16px 48px`, black 20 % light / 71 % dark): dialogs and sheets.

### Named Rules
**The Dark Edge Rule.** In dark mode every surface gets a 1 px border-subtle edge. Never rely on shadow alone to separate dark planes.

## Shapes

Gently rounded and consistent by tier: **xs** (4 px) for badges, key hints and count bubbles; **sm** (6 px) for buttons, inputs and nav rows in Compact; **md** (8 px) for the same controls in Touch, plus product tiles, menus, wells and toasts; **lg** (12 px) for surfaces; **xl** (16 px) for dialogs and sheets; **full** for chips, status pills, status dots and count badges. Borders are 1 px hairlines. The 2 px outline is reserved for selection (tiles in the cart) and the focus ring.

## Components

### Buttons
- **Shape:** control radius (sm Compact / md Touch). Heights follow the density metrics (sm/md/lg/xl), and the label is Body Strong (Title at xl, Label at sm).
- **Primary:** saffron fill, dark on-accent text. It may carry the full amount in the money font (`รับเงินสด ฿3,331.00`) and an `F9` hint chip. Used once per view.
- **Secondary:** surface fill with a 1 px control border. This is the default.
- **Ghost:** transparent, with a hover fill. Used for toolbar and row actions.
- **Danger:** danger-solid fill with on-danger text, for irreversible actions confirmed in a dialog. **Danger quiet** uses danger text with a hairline border for less severe destructive actions.
- **States:** hover and pressed fills step through the palette. Disabled buttons fade. Loading swaps the content for a spinner. The focus ring appears for keyboard focus only.
- **Icon button:** square, icon-only, and always has an accessible label and tooltip.

### Chips, Pills & Badges
- **Chip:** full-radius pill with a control border. When selected it fills with inverse. Quick-cash chips share the row width equally and use the money font.
- **Status pill:** a pill in the top bar with a dot (solid = live state, hollow = Offline) and Label text. It opens detail on click.
- **Badge:** 1–3 words of status on the tone's bg/fg pair (On sale, Out · 0, Not for sale). Never a button.
- **Count badge:** inverse full-radius bubble on tabs and on tiles already in the cart.

### Cards / Containers
- **Surface panel:** lg radius, surface fill, shadow 1, 16 px padding, plus the dark edge.
- **Sunken well:** md radius, sunken fill, 12 px padding: order summary, payment block.
- **Stat tile:** label, Money Large value, and a muted line explaining the number.

### Inputs / Fields
- **Text input:** surface fill, 1 px control border, control radius, control height. The label sits above in Label/muted; inline error text is danger.
- **Money input:** right-aligned mono. It selects everything on focus, formats as `1,500.00` on blur and never rounds silently.
- **Sell search:** search icon at left and `F2` hint at right. It keeps focus all day for barcode scanners, so focus shows as a firm 1.5 px border-selected outline, not the blue ring. `Esc` clears it.
- **Segmented:** 2–5 options with immediate effect (payment method, today/yesterday, amount/quantity).
- **Switch / Checkbox:** drawn to spec. The checkbox supports a mixed state for table headers.

### Navigation
- **Workspace switcher:** a top-left button that opens a menu of seven workspaces with `Alt N` hints. The active row uses Saffron Wash.
- **Command palette (`Ctrl K`):** raised overlay with a search field and keyboard-navigable entries for workspaces and actions.
- **Side nav:** rows at control height with right-aligned counts. The active row uses the sunken fill with Body Strong text. Groups are headed by a muted SemiBold caption ("Categories", "Views").
- **Tabs:** underlined, with optional count badges (Current bill / Held 2).
- **Steps:** a 3–5 stage indicator for shift closing (completed stages show a check in an inverse dot).

### Product tile
Raised fill, md radius, 1 px border; padding 12 × 10 Compact, 12 × 12 Touch. The name wraps (Body) at top left; stock or status sits at bottom left; the price (Money) sits at bottom right. A tile already in the cart gets the 2 px selected border and an inverse count badge at top right. Unavailable tiles (out of stock, not for sale) sink to the sunken fill with muted text and a danger status label. While pressed, the tile shrinks by 1.5 px. There are no product images.

### Cart line
A quantity box, the name with saffron-ink options and note lines beneath, and the right-aligned line amount, separated by hairlines. The current line gets the Selected Saffron fill and reveals its stepper (−, quantity, +), note (`F6`) and remove controls.

### Feedback
- **Toast:** inverse pill-card at the bottom centre, Body text, an optional saffron **Undo** action (also `Ctrl Z`) and a close control. Shadow 2, max 560 px.
- **Banner:** a persistent problem at the top of the area it concerns, in its status tone, with an optional action.
- **State view:** icon, title and body for empty, no-results and error states inside a content area.
- **Dialog:** blocking, at most 480 px wide, xl radius, 24 px padding, shadow 3 over the scrim. The safe action sits on the left and the commit on the right. `Esc` or a scrim click closes it.
- **Sheet:** the same content as an inspector, presented over the scrim on narrow windows.

### Icons
A built-in line icon set painted as vectors (no icon font, no emoji), sized to the density icon metric. Icons always accompany a text label or an accessible tooltip.

## Do's and Don'ts

### Do:
- **Do** use exactly one saffron primary per view, for the commit or money-moving action.
- **Do** read colors and sizes from `Theme` (palette, metrics, space, radius, type_scale). Change values in `design/tokens/` and regenerate.
- **Do** set every amount in IBM Plex Mono, in full with two decimals, right-aligned in columns.
- **Do** give every action both a visible control and a shortcut, and show the hint in Compact.
- **Do** check every screen in Light and Dark, Compact and Touch, Thai and English, at 1000 × 700 and 1280 × 820.
- **Do** keep keyboard focus visible with the blue ring, except for the always-focused sell search, which uses its firm border.
- **Do** prefer undo (toast plus `Ctrl Z`) over confirmation. Confirm only irreversible actions, and state the effect in numbers.

### Don't:
- **Don't** use the solid saffron fill for navigation, decoration or a second button in the same view. Navigation may use only the Saffron Wash, and charts only series 1.
- **Don't** use subtle text for information, or set any text below 12 px.
- **Don't** separate dark surfaces by shadow alone. Keep the highlight edge.
- **Don't** convey status by color alone.
- **Don't** add product images or any remaining Future-exploration surface (split tender, multi-branch, tablet layout) until it is designed in `design/` first. PromptPay QR, discounts, stock count, receipt printing, tables and the kitchen screen are built (see below).

## Implementation notes and intentional deviations from `design/`

- **No product images:** tiles are text-first; the image slot in the reference is not built.
- **Offline pill:** the app is offline-first and shows a hollow-dot "Offline" status pill rather than a sync status component.
- **Printing is not a step:** payment completes without a print stage. With a printer set up, receipts print automatically (or via a Print receipt button on the paid screen); a failed print shows a warning banner with Print again and never affects the sale. The printer card in Settings and the print buttons use existing primitives; printing itself is not in `design/`.
- **PromptPay QR and discounts:** built from the existing dialog, segmented, chip and kv-row primitives. The QR is dark-on-white with a quiet zone in both themes; discount rows use the danger value colour with a U+2212 minus.
- **Search focus:** the sell search keeps focus with a firm border instead of the focus ring (see Inputs).
- **Label weight:** only Regular and SemiBold Plex Sans are bundled, so the 500-weight Label style renders Regular. Money uses Plex Mono Medium (500) as specified.
- **Motion:** only the 120 ms `fast` duration is wired (egui animation time). The `base` (200 ms) and `slow` (280 ms) tokens exist but have no animated surfaces yet.
- **Manager approval and lock screen (built):** an action whose role access is Needs approval opens a blocking approval dialog (lock icon, "Needs a manager's approval", what and who asked, chips for members who may approve excluding the requester, PIN pad that also takes keyboard digits); the action runs only after a valid PIN and the approval covers that one action. Such buttons show a lock icon with "Needs approval"; denied actions are disabled with "Your role cannot do this". The role editor uses a three-way segmented control (Allowed / Approval / Denied) per permission in a scrolling list. With staff, a full-window lock screen (name choice + PIN pad) blocks everything until someone signs in; the top bar carries a name chip with Lock / switch user (`Ctrl L`).
- **VAT and tax invoices (built):** VAT rows on receipts use ordinary kv rows (value before VAT, VAT); Issue tax invoice opens a standard blocking dialog for the buyer. Dialog action rows (`w::dialog_actions`) have a bounded height so a right-to-left action row cannot stretch a modal.
- **Service charge, VAT on top and cash rounding (built):** extra kv rows in the order summary footer, collapsing into one caption line on short or compact windows so the pay button stays pinned.
- **Future items not built:** split tender, multi-branch and the tablet layout, as well as the unused chart series 3–4 and info-solid tokens.


## Recovery, import, activity and cost surfaces (2026-10-13)

The Data & backup card uses secondary actions, labeled folder-path input and an inline summary before a blocking danger confirmation. A prepared recovery replaces the shell with a stop/restart instruction and safety-backup path. CSV import uses the existing dialog, a bounded scrolling preview/error body and pinned secondary confirm/cancel actions. Activity is workspace eight: flat filters, pagination and divided event rows with expandable snapshots; money uses existing mono/right-aligned formatting. Product cost is an optional MoneyField; gross profit is a collapsible report section with explicit unknown-cost coverage. No new palette, metrics or typography tokens.

## Tables and kitchen surfaces (2026-10-13)

`design/` lists TableCard and KitchenTicket as Future exploration without full specs; they are now built from existing primitives and tokens (no new palette, metrics or typography tokens), so treat this section as the implemented reference until `design/` specifies them.

- **Tables floor (Sell → Tables tab, `Alt 9`):** Products / Tables underline tabs head Sell's right panel once tables exist; the floor replaces categories and tiles there while the bill stays on the left. Zones as body-strong headings over a grid of table cards that share the row evenly (min 152 px, 176 px Touch). The open table has a 3 px saffron (accent) border and "Open"; move mode shows an info banner with Cancel and fades busy tables. Header: secondary Counter / takeaway (plus icon) and ghost Open kitchen screen · N. A table bill's caption is "Table · N guests" plus a chevron icon (no ▾ glyph: the UI font lacks it) opening a guests popover. A free card is a plain surface with the name, seats and "Free" in success text; a busy card uses the Saffron Wash fill with the 2 px selected border, the bill total in Money type (priced exactly like checkout), guests · minutes since the first ticket and an "unsent N" label in warning text. Tapping a card loads its bill on the left and switches back to Products; sending to the kitchen, paying or holding a table bill returns to the floor. Without active tables the tabs are hidden and Sell is unchanged. Table bills have two footer modes: ordering (default) is one sunken total line (items · SC/VAT/discount detail · total in Money), Send to kitchen and a primary Take payment ฿total (F9); Take payment opens the full payment section (`SellState.paying` = that cart id) with a ghost Back to ordering (Esc) and a warning when dishes are unsent; the kitchen button is hidden there. Adding a dish or switching bills returns to ordering. Counter bills keep the inline payment section.
- **Table bill in Sell:** the bill name is the table; "Send to kitchen (F10)" is a full-width secondary footer button labelled with new/cancel counts ("Kitchen up to date" and disabled when nothing changed), dropping from Lg to Md on short windows so the saffron pay button stays the only primary and stays pinned.
- **Kitchen screen:** a separate OS window (egui immediate viewport) for a kitchen monitor. Ticket cards are top-aligned in a wrapping row, oldest first. The header shows table, guests and minutes waiting, turning warning (orange) from 10 minutes and danger (red) from 20, always with the number as text. Tapping a line ticks it cooked (strikethrough, muted); cancellation lines are marked in danger with a text label; "Done" bumps the ticket; "Recently done" lists bumped tickets with "Recall".
- **Settings → Tables & zones:** a card listing zones and tables with an add/edit dialog (name auto-incremented from the last table, zone field with chips for existing zones, seats field, active switch). The product editor gains a "Goes to the kitchen" switch once tables exist; the printer card gains a Kitchen tickets segmented control (Off / Receipt printer / Kitchen printer, the last with its own connection and paper).
- **Printed kitchen ticket:** large table name, ticket number, time, guests and staff, large dish lines with options, bold `** note **`, and `*** CANCEL ***` slips for cancellations.

## Print preview, templates and check bill surfaces (2026-10-13)

Printed documents are not specified in `design/`; these surfaces reuse existing primitives and tokens (no new palette, metrics or typography tokens).

- **Paper card:** any document is shown as the printer's own 1-bit render on a white paper card (primitive Stone 0 fill in both themes, like the PromptPay QR's white field, with the default border and shadow level 1). It is an image of exactly what prints, scaled to the card width, never re-laid-out with UI text.
- **Print preview dialog:** standard blocking dialog (440 wide) with a scrolling paper card. Print (printer icon, primary) appears only when a printer is configured; Close is ghost. The receipt dialog and Sales history detail show "Print preview" (always) beside Print copy; the paid screen shows it too.
- **Check bill:** Sell → More → Check bill opens the same preview dialog; printing happens from there. The paper reads "Check bill · not a receipt" so it is never mistaken for proof of payment.
- **Settings → Printed documents:** a card with a segmented control (Receipt / Check bill / Kitchen ticket), controls on the left in a fixed-width column (text areas bounded so the card cannot outgrow its siblings) and the live preview on the right (280 px, 220 px under 700 px of card width). Logo: path field with Use image / Remove logo, or drop a PNG/JPEG on the window. Defaults and Discard are secondary; Save is the card's only primary and needs the Settings permission.
- **Printed receipt/check bill:** optional centred logo, shop name, header lines, title, lines at Small/Normal/Large scale, totals, optional centred PromptPay QR (~45% of paper width, quiet zone), footer. VAT receipts and tax invoices keep every legally required line.
