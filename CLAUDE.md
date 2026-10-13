# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# POS Heart

Read and follow AGENTS.md. It is the canonical development guide (respond mainly in Thai; keep changes to the assigned scope; update docs when architecture or key flows change).
Setup and commands: README.md. Module/data decisions: docs/architecture.md. Scope: PRODUCT.md.

## Commands

Rust 1.95+, edition 2024. Workspace default member is `pos-desktop`.

```sh
cargo run -p pos-desktop --bin pos-heart                         # real local DB in OS app-data dir
cargo run -p pos-desktop --bin pos-heart -- --demo               # separate demo/ DB
cargo run -p pos-desktop --bin pos-heart -- --data-dir ./data    # explicit data dir
cargo run -p pos-desktop --bin pos-heart -- --window-size 1000x700   # layout checks (also 1280x820)

# Reproducible native captures (debug build; sample data only, never a real shop folder):
cargo run -p pos-desktop --example seed_screens -- /tmp/shots   # add --staff for sample owner/manager/cashiers + a custom shift-lead role, --vat for a fictional VAT shop + one tax invoice, --restaurant for 10% service + cash rounding, --tables for zones/tables with open table bills and kitchen tickets (combinable)
cargo run -p pos-desktop --bin pos-heart -- --data-dir /tmp/shots --page sell --preview sell-options --window-size 1280x820 --screenshot out.png
# Staff (seed with --staff): --preview approval | report-staff | staff-editor | role-editor; prefix as-cashier:<preset> or as-owner:<preset> to sign in that sample member first
# VAT (seed with --vat): --preview invoice | history-invoiced | credit-note (the seeded invoice sale has one refunded item and its credit note)
# Printing: --page settings --preview settings-printer (card + failure banner); --preview print-preview writes 58/80 mm receipt, shift-summary, tax-invoice and kitchen-ticket PNGs to DIR/print-preview
# Settings sections: --page settings --preview settings-shop | settings-pricing | settings-tables | settings-printing | settings-staff | settings-display | settings-data (other Settings presets select their section)
# Templates/preview: --preview templates | templates-bill | templates-kitchen (Settings → Printed documents with sample header/footer) | receipt-preview (latest sale) | check-bill (open bill)
# Split payment / split bill (Sell with a filled cart): --preview sell-split (split footer) | sell-split-review (review dialog, PromptPay QR for the transfer part when an ID is set) | bill-split (split-bill dialog)
# Tables (seed with --tables): --page tables (Sell with the floor tab); --preview kitchen (kitchen window embedded for capture) | table-bill | table-move (move mode) | table-pay (payment section open) | table-editor

# Required before finishing (same as CI on macOS + Windows, which adds --locked):
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Single test / single test file:
cargo test -p pos-core --test checkout insufficient_cash_rolls_back_stock_and_preserves_held_cart
cargo test -p pos-domain money
```

Integration tests live in `crates/pos-core/tests/` (activity, catalog, checkout, dining, discounts, import, payments, pricing, profit, refunds, reports, shifts, split, staff, stock, tax, void_reporting); unit tests are inline in `pos-domain` (money, options, sale, stock, discount, promptpay, pricing, staff, tax, dining) and `pos-desktop` (format, text, qr, window, worker/CSV, shop, quick cash, report, print settings/ESC-POS/render). Workspace lints forbid `unsafe` and deny `unwrap`/`expect`.

## Design system
- UI follows `design/` (POS Design System v0.1). Read `design/docs/07-egui-handoff.md` before UI work, and the matching `design/docs/components/<Name>.md` before building or changing a widget. `design/reference/` is a visual reference only — do not port its HTML/CSS/JS.
- Colours/sizes come only from `apps/pos-desktop/src/ui/` (`Theme::of(ui)`: `p` palette, `m` metrics; `space`, `radius`, `type_scale`). Never hard-code `Color32` or pixel sizes in screens. `ui/tokens.rs` is copied from `design/rust/tokens.rs` and skipped by rustfmt — regenerate it from the design system rather than hand-editing.
- Money: i64 satang; display through `ui::format` (grouped, 2 decimals, U+2212 minus) in the IBM Plex Mono family, right-aligned, never truncated.
- One saffron primary button per view (money-moving commit). Key actions keep a text label and a keyboard shortcut; icon-only actions need an accessible name and tooltip.
- Never put wrapped text in a justified layout (`ui.columns`, `Popup::menu`, `top_down_justified`): egui justifies by stretching glyph gaps, which detaches Thai vowels and tone marks. Use `w::columns` and set `Layout::top_down(Align::Min)` on menu popups that hold paragraphs. Product/bill/category names go through `ui::text::wrapped` for Thai-safe breaks.
- Check UI changes in Light + Dark, ไทย + English, Compact + Touch, at 1000×700 and 1280×820.

## Architecture in brief

- `apps/pos-desktop` (egui/eframe UI) → `crates/pos-core` (Diesel/SQLite persistence and transactions) → `crates/pos-domain` (Money, Cart, Sale, Shift, PaymentMethod, validation). Domain never imports UI or storage; no repository traits, no async runtime, no network.
- A single worker thread (`apps/pos-desktop/src/worker.rs`) owns the one SQLite connection. UI screens send typed requests over a bounded channel and render the worker's `Data` results; no DB work in paint. Worker requests a repaint on completion.
- Migrations are plain SQL in `crates/pos-core/migrations/NNN_*.sql`, applied by `database.rs` via `PRAGMA user_version` inside an IMMEDIATE transaction; unknown future versions refuse startup. `schema.rs` is hand-maintained to mirror the SQL — update both when adding a migration (next is 017).
- Writes (checkout, void, shift open/close, cash movement) are single IMMEDIATE transactions that revalidate catalog/shift state and commit receipt, stock, held-cart removal, ledger entry and shift totals together. Retry identity (cart ID / UUID) is looked up before the active-shift gate.
- Staff/permissions (schema 9, `pos-core/src/staff.rs`): `authorize` runs inside each write transaction after the retry lookup and shift gate. No active staff = unattributed, unchanged behaviour. Roles are data (schema 12 `roles` table; owner fixed in code): each permission is Allow / Approval / Deny per role. Approval gives `ApprovalRequired`, which the desktop replays as `Command::Approved` with another member's PIN (one-shot, no self-approval); Deny gives `Forbidden`, never overridable. Staff snapshots on records are never rewritten. Sessions are in memory; a PIN is not database protection.
- VAT (schema 10, `pos-core/src/tax.rs`): `Sale.tax` is snapshotted only at checkout; full tax invoices have their own number, one per sale, and are refused for voided/refunded sales. Credit notes (schema 16, `credit_notes`): every void or partial refund of a sale with a full tax invoice issues one in the same transaction (`tax::credit_note`), own CN number, `source_id` = the void/refund ID, values before VAT (per invoice / correct / difference) plus the difference's VAT.
- Receipts, voids and closed shifts are immutable JSON snapshots; legacy JSON must deserialize via defaults without being rewritten.
- `shop.json` (shop profile, PromptPay ID, print `templates`), `receipt-logo.png`, `printer.json`, `theme.txt`, `density.txt`, `language.txt`, `window.txt`, `current-bill.txt` and `drawer-count-<shift>.json` live beside the DB, are owned by the desktop app, and are excluded from DB backups (`VACUUM INTO`). Recovery folders carry the business sidecars (`shop.json`, `current-bill.txt`, drawer drafts, `receipt-logo.png`) but not printer/display files.
- The open bill autosaves through the held-cart store (background worker commands that never set `busy`); `current-bill.txt` marks it for restore. Checkout's existing held-row delete keeps paid bills consistent.
- Product options (schema 006, `options_json`) are priced only inside checkout via `pos_domain::price_with_options`; the desktop cart total and noncash review use the same function.
- Discounts (line/bill, baht/percent) live only in cart/receipt JSON (no migration) and are priced only by `pos_domain::price_sale` (checkout transaction, desktop cart, noncash review) in the fixed order line discounts → bill discount → service charge → VAT on top (++ prices) → cash rounding (cash only); `price_bill` is the discounts-only wrapper. Pricing settings are JSON in `app_settings` key `pricing` (schema 11). Partial refunds use `refund_share(line.net(), …)`; `SaleLine.total` stays gross. PromptPay ID is an optional `shop.json` field; its QR is still a manually confirmed Transfer.
- Split payment (no migration): `Pos::checkout_split` takes `SplitPayment { tenders, cash_given }`; `normalized()` merges parts per method (cash, transfer, card), drops zeros, needs ≥2 methods and cash given ≥ the cash part. Split bills are never cash-rounded and the parts must equal the exact total. The `sales` row keeps the first noncash part as `payment_method` with zero cash/change columns; the receipt JSON (`Sale.tenders`, serde default empty; `paid_by()`) is authoritative. One ledger row per part (`shifts::record_all`, sale counted once), only the cash part moves drawer expected; reports record each part by method. A split void refunds each part to its own method (`SaleVoid.tenders`, `refunded_by()`); partial refunds pick one of the sale's methods, capped at what it paid (`InvalidRefundMethod` / `RefundExceedsMethod`).
- Split bill: `Pos::split_bill` (one IMMEDIATE transaction, `pos_domain::split_cart` on the stored `sent` state) moves units and their kitchen-sent units to a new held bill without a table or guests, so nothing fires or cancels; a percent bill discount stays on both, a baht one on the original only; retry by the new bill ID returns the stored pair.

## Invariants and surface specs

For ORM, cancellation and daily reports, follow the transaction/date invariants in AGENTS.md and docs/architecture.md. Current evidence and unverified platforms: docs/validation.md.

Appearance and native layout: `design/` (source of truth), DESIGN.md (what the app implements) and .impeccable/surfaces/desktop.md. Use semantic tokens from the active palette and preserve both input methods and both densities.

Home, first-run setup and shop/device settings follow .impeccable/surfaces/startup.md. Home today totals and selected-date reports are distinct; shop metadata must not rewrite receipts.

Shift opening/closing and drawer cash follow .impeccable/surfaces/shifts.md. Preserve the transactional ledger, active-shift gate, retry identity, current-shift refund attribution and immutable reconciliation snapshot rules in AGENTS.md / docs/architecture.md. Drawer cash and daily sales reports are separate values.

Payments: .impeccable/surfaces/payments.md and the payment-method invariants in AGENTS.md / docs/architecture.md. Cash, manual transfer and manual card are distinct; only cash affects drawer expected. Preserve exact reviewed totals, original-method full refunds (each split part to its own method), legacy snapshots and atomic ledger writes. Note the `cash_movements` table also holds noncash payment audit rows (legacy name) — never sum it as physical cash.

## Recovery, imports, activity and cost

Follow AGENTS.md and the matching docs/architecture.md sections. Recovery includes business sidecars but excludes printer/display settings; stop the worker before next-launch activation and retain the original folder plus a restorable safety backup. New debug presets: `backup-review`, `backup-confirm`, `backup-pending`, `csv-import`, `csv-import-errors`, `activity`, `product-cost`, `report-profit`. Activity is workspace eight (`Alt 8`), using Reports access.

## Tables and kitchen (schema 15)

Follow the AGENTS.md "Tables and kitchen" rules and docs/architecture.md. One open bill per table (`hold()` refuses `TableNotFound` / `TableBusy`); `send_to_kitchen` diffs against the stored `sent` state in one IMMEDIATE transaction; `hold()` never rewrites `sent`; cancellations of sent dishes are audited in Activity; kitchen-screen actions are background worker commands that never block the counter. The floor is Sell's Tables tab (`Alt 9`, `--page tables`, `tables_ui::floor`; `SellState.pane`/`moving`); sending, paying or holding a table bill returns to it and Held lists counter bills only; the kitchen screen is a second OS window (`kitchen_ui.rs`). Multi-device is postponed: docs/multi-device.md.

## Print templates, preview and check bill

Follow the AGENTS.md "Print templates, preview and check bill" rules and docs/architecture.md. `print/template.rs` (templates in `shop.json`, logo import), `templates_ui.rs` (Settings → Printed documents), `print_ui.rs` (`paper()` preview of any `Doc`, `preview_dialog`, `check_bill_doc`). Templates never drop legally required VAT/tax-invoice lines; kitchen notes/cancellations always print; the check bill is not a receipt and is priced with `price_sale` like checkout.
