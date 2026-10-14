---
name: POS Heart
description: A calm, offline counter workspace for Thai and English sales: graphite chrome, paper panes, compact and touch.
colors:
  chrome: "#1F2023"
  chrome-raised: "#2A2B2F"
  chrome-hover: "#35373B"
  border-chrome: "#46484D"
  on-chrome: "#ECEBE7"
  on-chrome-muted: "#A6A7A2"
  canvas: "#2A2B2F"
  surface: "#F2F2EF"
  raised: "#FFFFFF"
  sunken: "#E9E9E5"
  hover: "#EEF1F5"
  pressed: "#E2E6EC"
  selected: "#DCE3ED"
  inverse: "#2A2B2F"
  ink: "#1C1D1F"
  muted: "#5C5E5A"
  subtle: "#868782"
  on-inverse: "#ECEBE7"
  accent-text: "#7E5303"
  on-accent: "#141310"
  on-danger: "#FFFFFF"
  border-subtle: "#E0E0DB"
  border: "#CDCDC7"
  border-control: "#868782"
  border-selected: "#4F75AA"
  focus-ring: "#2F6FD0"
  accent: "#F0B232"
  accent-hover: "#DC9810"
  accent-pressed: "#C48B10"
  accent-soft: "#FBE7B5"
  success: "#1A6E45"
  success-bg: "#DCEFE4"
  success-solid: "#2E8B57"
  warning: "#9A5306"
  warning-bg: "#F8E7D2"
  warning-solid: "#C76A0A"
  danger: "#A8302A"
  danger-bg: "#F7DFDC"
  danger-solid: "#B3332B"
  info: "#285C9E"
  info-bg: "#DDE8F6"
  info-solid: "#3D6FB5"
  cat-food: "#C9781A"
  cat-drink: "#3F78B5"
  cat-goods: "#5F8F5A"
  cat-snack: "#A0569A"
  chart-1: "#B07C0C"
  chart-2: "#55575C"
  chart-3: "#4F75AA"
  chart-4: "#2E8B57"
  chart-grid: "#E0E0DB"
  scrim: "rgba(9, 9, 11, 0.45)"
  jade-accent: "#0F7A5F"
  jade-accent-hover: "#0C6550"
  jade-accent-pressed: "#0A5443"
  jade-accent-soft: "#CFEADF"
  ocean-accent: "#1D6A8C"
  ocean-accent-hover: "#185A77"
  ocean-accent-pressed: "#134A62"
  ocean-accent-soft: "#CFE3EE"
  plum-accent: "#7B3C8F"
  plum-accent-hover: "#68327A"
  plum-accent-pressed: "#562966"
  plum-accent-soft: "#EAD7F1"
  ink-accent: "#2C2D31"
  ink-accent-hover: "#1F2023"
  ink-accent-pressed: "#111214"
  ink-accent-soft: "#E2E2DE"
  ink-chart-1: "#45474C"
  on-accent-light: "#FFFFFF"
typography:
  title:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: "24px"
  title-touch:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "17px"
    fontWeight: 600
  body:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "20px"
  body-touch:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: "22px"
  body-strong:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "13px"
    fontWeight: 600
    lineHeight: "20px"
  label:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: "18px"
  label-touch:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "13.5px"
    fontWeight: 600
  caption:
    fontFamily: "IBM Plex Sans, IBM Plex Sans Thai, Noto Sans Thai"
    fontSize: "11.5px"
    fontWeight: 400
    lineHeight: "16px"
  money-xl:
    fontFamily: "IBM Plex Mono"
    fontSize: "30px"
    fontWeight: 600
    lineHeight: "36px"
  money-xl-touch:
    fontFamily: "IBM Plex Mono"
    fontSize: "34px"
    fontWeight: 600
  money-lg:
    fontFamily: "IBM Plex Mono"
    fontSize: "22px"
    fontWeight: 600
    lineHeight: "28px"
  money:
    fontFamily: "IBM Plex Mono"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: "20px"
  money-touch:
    fontFamily: "IBM Plex Mono"
    fontSize: "16px"
    fontWeight: 500
  money-sm:
    fontFamily: "IBM Plex Mono"
    fontSize: "11.5px"
    fontWeight: 500
    lineHeight: "16px"
rounded:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "10px"
  xl: "14px"
  window: "12px"
  square-cap: "3px"
  full: "9999px"
spacing:
  s1: "4px"
  s2: "8px"
  s3: "12px"
  s4: "16px"
  s5: "20px"
  s6: "24px"
  s8: "32px"
  pane-gap: "6px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.md}"
    padding: "0 16px"
    height: "30px"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
    textColor: "{colors.on-accent}"
  button-primary-pressed:
    backgroundColor: "{colors.accent-pressed}"
    textColor: "{colors.on-accent}"
  button-primary-disabled:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.subtle}"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.md}"
    padding: "0 16px"
    height: "30px"
  button-ghost-hover:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    height: "30px"
  button-danger:
    backgroundColor: "{colors.danger-solid}"
    textColor: "{colors.on-danger}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.md}"
    padding: "0 16px"
    height: "30px"
  button-danger-disabled:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.subtle}"
  input-text:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    height: "30px"
  chip:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.full}"
    padding: "0 14px"
    height: "32px"
  chip-selected:
    backgroundColor: "{colors.inverse}"
    textColor: "{colors.on-inverse}"
    rounded: "{rounded.full}"
  workspace-tab:
    textColor: "{colors.on-chrome-muted}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
  workspace-tab-active:
    backgroundColor: "{colors.chrome-hover}"
    textColor: "{colors.on-chrome}"
    rounded: "{rounded.md}"
  toolbar:
    backgroundColor: "{colors.chrome-raised}"
    textColor: "{colors.on-chrome}"
    height: "40px"
  status-bar:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.on-chrome}"
    typography: "{typography.caption}"
    height: "24px"
  pane:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.lg}"
  pane-header:
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    height: "28px"
  product-tile:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.lg}"
    padding: "10px 12px"
    width: "150px"
    height: "78px"
  product-tile-unavailable:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.muted}"
    rounded: "{rounded.lg}"
  cart-line-current:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
  card:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.lg}"
    padding: "16px"
  sunken-well:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "12px"
  dialog:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.xl}"
    padding: "24px"
  toast:
    backgroundColor: "{colors.inverse}"
    textColor: "{colors.on-inverse}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
---

# Design System: POS Heart

> **Source of truth.** The design system is **POS Design System v0.2**, committed in `design/` (tokens: `design/tokens/tokens.dtcg.json`, generated `design/rust/tokens.rs`; docs: `design/docs/00–07`). This file records what the native Rust/egui app actually implements (`apps/pos-desktop/src/ui/tokens.rs`, `theme.rs`, `widgets.rs`, `icons.rs`, `chrome.rs`, `shell.rs` and the screen modules). When the two disagree, change `design/` first, regenerate tokens, then update the app and this record. Never hand-edit `ui/tokens.rs`.

## Overview

**Creative North Star: "Calm counter"**

A graphite workbench holding sheets of paper. The window frame (workspace tabs, page toolbar, status bar and the canvas between panes) is dark graphite; the working surfaces are light paper panes with hairlines instead of heavy frames. One accent colour (Saffron by default) marks the action that moves money. The screen stays quiet so the cashier always knows which button comes next. Money is set in a fixed-width face, shown in full to two decimals, and right-aligned everywhere it appears.

The world draws from three explored directions. From Studio: a neutral frame with one signal colour, workspace tabs, `Ctrl K`, switchable density, offline and shift status always visible. From Ledger: IBM Plex Mono money, right-aligned columns, F-key hints on desktop, hairlines instead of thick frames. From Counter: a real touch density (44–62 px controls), quick-cash chips, cart steppers, and an amount due readable from a metre away.

There is one theme; there is no light/dark/system appearance setting. Three device preferences shape every screen: **Density** (Compact for keyboard and mouse with shortcut hints, Touch with larger controls and type and hints hidden), **Accent** (five brand presets that change only the commit colour family) and **Shape** (Soft: rounded floating panes; Square: edge-to-edge panes, corners capped at 3 px). Every screen supports both input methods in both densities and both shapes.

**Key Characteristics:**
- Graphite chrome and canvas; paper panes; the accent is the only brand hue.
- One accent primary per view, reserved for commit actions such as take payment, confirm and save.
- IBM Plex Sans + IBM Plex Sans Thai for UI; IBM Plex Mono for every amount.
- 4 pt spacing grid, panes 6 px apart (Soft) or split by 1 px (Square).
- Every action has a visible control and a keyboard shortcut.
- Built-in line icons painted as vectors, always paired with a text label or tooltip.
- Category colours are data colours (tile stripe, dot), never status.

## Colors

A graphite frame around paper panes with a single accent signal, plus four status hues that only appear when something needs attention and four category hues that identify data.

### Chrome
- **Graphite Chrome** (chrome): title bar with workspace tabs, and the status bar.
- **Graphite Raised** (chrome-raised): the page toolbar. **Canvas** (canvas) has the same value, so panes read as sheets lying on the toolbar's plane; the Home shift card sits on it in chrome-hover.
- **Chrome Hover** (chrome-hover): hover/active fill for chrome buttons and the selected workspace tab; selected touch category chips use chrome-raised with on-chrome text.
- **Chrome text** (on-chrome / on-chrome-muted): labels, icons and the clock on chrome; muted for inactive tabs, shortcut hints and secondary status text (at least 5.8:1 on chrome).
- **Chrome border** (border-chrome): separators between toolbar groups and status items.

### Accent (brand preset)
- **Accent** (accent; default Saffron #F0B232): fill of the one commit button per view (Pay `F9`, confirm payment, confirm close, save) and the 2 px underline of the selected workspace tab. Hover and press step through accent-hover and accent-pressed. Text on it is on-accent (near-black for Saffron, white for every other preset).
- **Accent Wash** (accent-soft): the brief "just added" flash on a new cart line.
- **Presets** (`AccentPreset`, saved per device in `accent.txt`): Saffron หญ้าฝรั่น (default), Jade หยก, Ocean น้ำทะเล, Plum พลัม, Ink หมึก. A preset changes only accent, accent-hover, accent-pressed, on-accent, accent-soft and chart-1. Jade sits near success and Ocean near selection/focus, so their commit buttons always carry a text label.
- **Accent Ink** (accent-text, #7E5303): line options and notes in the cart, links. Text only, never a fill. It is not part of the preset swap and stays saffron-brown under every accent.

### Paper (inside panes)
- **Surface** (surface): every pane (cart, lists, tables, settings content).
- **Raised** (raised): inputs, product tiles, table cards, settings cards, Home task cards, menus, popovers and dialogs.
- **Sunken** (sunken): order summary wells, segmented tracks, unavailable tiles, the selected pane tab (Soft), and disabled primary/danger buttons.
- **Hover / Pressed** (hover, pressed): cool slate fills for pointer hover and press on rows and tiles; buttons hover to sunken.
- **Selected** (selected + border-selected): the current cart line, the active category and side-nav row, selected table rows. In Square, selected rows add a 2 px border-selected bar at the left edge. Tiles already in the bill and the current table card get a 2 px border-selected outline.
- **Inverse** (inverse) with on-inverse text: selected chips, count badges, toasts, tooltips.
- **Ink** (ink): primary text and all money values. **Muted** (muted): labels and metadata, at least 4.5:1 on surface, raised, sunken and selected. **Subtle** (subtle): placeholders and disabled text only.
- **Hairline / Border / Control border** (border-subtle, border, border-control): row dividers and pane header rules; card, tile and outline edges; input and control boundaries (at least 3:1 on raised). Free tables use a dashed border-control outline.
- **Focus Blue** (focus-ring): the 2 px keyboard-focus ring drawn 3 px outside a control. Deliberately not the accent.

### Status
- **Success** (success, success-bg, success-solid): paid, balanced, the open-shift dot, printer ready.
- **Warning** (warning, warning-bg, warning-solid): low stock, unsaved, no open shift, unsent dishes (badge and table stripe).
- **Danger** (danger, danger-bg, danger-solid with on-danger): out of stock, failed, refund amounts, validation errors; danger-solid fills destructive buttons only.
- **Info** (info, info-bg, info-solid): in progress, informational banners, "In kitchen" badges and the seated-table stripe.
- **Scrim** (scrim): the backdrop behind dialogs and sheets.

### Category and chart
- **Category** (cat-food, cat-drink, cat-goods, cat-snack): `Theme::cat_color(i)` cycles the four by category position (the catalog's category order), so the same category keeps its colour on its tile stripe, list dot and chip dot. Never used as status.
- **Charts**: chart-1 (accent family, current period), chart-2 (graphite grey, comparison), chart-grid for gridlines. chart-3 and chart-4 exist but have no series yet.

### Named Rules
**The One Accent Rule.** At most one accent primary button per view, and only for an action that commits or moves money (take payment, confirm, save). Everything else is secondary, ghost or danger. The workspace-tab underline is the only non-button use of the solid accent.

**The Semantic Only Rule.** Screens read colours from the active `Palette` through role names (`Theme::of(ui).p`). They never use `primitive` ramps or hex literals.

**The Chrome / Paper Rule.** Chrome colours (chrome, chrome-raised, chrome-hover, on-chrome) are for the frame and anything drawn directly on the canvas; paper colours are for everything inside a pane. Don't put paper text colours on graphite or chrome text inside a pane.

**The Not By Color Alone Rule.** Status always pairs a hue with text (badge label, "Out · 0", "Offline", "Not sent 2"). Money stays readable without its colour.

## Typography

**UI Font:** IBM Plex Sans (Latin) chained to IBM Plex Sans Thai, Regular and SemiBold, with Noto Sans Thai as the fallback for rare Thai marks (e.g. ฤๅ).
**Money Font:** IBM Plex Mono Medium and SemiBold, with Thai falling back to Plex Sans Thai.

**Character:** A single humanist superfamily. Thai and Latin share stroke and proportions, so bilingual product names sit naturally on one line, and the mono face makes digit columns line up. All fonts are bundled (OFL) so the app renders the same offline.

### Hierarchy
- **Title** (600, 16 px; 17 touch): pane-level and dialog titles, bill name, table names on floor cards, extra-large button labels.
- **Body** (400, 13 px compact / 15 touch): default text, product names, table cells, workspace tabs (SemiBold when selected).
- **Body Strong** (600, 13 / 15): button labels and emphasised values.
- **Label** (600, 12 px compact / 13.5 touch): pane headers, field labels, pane tabs, chips, toolbar.
- **Caption** (400, 11.5 px): metadata, helper text, status bar, the toolbar "Whole bill" group label. Never below 11 px, because Thai marks need the room.
- **Money XL** (Mono 600, 30 px; 34 touch): the amount due on Sell. One per view; it drops to Money Large on short windows.
- **Money Large** (Mono 600, 22 px): change, cash received, KPIs, the extra-large button amount.
- **Money** (Mono 500, 13 compact / 16 touch): line amounts, table cells, tile prices (600 on tiles and table cards). Always right-aligned in columns.
- **Money Small** (Mono 500, 11.5 px): unit prices, pane-header counts, shortcut key hints, the status-bar clock.

egui lays out lines from font metrics, so the line heights in the tokens are honoured through row heights and spacing rather than set on each run. Any style with weight 550 or more renders in the SemiBold family.

### Named Rules
**The Whole Baht Rule.** Money is always IBM Plex Mono, shows every digit with two decimals (`1,019.00`), and is right-aligned. Never abbreviate, truncate or round it for display.

**The Density Type Rule.** Only title (16→17), body (13→15), label (12→13.5), money (13→16) and money XL (30→34) grow with Touch density. Caption, money large and money small stay fixed.

## Layout

**Spacing.** A 4 pt grid (s1 4 through s8 32). Widgets space by 8 px; panes pad 12–16 px; dialogs pad 24 px; field groups separate by 24 px; panes sit `pane-gap` 6 px apart and 6 px from the window edges in Soft, and 1 px apart (splitters) with no outer margin in Square.

**Density metrics.**

| | Compact | Touch |
|---|---|---|
| Title bar / toolbar / status bar | 40 / 40 / 24 | 48 / 58 / 32 |
| Controls sm / md / lg | 26 / 30 / 44 | 44 / 44 / 62 |
| Row height | 30 | 52 |
| Icon | 16 | 20 |
| Cart pane | 392 (360 under 1100) | 430 (360 under 1100) |
| Category pane | 168 | none (chips) |
| Product tile minimum | 150 × 78 | 172 × 104 |
| Pane header | 28 | 38 |
| Shortcut hints | shown | hidden |

The minimum hit area in Touch is 44 px.

**Window chrome** (`chrome.rs`, `design/docs/components/Navigation.md`). Top to bottom: title bar → page toolbar → panes on the canvas → status bar.
- **Title bar** (chrome): workspace tabs Home, Sell, History, Shift, Products, Reports, Settings (`Alt 1–7`; Activity opens from Reports and highlights the Reports tab, Tables is a pane of Sell and highlights Sell; `Alt 8` / `Alt 9` still reach them). The selected tab has the chrome-hover fill, SemiBold label, a 2 px accent underline and, in Compact, its `Alt N` hint. Right side: shop name (plus "Demo data"), the command field (`Ctrl K`) and a busy spinner. The OS title bar and window controls are kept (the spec's fallback); tabs are drawn in the app's own top strip.
- **Toolbar** (chrome-raised): only the current page's actions as icon + label (+ shortcut hint) tools, grouped by chrome separators. Sell: New bill `Ctrl N` | caption "Whole bill" then Hold `F4`, Bill discount `F8`, Split bill, Send to kitchen `F10`; a chrome segmented control at the right switches Products / Tables. Home: New bill, Count & close, Back up. Products, History, Shift (tabs and close steps) and Reports (period, quick periods, export, Activity) supply their own toolbars; Settings has Back up; Activity has Back to reports.
- **Status bar** (chrome, Caption): every item is clickable with a hover fill. Left: signed-in staff (avatar, menu with Lock / switch user `Ctrl L`), Offline (hollow dot), shift status (success dot, or warning "No open shift"; opens Shift), drawer expected cash in Money Small (opens Shift), printer (ready / failed / none; opens Settings → Printing). Right: clock, Touch / Mouse switch, language, Rounded / Square switch, and Accent (accent dot) which opens the display popover. Narrow windows keep every item with short labels.

**Panes** (`w::pane_uis` / `pane_at`). A pane is a surface rectangle with its own header strip (28 px, 38 touch) ruled off by a hairline, then a body that scrolls on its own. Headers carry `pane_title` (Label; ink in Soft, muted in Square), `pane_count` (right-aligned Money Small) or `pane_tabs` (Soft: pills with a sunken selected fill; Square: flat cells split by borders). Commit buttons stay pinned to the bottom of their pane.

**Sell.** Three panes: bill (cart width) | categories (168, with colour dots and counts) | products (search and tile grid). In Touch the category pane is replaced by a row of category chips with dots above the tiles. The grid takes as many columns as the tile minimum allows, and each row grows to its tallest tile (plus a picture band when pictures are on). The bill pane holds Current/Held tabs, the bill name, cart lines and a pinned footer: summary rows, a rule, Amount due in Money XL, then step one, a full-width primary **Pay** (`F9`) (beside a secondary Send to kitchen when tables exist). Pay replaces the lines with the **payment panel** (step two): a ghost Back to bill (`Esc`), method, received, quick cash and change, and an XL primary confirm button whose label states what is still missing when it cannot run ("Enter cash received", "Not enough cash", "Enter every part") and the action when it can ("Confirm cash", "Show transfer QR", "Review card payment"). With an empty bill the Pay button shows disabled.

**Home.** Three full-height columns: (1) the shift card drawn on chrome-hover with duration, float and the next step, above the Today pane (net sales first, never drawer cash); (2) To do: raised task cards with a status-coloured left stripe, title, detail and one action; (3) Held bills & open tables.

**Management.** Products: side nav (176 px, 208 at 1100 and wider) with views and categories, then the table, plus the editor docked at 1180 and wider; actions live in the toolbar. History: receipts with a 400 px detail pane at 1180 and wider. Reports: toolbar period and export over KPI stat tiles, chart and table cards. Shift: toolbar tabs and close steps. On narrower windows inspectors open as sheets over the scrim.

**Settings.** A nav pane (210 px, 260 at 1100 and wider) with SideNav rows (a warning dot marks sections with unsaved drafts) beside a content pane titled with the section; its column is centred and capped at 760 px and holds raised cards (raised fill, subtle border, lg radius, 16 px padding, 12 px apart). Checkbox groups inside cards use equal-width columns (1–3 by width) rather than `horizontal_wrapped`, which squeezes wrapping labels to one letter per line. Display holds Language, Accent colour (swatch picker with Thai/English names), Corners (Rounded / Square) and density.

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
| `F8` | Bill discount |
| `F10` | Send to kitchen (when tables exist) |
| `F9` | Pay, then confirm (in the payment panel, F9 first focuses the cash field) |
| `Ctrl N` | New bill (Sell, Home) |
| `Ctrl K` | Command palette |
| `Alt 1–7` | Switch workspace tab (`Alt 8` Activity, `Alt 9` Tables) |
| `Ctrl Z` | Undo last undoable action |
| `Ctrl S` | Save in the product editor |
| `Ctrl L` | Lock / switch user |
| `F11` | Full screen |
| `Esc` | Back to bill / clear search / close dialog, sheet or popover |

### Named Rules
**The Both Hands Rule.** Every action has a visible control and a key. In Compact, buttons, tools and the selected tab show their key hint (`F9`, `F2`, `Alt 2`) as a bordered mono chip or muted hint; in Touch the hint is hidden but the key still works.

**The Scope Rule.** Toolbar actions apply to the whole page or bill; actions on one line live in the line editor under that line, never in the toolbar.

## Elevation & Depth

Depth comes mainly from the chrome/paper contrast: paper panes lie flat on the graphite canvas with no shadow, separated by the 6 px gap (Soft) or a 1 px splitter (Square). Inside a pane, sunken wells recess and raised cards/tiles lift by fill and hairline, not shadow. Only floating layers cast soft ambient shadows: menus, popovers and toasts at level 2; dialogs and sheets at level 3 over the scrim.

### Shadow Vocabulary
- **Shadow 1** (`0 1px 6px`, black 5.5 %): the print-preview paper card; panes do not use it.
- **Shadow 2** (`0 4px 16px`, black 16 %): menus, popovers, the display popover, toasts.
- **Shadow 3** (`0 16px 48px`, black 27 %): dialogs and sheets.

### Named Rules
**The Flat Pane Rule.** Panes never get a shadow or outer border; the canvas around them is the separation.

## Shapes

**Soft** (default): xs 4 for key hints; sm 6 for small chips and picture bands; md 8 for buttons, inputs, rows, tabs and toolbar tools; lg 10 for panes, tiles, cards and table cards; xl 14 for dialogs; full for chips, pills, dots and avatars. **Square**: every radius is capped at 3 px (`Theme::radius`), panes have 0 radius, pills and dots stay round, workspace tabs fill the bar height, and pane headers get a border-subtle band. Borders are 1 px hairlines. The 2 px outline is reserved for selection (tiles in the bill, the current table) and the focus ring. Product tiles and table cards carry a 3–4 px coloured stripe inside their left edge (category colour on tiles, status colour on seated tables).

## Components

### Buttons
- **Shape:** md radius (3 in Square). Heights follow the density controls (Sm, Md, Lg; Xl uses the lg height with a Title label), never below 44 in Touch.
- **Primary:** accent fill, on-accent text. It may carry the full amount in the money font and an `F9` hint chip. Used once per view.
- **Secondary:** raised fill with a 1 px control border. This is the default.
- **Ghost:** transparent, hovering to sunken. Used for row actions and Back.
- **Danger:** danger-solid fill with on-danger text, for irreversible actions confirmed in a dialog. **Danger quiet** uses danger text with a hairline border.
- **Disabled:** primary and danger turn grey (sunken fill, subtle text), never a faded accent, and the label says why when the reason is known. Other kinds fade.
- **States:** hover and pressed step through the palette; loading swaps the content for a spinner; the focus ring appears for keyboard focus only.
- **Icon button:** square, icon-only, always with an accessible label and tooltip.
- **Toolbar tool:** icon + label (+ hint) in chrome colours, chrome-hover on hover.

### Chips, Pills & Badges
- **Chip:** full-radius pill with a control border on raised; selected fills with inverse. Quick-cash chips share the row width equally and use the money font.
- **Category chip (Touch Sell):** pill with the category dot; selected uses chrome-raised with on-chrome text.
- **Badge:** 1–3 words of status on the tone's bg/fg pair, xs radius (On sale, Out · 0, Not sent 2, In kitchen). Never a button.
- **Count badge:** inverse bubble on tabs and on tiles already in the bill.

### Cards / Containers
- **Pane:** surface, lg radius in Soft, header strip, own scrolling.
- **Card:** raised fill, subtle border, lg radius, 16 px padding (Settings sections, Home task cards).
- **Sunken well:** md radius, sunken fill, 12 px padding: order summary, payment block.
- **Stat tile:** label, Money Large value, and a muted line explaining the number.

### Inputs / Fields
- **Text input:** raised fill, 1 px control border, md radius, control height. The label sits above in Label/muted; inline error text is danger.
- **Money input:** right-aligned mono. It selects everything on focus, formats as `1,500.00` on blur and never rounds silently.
- **Sell search:** search icon at left and `F2` hint at right. It keeps focus all day for barcode scanners, so focus shows as a firm 1.5 px border-selected outline, not the blue ring. `Esc` clears it.
- **Segmented:** 2–5 options with immediate effect (payment method, language, view). The chrome variant sits at the right of the toolbar.
- **Accent picker:** five swatches with the preset names.
- **Switch / Checkbox:** drawn to spec; the checkbox supports a mixed state for table headers.

### Navigation
- **Workspace tabs:** see Layout → Window chrome.
- **Command palette (`Ctrl K`):** raised overlay with a search field and keyboard-navigable entries for workspaces, actions and products.
- **Side nav:** rows with optional colour dots and right-aligned counts; the active row uses the selected fill (plus a 2 px bar in Square). Groups ("Views", "Manage") are headed by a muted SemiBold caption.
- **Pane tabs:** in the pane header, optional counts (`Held · 2`).
- **Steps:** a 3–5 stage indicator for shift closing.

### Product tile
Raised fill, lg radius, 1 px subtle border (default border in Square); padding 12 × 10 Compact, 12 × 12 Touch; a 4 px (3 px Square) category stripe inside the left edge. The name wraps to three lines (Body) at top left; stock or status at bottom left; the price (Money, 600) at bottom right. When product pictures are on (`product-images.txt`), rows with a picture get a picture band above the names. A tile already in the bill gets the 2 px border-selected outline and an inverse count badge. Unavailable tiles sink to the sunken fill with muted text, a dimmed stripe and a danger status label. While pressed, the tile shrinks by 1.5 px.

### Cart line
A quantity box, the name with accent-ink options and note lines beneath, and the right-aligned line amount, separated by hairlines. The current line gets the selected fill (plus a 2 px bar in Square) and reveals its stepper (−, quantity, +), note (`F6`) and remove controls. A newly added line flashes accent-soft for 0.6 s.

### Feedback
- **Toast:** inverse card at the bottom centre, Body text, an optional **Undo** action (also `Ctrl Z`) and a close control. Shadow 2, max 560 px.
- **Banner:** a persistent problem at the top of the area it concerns, in its status tone, with an optional action.
- **State view:** icon, title and body for empty, no-results and error states inside a pane.
- **Dialog:** blocking, raised, xl radius, 24 px padding, shadow 3 over the scrim. The safe action sits on the left and the commit on the right. `Esc` or a scrim click closes it.
- **Sheet:** the same content as an inspector, presented over the scrim on narrow windows.

### Icons
A built-in line icon set painted as vectors (no icon font, no emoji), sized to the density icon metric. Icons always accompany a text label or an accessible tooltip.

## Do's and Don'ts

### Do:
- **Do** use exactly one accent primary per view, for the commit or money-moving action.
- **Do** read colours and sizes from `Theme` (palette, metrics, space, radius via `Theme::radius`, type_scale). Change values in `design/tokens/` and regenerate.
- **Do** draw new panes with `w::pane_uis` / `pane_at` and their header helpers, and put page actions in the page toolbar.
- **Do** set every amount in IBM Plex Mono, in full with two decimals, right-aligned in columns.
- **Do** give every action both a visible control and a shortcut, and show the hint in Compact.
- **Do** check every screen in Soft and Square, Compact and Touch, Thai and English, at 1000 × 700 and 1280 × 820 (accent presets at least once for new accent uses).
- **Do** keep keyboard focus visible with the blue ring, except for the always-focused sell search, which uses its firm border.
- **Do** make a disabled commit button grey and say what is missing.
- **Do** prefer undo (toast plus `Ctrl Z`) over confirmation. Confirm only irreversible actions, and state the effect in numbers.

### Don't:
- **Don't** use the solid accent for navigation (other than the tab underline), decoration or a second button in the same view. Charts use it only for series 1.
- **Don't** fade an accent or danger button to show it is disabled.
- **Don't** use category colours for status or status colours for categories.
- **Don't** give panes shadows or outer borders, or hard-code radii (Square must be able to cap them).
- **Don't** use subtle text for information, or set any text below 11 px.
- **Don't** convey status by colour alone.
- **Don't** add multi-branch or the tablet layout until they are designed in `design/` first.

## Implementation notes and intentional deviations from `design/`

- **OS title bar kept:** the design's unified title bar (custom window controls, drag) is not built; the native title bar stays and the workspace tabs sit in the app's first strip. This is the spec's documented fallback.
- **Single theme:** there is no appearance setting; egui runs its light visuals with the v0.2 palette. Accent (`accent.txt`), shape (`shape.txt`) and density (`density.txt`) are per-device files beside the database; backups and recovery do not carry them.
- **Offline status:** the app is offline-first and shows a hollow-dot "Offline · data on this device" status item rather than a sync status component.
- **Printing is not a step:** payment completes without a print stage. With a printer set up, receipts print automatically (or via Print receipt on the paid screen); a failed print shows a warning banner with Print again and a danger printer dot in the status bar, and never affects the sale.
- **PromptPay QR and discounts:** built from the existing dialog, segmented, chip and kv-row primitives. The QR is dark-on-white with a quiet zone; discount rows use the danger value colour with a U+2212 minus.
- **Search focus:** the sell search keeps focus with a firm border instead of the focus ring (see Inputs).
- **Accent ink:** `fg-accent` is not part of the preset swap in the generated tokens, so cart options and links stay saffron-brown under Jade/Ocean/Plum/Ink.
- **Motion:** only the 120 ms `fast` duration is wired as egui animation time, plus the 0.6 s line flash; `base` (200 ms) has no animated surfaces yet.
- **Manager approval and lock screen (built):** an action whose role access is Needs approval opens a blocking approval dialog (lock icon, "Needs a manager's approval", what and who asked, chips for members who may approve excluding the requester, PIN pad that also takes keyboard digits); the action runs only after a valid PIN and the approval covers that one action. Such buttons show a lock icon with "Needs approval"; denied actions are disabled with "Your role cannot do this". The role editor uses a three-way segmented control (Allowed / Approval / Denied) per permission in a scrolling list. With staff, a full-window lock screen (name choice + PIN pad) blocks everything until someone signs in; the status bar carries the signed-in name with Lock / switch user (`Ctrl L`).
- **VAT and tax invoices (built):** VAT rows on receipts use ordinary kv rows (value before VAT, VAT); Issue tax invoice opens a standard blocking dialog for the buyer. Dialog action rows (`w::dialog_actions`) have a bounded height so a right-to-left action row cannot stretch a modal.
- **Service charge, VAT on top and cash rounding (built):** extra kv rows in the order summary footer, collapsing into one caption line on short or compact windows so the pay button stays pinned.
- **Split payment, split bill and product pictures (built):** split payment is part of the payment panel with its own review dialog; split bill is a toolbar action; pictures are an opt-in tile band. Chart series 3–4 have no series yet.

## Recovery, import, activity and cost surfaces (2026-10-13)

The Data & backup card uses secondary actions, labeled folder-path input and an inline summary before a blocking danger confirmation. A prepared recovery replaces the shell with a stop/restart instruction and safety-backup path. CSV import uses the existing dialog, a bounded scrolling preview/error body and pinned secondary confirm/cancel actions. Activity is workspace eight: flat filters, pagination and divided event rows with expandable snapshots; money uses existing mono/right-aligned formatting. Product cost is an optional MoneyField; gross profit is a collapsible report section with explicit unknown-cost coverage. No new palette, metrics or typography tokens.

## Tables and kitchen surfaces (2026-10-13)

`design/` lists TableCard and KitchenTicket as Future exploration without full specs; they are now built from existing primitives and tokens (no new palette, metrics or typography tokens), so treat this section as the implemented reference until `design/` specifies them.

- **Tables floor (Sell → Tables, `Alt 9`):** once tables exist, a chrome segmented control at the right of the Sell toolbar switches the products area between Products and Tables; the floor replaces the categories and products panes while the bill pane stays on the left. Zones as body-strong headings over a grid of table cards that share the row evenly (min 152 px, 176 px Touch). A free table is a dashed border-control outline (hover fill only) with the name in Title, seats and "Free". A seated table is a raised paper card (lg radius, subtle border, hover fill) with a 3 px status stripe inside its left edge (warning when dishes are unsent, otherwise info), the name in Title, guests/seats, "Open · N min" since the first ticket, the bill total in Money 600 (priced exactly like checkout) and a status badge ("Not sent N" warning, "In kitchen" info). The table whose bill is open gets the 2 px border-selected outline. Move mode shows an info banner with Cancel and fades tables that are not targets. Header: secondary Counter / takeaway and ghost Open kitchen screen · N. A table bill's caption is "Table · N guests" plus a chevron icon (no ▾ glyph: the UI font lacks it) opening a guests popover. Tapping a card loads its bill on the left and switches back to Products; sending to the kitchen, paying or holding a table bill returns to the floor. Without active tables the switch is hidden and Sell is unchanged. Table and counter bills share the two-step footer: Pay (`F9`) opens the payment panel, which warns when dishes are unsent.
- **Table bill in Sell:** the bill name is the table; the footer's secondary Send to kitchen (`F10`, also in the toolbar) sits beside Pay at 40 % of the row, labelled with new/cancel counts (full text in its tooltip) and disabled when the kitchen is up to date, so the accent Pay button stays the only primary and stays pinned.
- **Kitchen screen:** a separate OS window (egui immediate viewport) for a kitchen monitor. Ticket cards are top-aligned in a wrapping row, oldest first. The header shows table, guests and minutes waiting, turning warning (orange) from 10 minutes and danger (red) from 20, always with the number as text. Tapping a line ticks it cooked (strikethrough, muted); cancellation lines are marked in danger with a text label; "Done" bumps the ticket; "Recently done" lists bumped tickets with "Recall".
- **Settings → Tables & zones:** a card listing zones and tables with an add/edit dialog (name auto-incremented from the last table, zone field with chips for existing zones, seats field, active switch). The product editor gains a "Goes to the kitchen" switch once tables exist; the printer card gains a Kitchen tickets segmented control (Off / Receipt printer / Kitchen printer, the last with its own connection and paper).
- **Printed kitchen ticket:** large table name, ticket number, time, guests and staff, large dish lines with options, bold `** note **`, and `*** CANCEL ***` slips for cancellations.

## Print preview, templates and check bill surfaces (2026-10-13)

Printed documents are not specified in `design/`; these surfaces reuse existing primitives and tokens (no new palette, metrics or typography tokens).

- **Paper card:** any document is shown as the printer's own 1-bit render on a white paper card (primitive `PAPER_0` white fill, like the PromptPay QR's white field, with the default border and shadow level 1). It is an image of exactly what prints, scaled to the card width, never re-laid-out with UI text.
- **Print preview dialog:** standard blocking dialog (440 wide) with a scrolling paper card. Print (printer icon, primary) appears only when a printer is configured; Close is ghost. The receipt dialog and Sales history detail show "Print preview" (always) beside Print copy; the paid screen shows it too.
- **Check bill:** Sell → More → Check bill opens the same preview dialog; printing happens from there. The paper reads "Check bill · not a receipt" so it is never mistaken for proof of payment.
- **Settings → Printed documents:** a card with a segmented control (Receipt / Check bill / Kitchen ticket), controls on the left in a fixed-width column (text areas bounded so the card cannot outgrow its siblings) and the live preview on the right (280 px, 220 px under 700 px of card width). Logo: path field with Use image / Remove logo, or drop a PNG/JPEG on the window. Defaults and Discard are secondary; Save is the card's only primary and needs the Settings permission.
- **Printed receipt/check bill:** optional centred logo, shop name, header lines, title, lines at Small/Normal/Large scale, totals, optional centred PromptPay QR (~45% of paper width, quiet zone), footer. VAT receipts and tax invoices keep every legally required line.
