# POS Heart

แอป POS เดสก์ท็อปสำหรับร้านค้าทั่วไปและร้านอาหารแบบเคาน์เตอร์ ใช้ Rust + egui/eframe, ฐานข้อมูล SQLite ในเครื่องผ่าน Diesel ORM และ UI ไทย / English ตาม POS Design System v0.1 (`design/`)

**สถานะ: development foundation / ยังไม่พร้อมแทนระบบขายหน้างานจริง**

## Run

Install Rust stable (1.95+), and native build tools (macOS: Xcode Command Line Tools; Windows: Visual Studio C++ Build Tools). SQLite and the Thai font are bundled; customers do not install a database server.

```sh
cargo run -p pos-desktop --bin pos-heart
# Separate sample database, never seeds the real shop automatically:
cargo run -p pos-desktop --bin pos-heart -- --demo
# Explicit local data directory:
cargo run -p pos-desktop --bin pos-heart -- --data-dir ./data
```

The initial build downloads dependencies. The built app works without internet.

## Current flow

1. First launch: set a shop name and add products, or choose Set up later. Existing databases and Demo open Home without this step.
2. The top bar holds the workspace switcher (Home · Sell · Sales history · Shift & drawer · Products · Reports · Settings · Activity log · Tables, `Alt 1–9`), the `Ctrl K` command palette, shift/offline status and display preferences (language, Light/Dark/System, Compact/Touch density).
3. Home shows today’s sales tiles, expected drawer cash, pending work (held bills, an unfinished drawer count, backup older than 7 days) and shortcuts. Settings can make a cashier device open Sell immediately.
4. Products: categories and saved views (low stock, out of stock, not for sale), search, bulk category / stop-selling, CSV export and an inspector (sheet below 1200 points) for price, category, stock, SKU, barcode, on-sale and option groups such as sweetness or toppings with extra prices.
5. Sell: cart/payment on the **left**; categories and product tiles on the **right**. Tap a tile or scan/type a code and press Enter. Products with options open an options sheet. Quantities, notes and options stay editable per line; removing a line can be undone. Discounts in baht or percent apply to one line (`F7`) or the whole bill (`F8`), with an optional reason; the cart footer shows items total, discount, any service charge, VAT on top and cash rounding, and amount due.
6. The open bill saves itself and comes back after a restart. Hold it (`F4`) to start another; held bills reopen from the Held tab.
7. Open a shift with the counted opening float, then select Cash / Transfer / Card. Cash shows quick amounts and change; transfer/card need an explicit review of the exact amount. With a PromptPay ID in Settings, the transfer review shows a PromptPay QR carrying the exact amount. Split pays one bill with several methods (for example part cash, part transfer); Sell → More → Split the bill moves some items to a separate bill that is paid on its own.
8. Sales history lists immutable receipts with an inspector. Refund some items (choose units per line, several times until everything is returned) or void a whole untouched bill, always through the original payment method with a reason (a split sale's void refunds each part to its own method; an item refund picks one of its methods); tracked units return to stock.
9. Optional VAT: once the shop's VAT registration is entered in Settings → VAT, receipts are abbreviated tax invoices (VAT included), and Sales history can issue one full tax invoice per receipt.
10. Optional staff sign-in: once an owner is created in Settings → Staff & permissions, everyone signs in with a PIN. Each role sets every permission to allowed, needs approval or denied; by default cashiers need an approver's PIN for refunds, voids, cash in/out, product and stock changes and discounts above their limit.
11. Close a shift in steps: count notes and coins (draft autosaved), reconcile against expected drawer cash, give a reason if it does not balance, confirm.
12. With a receipt printer set up (Settings → Receipt printer), receipts print after payment or on demand (Print copy from Sales history), and closing a shift prints its summary. Print preview (receipt, history, paid screen) and Sell → More → Check bill show exactly what prints, with or without a printer; Settings → Printed documents customizes receipts, check bills and kitchen tickets.
13. Reports cover today, yesterday, 7 days, this month or a custom period (up to 366 days): gross/refund/net tiles against the previous period, sales by hour (one day) or by day (longer periods), receipts by method, top products and categories, with CSV export.
14. Products that track stock change only through Receive, Remove (with a reason) and Count in the product inspector, each kept in a stock history.
15. Optional tables: add zones and tables in Settings → Tables & zones. Sell then has Products / Tables tabs (`Alt 9`); tap a table to open its bill on the left, add dishes, then Send to kitchen (`F10`), which returns to the floor. The kitchen screen opens as a second window for a kitchen monitor (see Tables and kitchen).

Default database: OS application-data directory (macOS `~/Library/Application Support/com.pos-heart.POS Heart/`; Windows resolved through `directories::ProjectDirs`). Actual path follows that library's platform conventions. `--data-dir` gives explicit control. Demo adds a `demo/` subdirectory. Preferences live beside the DB. The history backup action reports the saved filename there.

## Project layout

```text
apps/pos-desktop/src/
  app.rs           state, worker events, current-bill autosave, layout
  shell.rs         top bar, workspace switcher, Ctrl K palette, toasts with undo
  ui/              design system: tokens.rs (from design/), theme.rs, widgets.rs, icons.rs, format.rs, qr.rs (PromptPay QR)
  home_ui.rs       today’s tiles, drawer status, pending work, shortcuts
  sale_ui.rs       cart/payment left, categories + tiles right, options sheet
  discount_ui.rs   line/bill discount dialog (baht or percent, reason)
  catalog_ui.rs    product views, table, bulk actions, inspector, option editor
  history_ui.rs    receipts table, inspector, full void/refund
  report_ui.rs     stat tiles, hourly chart, methods, top products, CSV
  shift_ui.rs      open, cash in/out, steps-based close with denomination count, history
  settings_ui.rs   first-run setup, shop/device preferences, display, backup/export
  staff_ui.rs      lock screen, name chip, approval dialog, Staff & permissions card with role editor
  tax_ui.rs        VAT settings card, tax invoice buyer dialog
  print_ui.rs      receipt printer settings card, print buttons, failure banner, on-screen print preview, check bill
  templates_ui.rs  Settings → Printed documents: receipt/check-bill/kitchen templates with live preview
  tables_ui.rs     table floor (Sell's Tables tab), Tables & zones settings card
  kitchen_ui.rs    kitchen screen (second OS window): tickets, tick, done, recall
  print/           printer.json settings, rendering to a bitmap, ESC/POS encoding, receipt/shift/check-bill/kitchen-ticket documents, template.rs (templates, logo import)
  shop.rs          versioned local profile with atomic replacement
  i18n.rs          Thai/English interface and error translations
  auto_backup.rs   opt-in maintenance, verified snapshot index and retention
  auto_lock.rs     monotonic native-input idle timer and device policy
  worker.rs        bounded command channel, session revocation; database work and CSV export off the UI thread
  preview.rs       debug-only `--preview` presets for screenshots
apps/pos-desktop/examples/seed_screens.rs   sample data folder for layout captures
crates/pos-domain/  money, product/options/cart/sale data, discounts, PromptPay payload and validation
crates/pos-core/    Diesel catalog, checkout, held carts, voids, shifts and reports
  migrations/      versioned SQLite schema (001–017)
assets/fonts/      IBM Plex Sans / Sans Thai / Mono, Noto Sans Thai fallback, OFL licenses
design/            POS Design System v0.1 (tokens, docs, reference screens)
docs/architecture.md, docs/multi-device.md (proposal, not built)
```

No web frontend, HTTP backend, Tokio runtime, Docker or database service is required. Database code is intentionally centralized rather than hidden behind speculative repository traits.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p pos-desktop --bin pos-heart
```

Layout captures (debug build; never point at a real shop folder):

```sh
cargo run -p pos-desktop --example seed_screens -- /tmp/pos-heart-shots
cargo run -p pos-desktop --bin pos-heart -- --data-dir /tmp/pos-heart-shots --page sell --window-size 1000x700 --screenshot sell.png
```

`--page` accepts home, sell, history, shift, products, reports, settings, activity or tables. Debug builds also accept `--preview NAME` (for example `sell-review`, `sell-options`, `void`, `close-confirm`, `palette`, `product`) to open a dialog or state before capturing.

CI is configured to run formatting, clippy and tests on macOS and Windows; a remote run is not claimed until the project is published and it runs. Build/package separately for each OS; signing, installers and Windows runtime testing are future work. See [validation evidence](docs/validation.md) for checks and the macOS 1280×820 runtime results; the 1000×700 minimum has scoped macOS UI evidence.

## Data correctness

Integer satang and checked arithmetic; no floating-point currency. Diesel handles typed persistence; bound raw SQL is limited to migrations, SQLite configuration, daily reports and VACUUM INTO backup. Checkout re-reads products in an IMMEDIATE transaction, refuses products that are not for sale, prices each line’s options from the catalog, recomputes line and bill discounts from those prices, computes totals, aggregates repeated item quantities, and commits stock, receipt and held-bill removal together. Stable cart IDs make retries idempotent. Receipt names, prices and option choices are snapshots. WAL + FULL durability. Schema migrations refuse unknown future versions.

## Known limitations

Single shop/register. Cash, manual transfer (optionally with a PromptPay QR) and manual card records; one method per bill, or a split payment across them. The app never checks that a transfer arrived; the cashier confirms it. Refunds go through the original method only (whole bill or selected items); no exchanges or store credit. Manual discounts and automatic promotions/coupons are supported; member pricing and points are outside scope. Credit notes are issued for refunds/voids of full tax invoices; no VAT return filing, e-Tax or other taxes, recipe stock, purchase orders or suppliers, online ordering, card terminal or other device integration (receipt printers aside) (the design system's tablet screens are marked Future and are not built; tables and the kitchen screen are built, see below). Barcodes are stored per product and looked up exactly on Enter; real scanner hardware has not been tested. Option groups: at most 10 per product, 20 choices each. Sell loads up to 500 products per category or search; Products pages 100, history 50 receipts, held list latest 100. No cloud sync or multi-device use (proposal for later: [docs/multi-device.md](docs/multi-device.md)). Database is not encrypted. Thai line breaking follows egui (spaces first), not dictionary segmentation. Before real use, verify refund procedures, backup/restore and field tests appropriate to the shop.

## Guidance

Read AGENTS.md for coding standards, PRODUCT.md for scope, DESIGN.md for UI decisions, and docs/architecture.md for transactions and dependency direction. Meaningful changes must keep these documents current. No license for project code has been selected yet; bundled fonts retain their own license.

## Voids and daily summary

Sales history → Void / refund requires a reason and confirmation after returning the original sale total through its original payment method. This records the refund; it does not transfer money or reverse a card charge. Original receipts remain visible with VOIDED status and the reason/time/refund. Originally tracked units return once; untracked food does not. Old receipts prompt for explicit stock review because their original stock policy was never recorded. Archived products still receive returns; changed tracking must be corrected before returning originally tracked units.

Daily summary uses YYYY-MM-DD in Bangkok UTC+7. Sales belong to payment day, refunds to cancellation day; a refund of yesterday’s sale appears today and can produce negative net. Product net units and amounts use the same event dates. These totals exclude opening float and other drawer movements.

## Appearance and input

Choose language, Light/Dark/System and Compact/Touch density from the sliders button in the top bar or in Settings; they apply immediately and are saved as `language.txt`, `theme.txt` and `density.txt` next to the local database, independently of sales. Missing or unrecognized preferences use Thai, System and Compact. Colours, sizes and type come from the design-system tokens (warm stone neutrals, one saffron accent for money-moving actions, IBM Plex Sans Thai/Sans for text and IBM Plex Mono for every amount).

The counter layout keeps the cart and payment on the left and product tiles on the right; totals and the pay button stay pinned while lines scroll. Sell keys: `F2` or `/` search (kept focused for scanners), `Enter` add the matching code or first result, `↑ ↓` select a line, `+ −` quantity, `F3` type quantity, `F6` note/options, `F7` line discount, `F8` bill discount, `Del` remove (undo with `Ctrl Z`), `F4` hold, `F10` send to kitchen (with tables), `F9` cash field then take payment. Everywhere: `Ctrl K` command palette, `Ctrl L` lock / switch user (with staff), `Alt 1–9` workspaces, `Ctrl S` save a product, `Esc` closes overlays. Touch density raises controls to 48–64 points and hides shortcut hints (the keys still work). Irreversible or money-moving decisions use blocking dialogs; reversible ones show an undo toast.

The window is freely resizable but never smaller than 1000×700 points, the size every layout is designed for. If the screen cannot fit that (for example 1366×768 at 125% Windows scaling), the app shrinks its UI zoom to fit (down to 60%) and maximizes instead of cutting off the pay button; a zoom you choose yourself with Ctrl +/− is left alone. Home, Shift, Reports and Settings stay centred at most 1440 points wide; Sell, Products and History use the full width. Full screen (F11, the display menu or Settings) is a per-device preference saved as `window.txt`. For reproducible native layout checks, launch with `--window-size 1000x700` (minimum) or `--window-size 1280x820` (default). Sizes are content-area logical points. Physical touch hardware and Windows runtime still need testing.

## Shop setup and Home

Home is the default start page. A genuinely new data directory shows skippable setup first; Save & add products opens Products, and Set up later saves defaults and opens Home. Existing databases and Demo bypass setup. Every shop sells products and food in the same bill; there is no shop type to choose.

Settings edits the shop name and Open directly to Sell. Save commits those edits; Discard changes resets the draft. Language/appearance/density save immediately. Draft shop and product edits survive page navigation, and closing the window warns before discarding them. The open bill is saved automatically, so closing never loses it. macOS native Quit / Cmd+Q bypasses the window-close prompt; save shop and product edits before using Quit.

`shop.json` next to the database holds version-1 shop identity, setup completion, startup preference an optional PromptPay ID (omitted when empty, so profiles without it are unchanged; app builds older than this field cannot read a profile that has one). Its `kind` field (a former descriptive shop type) is no longer shown but is kept so existing profiles and older app versions still read the file. Print templates are stored as `templates` (omitted while default), with the receipt logo in `receipt-logo.png` beside it. Writes use a synced temporary file followed by atomic replacement. Malformed or future profiles are refused without overwrite, and Settings becomes read-only until the file is corrected. SQLite backups contain business records, not these local preferences or language/theme files. `printer.json` beside it holds this device's receipt printer (see Receipt printing) and is also outside backups. Settings displays the data directory and recovery backup/restore controls (see Recovery backups below).

Home’s today totals are independent of the report’s selected date. It refreshes on entry and once per minute while Home is visible; sales/refunds also refresh the cached data. Opening a held bill while another bill is open keeps both (the open one becomes held). Home separately shows the active shift and expected drawer cash.

## Shifts and cash drawer

Shift & drawer lets an operator enter the opening float (zero is valid), record positive cash in/out with a reason, and close in three steps: count each note and coin (drafts autosave locally per shift), reconcile, then confirm. A count that differs from expected needs a reason. Cash movement and closing each require a blocking confirmation. Hold or clear the current unpaid cart before closing. Held bills can carry into a later shift; only payment moves cash.

New sales and refunds of every method require an open shift. Expected cash = opening float + cash sale totals after change − cash refunds paid in this shift + cash in − cash out. Transfer/card amounts are excluded from the drawer and shown separately. Daily sales reports remain separate. A refund of an older receipt belongs to the currently open shift, even when the original sale predates shift tracking. Old receipts remain unassigned and unchanged.

Closing stores expected, counted, signed difference (counted − expected), the reason and the denomination breakdown. Negative means shortage; positive means overage. Closed shifts and movements are read-only. View shift history (numbered in opening order) and its cash records with 50-row pagination. Negative expected cash is allowed for recorded outflows/refunds and remains visible for investigation. Closing totals cannot be silently confirmed from a stale revision. Database backups include shifts and cash records; local profile/theme/language remain separate.


## Payment methods

Sell → payment selects Cash / Transfer / Card. Cash retains tender, quick amounts and change (`F9`). Transfer/card show a blocking review with the exact amount and explicit **Confirm payment received** action. Verify your bank/terminal yourself; the app works offline and cannot verify, charge or refund externally. A changed product total refuses confirmation and asks for a new review. New bills reset to Cash.

Receipts and Sales history identify the method. Daily reports show sales, refunds and net per method on Bangkok activity dates. Shifts retain separate transfer/card sales/refunds and label each ledger entry; only cash affects expected/count discrepancy. Full refunds require the original method and operator confirmation after returning funds externally.

Split (แบ่งจ่าย) is the fourth payment option: enter the transfer and/or card amounts and cash pays the rest, with cash handed over, change, quick amounts for the cash part and an equal-shares guide for 2–4 people. Review split payment lists each part (with a PromptPay QR for exactly the transfer part when an ID is set) before confirming. A split bill is never cash-rounded; the parts must add up to the exact amount due. Only the cash part goes to the drawer; reports and shifts count each part under its method; receipts print each part plus cash received and change. Voiding a split sale refunds every part to its own method; item refunds choose one of the sale's methods, up to what that method paid. Sell → More → Split the bill (แยกบิล, bills with two or more units) moves chosen units to a new bill that opens for payment while the rest stays held or on its table; dishes already sent to the kitchen are neither re-sent nor cancelled. No schema change. Schema 004 preserves all old receipt/void JSON and closed shifts, defaulting absent method fields to Cash. Back up before upgrading production data.

## Catalog views and product options

Schema 005 adds a scanner barcode (unique when present) separate from the shop's product code, and an **On sale** switch distinct from removing a product. Not-for-sale products stay listed, show a disabled tile in Sell and refuse checkout. Products show low stock at 5 units or fewer. Bulk actions change category or stop/resume selling for all selected products at once (all or nothing), with undo for stop/resume. Export writes every product, or the selected day's report, as UTF-8 CSV into the data folder without overwriting an existing file; cells that look like spreadsheet formulas are neutralised.

Schema 006 adds option groups per product (for example sweetness: required, pick one; extras: optional, pick several). A choice may add a price per unit. Checkout prices options from the catalog in the same transaction and snapshots option names and prices on the receipt; if a held bill's options no longer exist, payment is refused until they are chosen again (`F6`). Existing products start with no options; existing receipts are not rewritten. Back up before upgrading production data.

## Stock history, period reports and partial refunds

Schema 007 adds `stock_movements`. Starting to track a product records its starting stock; afterwards the inspector shows the current count and Receive / Remove / Count actions with a recent history. Remove needs a reason (damaged, expired, used in shop, lost). A count is refused if stock changed after the count started, so a sale during counting is never overwritten. Saving a product edit keeps the stored stock.

Reports accept a period of up to 366 Bangkok days. A single day compares with the same weekday last week; longer periods compare with the equally long period before. Categories use each product's current category. CSV export includes per-day and per-category rows.

Schema 008 adds `sale_refunds`. Refund items picks units per receipt line (up to what is left), shows the refund amount, requires a reason and records it in the current shift through the original method; tracked units return to stock. Sales history marks receipts as part refunded or refunded. A bill with a partial refund cannot also be voided whole. Back up before upgrading production data.

## Discounts and PromptPay QR

A discount is a baht amount or a percentage (rounded half up to the satang) on one line or on the whole bill, with an optional shared reason (up to 240 characters). Line discounts apply first; the bill discount applies to what remains and is spread across lines in proportion, so each receipt line knows what was actually paid for it. A discount larger than the price, or an invalid percentage, refuses payment until it is fixed; a fully discounted (free) bill is allowed. Checkout recomputes every discount from catalog prices in the same transaction, and a transfer/card review must match the discounted total. Receipts keep the gross line amounts, line and bill discounts and the reason; reports count sales at the amount paid and show discounts given separately (also in CSV). Partial refunds return each unit's share of what was paid, and all refunds of a line add up exactly to it. Held bills keep their discounts. Older receipts read as undiscounted; no schema change was needed.

Settings → Shop accepts a PromptPay ID (10-digit mobile, 13-digit national/tax ID or 15-digit e-wallet; dashes and spaces allowed). The Transfer button then reads Show transfer QR, and the review shows a Thai QR (EMVCo, single-use with the exact amount) with the amount, shop name and masked ID. It is still recorded as a Transfer: the app works offline and cannot see the bank, so check the money arrived before pressing **Money arrived, confirm** (เงินเข้าแล้ว ยืนยัน).

## Receipt printing

Settings → Receipt printer sets up an ESC/POS thermal printer for this device only: connection (None, Network by IP or host name with optional `:port`, default 9100, or a System printer queue on macOS/Linux via CUPS), paper 58 mm or 80 mm, print receipts automatically, open the cash drawer (connected to the printer) on cash payments, and an optional footer line. Save or Discard; unsaved printer changes count as unsaved changes when closing the window. Print test page works after saving; Save preview image writes `receipt-preview-<time>.png` to the data folder so the layout can be checked without a printer. Windows currently supports network printers only.

Receipts include options, notes, unit prices, line and bill discounts, refund notes and the footer; reprints from Sales history are marked as a copy. Closed shifts have Print summary (drawer statement, counted and difference, denominations, sales by method); with a printer set up, the close button reads Close & print summary. Thai is printed as an image, so printers without Thai fonts work. Printing runs in the background, one job at a time; if it fails, a warning banner says the data is saved and offers Print again. A print failure never changes or cancels the sale. Settings live in `printer.json` (version 1) beside the database, not in backups; an unreadable or newer file is refused and not overwritten. The app only uses the network when you configure a network printer on your local network. No physical printer, cash drawer or CUPS raw queue has been tested yet.

### Printed documents, preview and check bill

Settings → Printed documents (roles with Settings allowed) has tabs Receipt / Check bill / Kitchen ticket: controls on the left, a live preview on the right of exactly what will print (the latest sale or a sample; the open or a held bill; the latest kitchen ticket or a sample). Receipt: title (replaces "Receipt" on non-VAT receipts; "not a tax invoice" still prints), header lines under the shop name (address, phone, LINE), footer lines, logo, text size Small/Normal/Large, language (app, Thai or English) and switches for bill name, staff, unit price, options, notes, SKU, cash & change and the PromptPay QR on check bills. Kitchen ticket: size (Large by default), language, options, guests and staff; notes and cancellations always print. VAT receipts and tax invoices keep every legally required line regardless. Defaults / Discard / Save. A footer set earlier in Receipt printer is used until the template has its own footer.

Logo: paste a PNG/JPEG path or drop the file on the window. It is resized to fit 268×160 dots, converted to black and white (dithered) and saved as `receipt-logo.png` beside the database; it is included in recovery backups.

Print preview (receipt dialog, Sales history detail and the paid screen) shows the document on a paper card at the receipt printer's width (80 mm when none is set), with Print when a printer is configured. Sell → More → Check bill previews a bill for the customer before payment: lines, discounts, service charge, VAT added, the amount due, a note with the cash-rounded amount and, when a PromptPay ID is set, a QR for the exact amount. It is not a receipt and records nothing. Not yet verified on printer hardware, drag-and-drop on Windows, or Windows generally.

## Staff, PINs and approvals

Staff sign-in is optional. A shop without staff works exactly as before (records stay unattributed). Settings → Staff & permissions → start staff sign-in creates the first member, who must be an Owner, and signs them in. Owners then add or edit staff: name, role, a new 4–8 digit PIN entered twice, and an active switch; there is always at least one active owner.

Roles: Owner is fixed (everything, may approve). Manager and Cashier are built in and editable; owners can add their own roles (for example a shift lead) and delete unused custom ones. For each of ten permissions (partial refund, void whole bill, discount above the role's limit, cash in/out, close shift, products, stock, reports, settings for shop/pricing/VAT/printer/backups, and manage staff, which is owner only) a role is set to Allowed, Needs approval or Denied. A role also has "May approve others" and "Discount without approval %" (cashier default 10%, carried over from the earlier shop-wide limit). Changes take effect immediately. Default cashier: refunds, voids, discounts above 10%, cash in/out, products and stock need approval; closing a shift and reports are allowed; settings and staff are denied. Managers may do everything except manage staff.

While any active staff exist, the app opens on a lock screen: choose your name and enter your PIN. The top bar shows who is signed in with Lock / switch user (`Ctrl L`). When an action needs approval, a "Needs a manager's approval" dialog lists the other active members whose role may approve that permission and asks one of them for their PIN; the approval covers that one action only, and nobody can approve their own request. Denied actions are disabled ("Your role cannot do this") and cannot be approved. Five wrong PINs lock sign-in and approvals for 30 seconds. Inactive staff cannot sign in.

Receipts, voids, refunds, cash records, stock changes and shifts record who acted (and who approved) as snapshots that are never rewritten; receipts and printed receipts show the staff name, and shifts show opened/closed by. Reports has a Staff tab: sales by who took payment, refunds by who processed them.

A PIN identifies who is at the counter; it does not encrypt or protect the database file. A restart opens locked. Settings can enable device-local idle locking (Off by default; 1, 5, 10, 15 or 30 minutes). Business drafts and bills survive locking; payment confirmations need a new review after sign-in. An owner can set a new PIN; there is no separate PIN recovery service. Schema 9 adds the staff tables and schema 12 the roles table; older app builds cannot open an upgraded database. Back up before upgrading production data.

## VAT and tax invoices

VAT is off until someone whose role allows Settings fills Settings → VAT: registered switch, registered name, 13-digit tax ID (the check digit is verified), head office or 5-digit branch, address, rate (default 7%) and the Revenue Department POS number if issued. Prices include VAT by default (or VAT is added on top, see below). Each receipt paid while VAT is on snapshots the seller details, rate, value before VAT and VAT (rounded half up to the satang); changing the settings later never rewrites receipts, and receipts paid before VAT stay plain receipts. The receipt view and printout show value before VAT, VAT and "Abbreviated tax invoice · VAT included" (ใบกำกับภาษีอย่างย่อ / ใบเสร็จรับเงิน) with the seller block and POS number.

For a buyer who needs a full tax invoice, Sales history → Issue tax invoice takes the buyer's name, tax ID, head office/branch and address. Each receipt gets at most one, numbered in its own sequence (TI-000001…), and it cannot be edited afterwards; it prints automatically when a printer is set up, and Print tax invoice prints a copy. Voided or refunded receipts cannot get one afterwards. Refunding or voiding a receipt that already has one issues a credit note (ใบลดหนี้, CN-000001…, own sequence) automatically: seller and buyer, the original invoice, returned items, value per invoice, correct value, difference and its VAT, and the reason. Sales history shows a Credit note button per note that opens its print preview (Print when a printer is set up). Reports show VAT in sales, VAT in refunds and net (also in CSV).

Using a POS to issue abbreviated tax invoices requires Revenue Department approval first (ภ.พ.06). The app does not file VAT returns, and the invoice layout has not been reviewed by an accountant. Schema 10 adds `tax_invoices`; older app builds cannot open an upgraded database. Back up before upgrading production data.

## Service charge, ++ prices and cash rounding

Settings → Pricing (roles with Settings allowed) sets an optional service charge percentage and cash rounding (Off, 0.25, 0.50 or 1 baht; rounding down in the customer's favour by default, or to the nearest step). Each product has a Service charge applies switch (for example off for retail goods in a restaurant), and the bill's More menu has No service charge (takeaway). The VAT card chooses whether product prices include VAT or have it added on top ("++" prices). The bill is always priced in this order: discounts, then service charge on eligible items, then VAT on top, then cash rounding. Rounding applies to cash only; transfer and card pay the exact amount. Receipt VAT is taken from the amount actually paid. Receipts, printouts and reports show service charge, VAT added and cash rounding; refunds still return exactly what was paid. No legal cash-rounding rule is claimed: confirm service charge, VAT and rounding with your accountant. Schema 11 adds the product switch; older app builds cannot open an upgraded database. Back up before upgrading production data.

## Recovery backups, CSV import, activity and costs (2026-10-13)

- **Settings → Data & backup:** Create recovery backup writes a new `.posbackup` folder beside the database. Copy the whole folder off the device. It contains a consistent SQLite snapshot, optional `shop.json`, current-bill marker, receipt logo and drawer-count drafts, with a versioned SHA-256 manifest. Printer, display, automatic-backup and idle-lock preferences stay device-local. This format is a folder, not a ZIP file; old standalone `.sqlite3` exports are database-only snapshots and are not accepted by the recovery-folder UI.
- **Restore:** paste the backup folder path, Inspect backup, then Review recovery. Review shows shop, backup time, schema and counts. Save/discard drafts and hold/clear the current bill first. Confirmation validates/stages data and stops the database worker; close and reopen to activate. A full safety backup of current data is created automatically, and the whole original directory is retained beside the data folder. The completion screen gives the safety backup path, which can be restored through the same UI. Activation resumes an interrupted directory swap. Keep a single POS Heart instance on a data folder; do not manipulate the pending stage/journal by hand. Windows power-loss behavior remains unverified.
- **Products → Import CSV:** paste a UTF-8 CSV path, read/preview, fix every reported row, then Confirm import all. Creates new products only; SKU/barcode conflicts include archived products and are rechecked inside the write transaction. Required headers: `sku,name,price`; optional: `category,stock,barcode,on_sale,service_charge,cost`. Blank stock means untracked; blank cost means unknown; `0` is explicit zero. Booleans accept `1/0`, `true/false`, blank defaults true. Unknown/duplicate headers are refused. Limit: 2 MiB / 5,000 records. [Fictional import template](docs/products-import-example.csv). Configure options after import; CSV does not round-trip options.
- **Activity log** (workspace / Settings, `Alt 8`): search, type filters and newest-first pages of 50; product/price/cost changes, discounts, refunds, voids, stock, staff and roles. Actor/approver snapshots never include PINs/hashes. Events start with schema 14; no fabricated historical log. Uses Reports permission.
- **Products → Unit cost; Reports → Estimated gross profit:** optional standard per-unit cost is snapshotted at checkout. Profit excludes VAT, includes service and reverses original revenue/cost on refund day; unknown-cost lines appear separately. This estimates standard-cost gross profit before operating expenses, not inventory averaging. Enter an inclusive standard cost for products with options; option-specific costs are not modeled. CSV exports include cost and profit coverage. Legacy costs stay unknown and receipts are never rewritten.

Schemas 13-14 added costs and activity; the current schema is 17, including tables, credit notes and promotions below. Back up with the previous build before upgrading a real shop.

## Tables and kitchen (2026-10-13)

- **Settings → Tables & zones** (Settings permission): add or edit tables with a zone, name (the next name auto-increments), seats and an active switch. A table with an open bill cannot be switched off; names are unique per zone. Shops without tables see no change.
- **Tables** (Sell → Tables tab, `Alt 9`): the floor grouped by zone beside the bill. Each card shows free/busy, the bill total, guests, minutes since the first kitchen ticket and dishes not yet sent; the open table is outlined. Tap a table to load its bill on the left (the previous bill stays held); Counter / takeaway starts a bill without a table. Sending to the kitchen, paying or holding a table bill returns to the floor. Set guests from the "Table · N guests" caption; More → Move to another table moves the bill to a free table. Each table holds at most one open bill; Held lists counter bills only. While ordering, a table bill shows only its total, Send to kitchen and Take payment (F9), which opens cash/transfer/card (Esc goes back), so more dishes stay visible.
- **Send to kitchen (`F10`)** in Sell sends only what changed since the last send (new dishes and cancellations), then parks the table bill and returns to Tables. Clearing a bill the kitchen already has sends a cancellation and is recorded in the Activity log. Products have a Goes to the kitchen switch (off for bottled drinks or retail goods). Paying frees the table.
- **Kitchen screen** (Open kitchen screen · N on Tables): a second window for a kitchen monitor, oldest tickets first, with table, guests and minutes waiting (orange from 10, red from 20 minutes). Tap a line to tick it cooked, Done to clear the ticket, Recently done → Recall to bring one back. It never blocks the counter.
- **Kitchen tickets on paper:** Settings → Receipt printer → Kitchen tickets: Off, on the receipt printer, or on a separate kitchen printer (network or system queue, its own paper size). Tickets show a large table name, dishes, options, notes and `*** CANCEL ***` slips. Print jobs queue, so no ticket is lost while a receipt prints.

Schema is now 15; older builds cannot open an upgraded database. Back up before upgrading. Not yet verified: a real second monitor, kitchen printer hardware and Windows. Using several devices (ordering tablets, a networked kitchen display) is not built; see [docs/multi-device.md](docs/multi-device.md).

## Promotions and coupons (2026-10-14)

Settings → Promotions adds shop promotions: percent off, baht off each unit, buy X get Y free (within one bill line) and spend-over bill discounts, for everything, a category or one product, optionally only on some days, at some hours (Bangkok time, may run past midnight), between dates, with a coupon code and a maximum number of bills. They apply automatically while selling and again at payment; each line gets its single best promotion, and a cashier's own discount always comes first. More → Enter coupon adds a code to the bill. Receipts and history show promotion names; each promotion shows how many bills used it.

## Automatic backups and idle lock (development round, 2026-10-14)

Settings → Data & backup offers optional automatic recovery backups, off by default. Choose once per Bangkok day while the app is open, after a successful shift close, or both; keep 3–30 verified automatic snapshots (default 7). The daily check runs once per minute. Snapshots use the last autosaved, committed bill, not edits still waiting in the UI. Results show the latest verified backup and recoverable errors. Foreground configuration and Retry require Settings access; scheduled device maintenance continues when a cashier is signed in or the app is locked.

The version-1 device policy is `auto-backup.json`; snapshots and their index live in `automatic-backups/`. Retention prunes only indexed, valid automatic snapshots after a new verified snapshot is saved. Manual backups, damaged/unindexed folders and the recovery safety bundle are preserved. Local backups share the computer's storage: copy recovery folders to separate storage for protection from device loss.

Settings → Staff & permissions → Lock when idle is off by default and applies only with staff sign-in enabled. It uses a monotonic timer and native keyboard, mouse and touch input in both app windows; repaint and worker completion do not extend a session. Locking revokes queued actions/sign-ins from the old session, clears PIN drafts and requires a fresh sign-in. Already executing transactions may finish and their committed results remain valid. `auto-lock.json` is a version-1 device preference. Recovery bundles exclude both device policies and restoring retains the destination device's policies.

This round remains development work with synthetic data and native macOS checks. Windows runtime, physical touch, scanners, printer hardware and production operations remain unverified; see [validation evidence](docs/validation.md) for the actual checks performed.

## Ingredients and recipes (2026-10-14)

Products → Ingredients lists each ingredient with its stock in its own unit, a low-stock warning level and an optional cost per unit; Receive, Remove (with a reason), Count and History record every change. In the product editor, Recipe lists what one unit sold uses (up to three decimals) and shows the recipe cost when every ingredient has a cost (Use as cost copies it). Each sale deducts its recipes automatically; ingredient stock may go below zero and never blocks a sale. Voiding a whole bill gives the ingredients back; partial refunds do not, as the food was made. In Adjust by option, a choice can add or remove ingredients per unit (e.g. + fried egg: +1 egg; no egg: −1 egg); totals never go below zero. Reports have Promos (bills and amount per promotion; whole voids subtract) and Ingredients (used by recipes net of voids, removed by hand, cost at today's unit cost) tabs; the CSV export does not include them yet.

## Product pictures (2026-10-14)

In the product editor, Picture takes a PNG or JPG (paste its path or drop the file on the window) for a saved product; it is shrunk to 256 px and saved immediately, and Remove deletes it. Sell tiles show the picture above the name and price; Settings → Language & display → Product pictures turns them off on this device for smaller tiles. Pictures load only for products on screen.

## Categories (2026-10-14)

Products → Manage → Categories lists categories in the shop's order (also used in Sell and the Products sidebar) with their product counts. Add creates an empty category; the arrows reorder; Rename moves every product along, and renaming to an existing name merges the two after a warning; Delete moves the category's products to another category or leaves them uncategorised. A product saved or imported with a new category name adds it at the end.

## Shared option sets (2026-10-14)

Products → Manage → Option sets keeps option groups used by many products (sweetness, toppings, cup size). Edit a set once and every product using it changes; past receipts keep what was sold. Link sets in the product editor (Shared option sets chips, below the product's own options) or select products in the table and use Option set in the bulk bar to add or remove one on all of them. Deleting a set removes it from every product. Adding a required set, or removing a choice, makes open bills with those products choose again.
