# Working on POS Heart

ตอบภาษาไทยเป็นหลัก เปลี่ยนเฉพาะขอบเขตที่ได้รับมอบหมาย และอัปเดตเอกสารเมื่อเปลี่ยนสถาปัตยกรรมหรือ flow สำคัญ

## Start
- Read README.md for setup and current limitations.
- Read docs/validation.md for verified behavior and outstanding platform checks.
- Before changing data, checkout, or module layout, read docs/architecture.md.
- Before changing product behavior, read PRODUCT.md. UI work also reads DESIGN.md.
- UI follows the POS Design System v0.1 in `design/` (source of truth). Read `design/docs/07-egui-handoff.md` before UI work and the matching `design/docs/components/<Name>.md` before building or changing a widget. `design/reference/` is a visual reference only — never port its HTML/CSS/JS.

## Rules
- Dependency direction: desktop → core → domain. Domain never imports UI or storage.
- Use integer satang and checked arithmetic for all money; no f32/f64 for currency.
- Checkout must validate products/stock/prices again inside its write transaction, including on-sale status and option prices.
- Option prices come from the catalog inside that transaction, never from the UI. Lines with different options or notes stay separate; stock still aggregates by product. Receipts snapshot option names and prices; unknown, stale or missing required options refuse the whole checkout.
- Sale, stock deduction, held-cart removal, receipt sequence and retry identity commit together.
- Never delete paid sales or rewrite their name/price snapshots.
- Full void: original receipt stays immutable; refund and stock return commit atomically and idempotently. Never guess legacy stock.
- Partial refunds (`refund_items`): per receipt line, priced with `refund_share(line.net(), …)` so all refunds of a line sum exactly to what was paid after discounts (legacy receipts: snapshot unit price, options included), never more than sold minus already refunded, original method only (for split sales, one of the sale's methods, never more than that method paid), to the refunding shift. Request ID is looked up before the shift gate. A sale with partial refunds cannot be voided whole, and a voided sale takes no partial refunds.
- Bills are priced only by `pos_domain::price_sale` (`price_bill` is its discounts-only wrapper): inside the checkout transaction from catalog prices and settings loaded there, and the same function for the desktop cart and noncash review. Fixed order: line discounts → bill discount → service charge on eligible lines → VAT added on top (only when prices exclude VAT) → cash rounding (cash payments only; noncash reviews equal the unrounded total). Each step is spread over lines by largest remainder so shares sum exactly and `net()` (which includes service, VAT added and rounding) is what was paid. Receipt VAT is taken from the amount actually paid. Never rewrite receipts when pricing or tax settings change. Never accept a UI-computed discount amount; a discount above the price refuses the whole checkout. `SaleLine.total` stays gross; use `net()` for paid amounts. Legacy receipts read as zero discount and are never rewritten.
- Stock changes outside sales and refunds go through `adjust_stock` (receive, remove with reason, count with the stock shown when counting started) and are logged in `stock_movements`. Product edits from the desktop set `keep_tracked_stock` so saving never overwrites stock sold meanwhile.
- Use Diesel typed queries for ordinary persistence; parameter-bound raw SQL where SQLite-specific operations or reporting justify it. Keep both on the same connection.
- Reports use Bangkok event dates: payment for sales, cancellation for refunds; negative net is valid.
- UI colours and sizes come only from `ui::Theme` (`ui/tokens.rs` Palette/Metrics, `space`, `radius`, `type_scale`); never hard-code `Color32` or pixel sizes in screens. Keep Light/Dark/System and Compact/Touch parity; persist appearance, density and language separately from business data.
- Money displays with `ui::format` (grouped, 2 decimals, `−` for negatives) in the IBM Plex Mono number font, right-aligned and never truncated. One saffron primary button per view, reserved for the money-moving commit.
- Preserve mouse/keyboard and touch access, left payment/right products, and blocking confirmations for sale/stock or unsaved data. Reversible actions (remove line, clear/hold bill, stop selling) act at once with an undo toast instead of a dialog.
- The open bill autosaves to `held_carts` (marked by `current-bill.txt` beside the database) and is restored on launch; an emptied bill discards its saved copy. Background autosave never sets `busy` or shows banners.
- Home must use today_summary, independent of the selected report date; never imply net sales is drawer cash.
- Keep shop.json versioned and separate from sales; refuse malformed/future profiles without overwriting, and do not force existing databases through setup.
- Draw icons in `ui/icons.rs` (24-unit grid, 1.8 stroke); icon-only actions need an accessible name and tooltip. Touch density keeps targets ≥ 44 points; retain visible text on critical actions.
- Staff (schema 9): `authorize` runs inside each write transaction, after the retry-identity lookup and the shift gate; never check permissions only in the UI. With no active staff, behaviour is unchanged and records stay unattributed. Approvals are one-shot for a single replayed command. Staff snapshots (`staff`, `opened_by`, `closed_by`, approver) are never rewritten; legacy JSON reads as unattributed. Always keep one active owner. Roles (schema 12) are data looked up on each write: Allow / Approval / Deny per permission; Deny is never overridable by approval; nobody approves their own request; the owner role is fixed in code and only ManageStaff (owner) edits staff and roles; built-in roles cannot be deleted and roles in use refuse deletion. Check permissions, never hard-coded role names. Never describe a PIN as protecting the database file; sessions are in memory only.
- Product pictures (schema 20): stored only through `set_product_image` (PNG signature, ≤ 512 KB, Catalog permission) in `product_images`; catalog queries never load picture bytes, and `image_rev` must change with every picture change so caches refresh. Pictures never block selling and never appear on receipts. The desktop must load pictures lazily (visible tiles and the open editor) with a bounded texture cache.
- Ingredients (schema 18): recipe use is deducted only inside the checkout transaction from the catalog recipes it reads, as one `sale` movement per ingredient tied to the sale; a void gives back exactly those movements in its transaction; partial refunds and retries deduct or return nothing more. Ingredient stock may be negative and never blocks a sale (product unit stock still does). Quantities are integer thousandths with checked arithmetic. Ingredient edits never change stock; manual receive/remove/count movements are retry-safe by request ID and need the Stock permission. Option recipes (schema 19) adjust the base recipe per chosen choice (signed amounts, rows must name a choice of the product, per-ingredient totals never below zero) only through `pos_domain::line_recipe`. The report's promotion and ingredient sections come from receipts (`Sale.promotions`) and ingredient movements in the period; never recompute them from the current catalog.
- Promotions (schema 17): defined in Settings → Promotions (Settings permission) and applied only through `pos_domain::apply_promotions`, which checkout runs inside its transaction from the stored list at payment time and the desktop runs for display. Manual line/bill discounts win; one best promotion per line; spend-over is judged after line discounts and only without a manual bill discount; coupons need their code on the bill; used-up, inactive or out-of-date/day/hour promotions never apply. Receipts snapshot promotion names and amounts; deleting a promotion never rewrites receipts; refunds do not give uses back. Promotion discounts are not the cashier's discounts (no approval, no Discount activity).
- VAT (schema 10): `SaleTax` is snapshotted only inside the checkout transaction when VAT is enabled; settings changes never rewrite receipts, and pre-VAT receipts stay non-tax receipts. Full tax invoices use their own running number, at most one per sale (same-buyer retry returns the original, a different buyer is refused). Voided or partly refunded sales cannot get a full tax invoice afterwards. Credit notes (schema 16): every void or partial refund of a sale that has a full tax invoice issues a credit note (ใบลดหนี้, s.86/10) inside the same IMMEDIATE transaction, numbered in its own sequence (CN), one per void/refund (`source_id` UNIQUE, so retries never duplicate), with seller and buyer snapshots, the original invoice number/date, returned lines, value per invoice / correct value after all notes / difference (before VAT) and the difference's VAT; never rewritten. Tax IDs (seller and buyer) must pass the Thai mod-11 check digit. Never claim the app files VAT returns or that its layout is legally reviewed.
- Keep database I/O off the UI thread; repaint on input or worker completion.
- Printing happens only after a committed sale, shift close or kitchen send, on its own thread (jobs queue, so a kitchen ticket is never dropped while another job prints), and never blocks, undoes or alters it; a failure only offers Print again. Printer settings are device-local in versioned `printer.json` (refuse unreadable/future files without overwriting; not in backups). Network printing is LAN-only and only when the user configures it.
- No network, telemetry, cloud account or auto-seeded business data by default (a user-configured LAN printer is the only network use).
- Keep errors recoverable. No unwrap/expect/panic in application paths.
- New formats/versions require explicit migration and refusal of unknown future versions.
- Do not claim Windows runtime validation from a macOS build.
- Do not commit or push unless requested. Never include real shop data.

## Verify
Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings, and cargo test --workspace.
For checkout changes, verify exact decimals, retry safety, insufficient cash/stock rollback, restart persistence and held-cart handling.
For UI changes, read .impeccable/surfaces/desktop.md and inspect the native flow at 1280x820 and 1000x700 in both palettes and both densities. Verify Thai/English, keyboard focus and confirmation behavior; distinguish runtime evidence from compilation. Reproducible captures: `cargo run -p pos-desktop --example seed_screens -- DIR` then run the debug binary with `--data-dir DIR --page NAME [--preview NAME] --screenshot OUT.png` (see docs/validation.md). `--preview` exists only in debug builds and is evidence of layout, not of a clicked-through flow.

For partial refund changes, verify per-line limits across several refunds, option-inclusive amounts, stock return, cross-shift attribution, retries after closing, void interplay and report days. For stock changes, verify retries, stale counts, removal limits and the edit race.

For void/report changes, verify duplicate/concurrent retries, rollback after audit-write failure, archived/changed stock, legacy migration, Bangkok midnight boundaries, prior-day refunds, signed amounts and backup reopening.

Startup/settings UI work also reads .impeccable/surfaces/startup.md. Verify fresh setup and skip, existing DB launch, persisted direct-to-Sell preference, draft/cart preservation, and report-date isolation.

Shift work also reads .impeccable/surfaces/shifts.md. New payments/refunds require an open shift, with a stable observed shift ID from desktop requests. Their ledger entry and checked drawer totals commit in the same transaction as receipt/void/stock. Original retry lookup precedes the shift gate. Refunds belong to the refunding shift; never assign legacy sales to fabricated historical shifts. Closed count snapshots are immutable; stale revisions must refuse closing. The desktop closes with `close_shift_with`: a counted total that differs from expected requires a reason, and a denomination breakdown must add up to the counted total. Drawer-count drafts autosave locally per shift and are deleted after closing. Keep daily net sales distinct from drawer cash. Verify cash-minus-change, cross-shift refunds, cash-movement retries, ledger-failure rollback, single-active-shift races, stale closing, signed discrepancy, clock guards, pagination and backup persistence.

Product option work verifies exact option totals, stale/missing options rollback, held bills with changed options, noncash review totals including options and receipt snapshots.

Payment work also reads .impeccable/surfaces/payments.md. One method per bill unless it is paid as a split payment (below); noncash confirmation pins the exact reviewed total and shift. Revalidate both in the write transaction. Cash tender/change are zero for noncash. Full refunds use the original method; never imply the app transfers/charges/refunds externally. Shift.sales/refunds remain cash-only; transfer/card buckets are separate, while counts/revision include every method. Manual drawer movements must be Cash. Legacy method defaults must not rewrite immutable receipt/void/closed-shift JSON. A PromptPay QR (optional ID in shop.json) is only a way to show a Transfer: it records `PaymentMethod::Transfer`, never claims the money arrived, and the cashier confirms manually. Verify method totals, stale reviews, original-method refund refusal, retries, ledger rollback, migration constraints/sequence, backup and drawer exclusion.

Split payment and split bill (no migration):
- `Pos::checkout_split` takes `SplitPayment { tenders, cash_given }`. `normalized()` merges parts per method (order cash, transfer, card), drops zero parts and refuses fewer than two methods (`InvalidSplit`); cash given must cover the cash part and is zero without one. Split bills are priced without cash rounding and the parts must equal the exact total (`PaymentTotalChanged`). The reviewed parts pin the total and the request pins the observed shift; both are revalidated in the write transaction.
- The receipt JSON is authoritative: `Sale.tenders` (serde default empty for single-method and legacy receipts; read through `paid_by()`), `Sale.cash` = cash handed over, `change`. The `sales` row stores the first noncash part as `payment_method` with zero cash/change columns to satisfy its CHECK. Cart-ID retry returns the original receipt.
- One ledger row per part (`shifts::record_all`: first row keeps the sale ID, the rest get new UUIDs; the sale counts once). Only the cash part changes drawer expected. Reports record each part under its method; cash received/change come from `Sale.cash`/`change`.
- A split void refunds every part to its own method (`SaleVoid.tenders`, `refunded_by()`, one ledger row per part; `sale_voids.refund_method` keeps the primary method). A partial refund names one of the sale's methods (`InvalidRefundMethod`) and cumulative refunds by that method never exceed what it paid (`RefundExceedsMethod`); single-method sales are unchanged.
- Split bill: `Pos::split_bill(cart, &[(line, units)], new_id, now)` is one IMMEDIATE transaction that applies `pos_domain::split_cart` to the **stored** kitchen state. Moved units take their sent units (sent first), so neither bill fires or cancels dishes. A percent bill discount stays on both bills, a baht one only on the original; the new bill has no table or guests. At least one unit must move and one stay (`InvalidBillSplit`); both bills are held; a retry with the same new ID returns the stored pair.

Verify exact part totals with service/VAT/discounts, no rounding on split bills, drawer expected from the cash part only, per-method report and shift buckets, split void and per-method refund limits, retries, legacy receipts without `tenders`, and kitchen state after splitting a sent table bill.

## Recovery, import, activity and costs

Read the corresponding docs/architecture.md sections before changing these flows.
- Restore a verified, reviewed, staged snapshot; keep a restorable safety backup and original directory, stop the worker before activation, and activate before opening SQLite. Exclude device printer/display settings from portable bundles.
- CSV import is create-only and atomic, bounded to 2 MiB/5,000 records. Recheck Catalog access and SKU/barcode conflicts (including archived products) inside its transaction; never overwrite stock through import.
- Activity commits in the recorded write's transaction; retries never duplicate events. Snapshot business fields and attribution only, never PIN/hash data. Preserve legacy history without fabricated events.
- Standard cost is optional integer satang: unknown differs from zero. Snapshot cost and allocated VAT at checkout; separate covered revenue/cost from unknown-cost revenue, reverse original values on refund day, and never infer historical cost from today's catalog.

Verify checksum/future-format refusal, all-row validation, changed reviewed sources, restart/interrupted activation, safety-backup reopening, device settings, empty-cart flushing, CSV malformed/duplicate/stale rows and rollback, activity rollback/attribution/retries, unknown/zero cost, exact VAT/refund totals, immutable legacy bytes and negative profit. Native presets are listed in CLAUDE.md.

## Tables and kitchen (schema 15)

Read docs/architecture.md "Tables and kitchen" before changing these flows.
- One open bill per table: `hold()` checks the table exists, is active and holds no other bill (`TableNotFound` / `TableBusy`). Moving a bill = holding it on another table (desktop: More → Move to another table, free tables only). The floor is Sell's Tables tab (`tables_ui::floor`), not a separate workspace. Switching off a table with an open bill is refused.
- `send_to_kitchen` is one IMMEDIATE transaction that diffs the cart against the **stored** `sent` state (never the UI copy), holds the bill with the new state and inserts the ticket together, so lost replies or stale copies never re-fire dishes. `hold()` never rewrites `sent`; an autosave queued before a send cannot roll back kitchen state.
- Cancelling dishes the kitchen already has produces a cancellation ticket and an Activity `kitchen` event in the same transaction; clearing such a bill sends a cancellation, not an undo toast.
- Kitchen screen actions (bump, recall, tick) are background worker commands that never set `busy` or block the counter. Paying frees the table; tickets stay until bumped.

Verify one bill per table, moves and switch-off refusal, only-differences sends and cancellations after retries/stale copies/autosaves, activity on cancellation, bump/recall/tick, paying freeing the table, Light/Dark, Thai/English, Compact/Touch on the Sell floor tab (including move mode), kitchen screen and table bill.

## Print templates, preview and check bill

Read docs/architecture.md "Print templates, preview and check bill" before changing these flows.
- Templates (`shop.json` `templates`, validated, omitted while default) only change presentation. VAT receipts and full tax invoices always keep every legally required line whatever the template turns off; a template title replaces only the non-VAT receipt heading, and "not a tax invoice" still prints.
- Kitchen tickets always print notes and cancellations; the kitchen template only sizes text and toggles options/guests/staff.
- The check bill is not a receipt: it says so, records nothing, and is priced exactly like checkout through `pos_domain::price_sale` (non-cash exact amount, cash-rounded amount only as a note; a PromptPay QR carries the exact amount and is still a manually confirmed Transfer).
- `receipt-logo.png` (imported, resized, 1-bit dithered) is a business sidecar: it is in recovery backups beside `shop.json`; printer settings stay device-local.
- Preview renders the same `Doc` the printer gets and works without a printer; it never prints on its own.

Verify template defaults/partial and future-safe JSON, validation limits, logo import/reload and backup, VAT lines with every toggle off, kitchen notes/cancellations, check-bill totals against checkout (service, VAT on top, discounts, rounding), and the template card/preview in Light/Dark, Thai/English, Compact/Touch at 1000×700 and 1280×820.

## Automatic backups and idle lock

Read docs/architecture.md "Automatic device backups" before changing schedules, retention or maintenance authorization, and "Idle locking and worker session revocation" before changing inactivity, locking or command/result handling.

- Automatic backup is opt-in, versioned and device-local; scheduled maintenance may bypass current staff Settings only through the persisted enabled policy. Foreground configuration/retry and recovery keep Settings authorization.
- Create and verify a new snapshot before pruning only valid indexed automatic snapshots (retention 3–30). Preserve manual, damaged/unindexed and safety backups. Snapshot committed autosaves, not UI drafts; failures never undo business commits.
- Lock by monotonic native-input inactivity (default off; 1/5/10/15/30), inactive without staff. Revoke stale queued actions/sign-ins by worker epoch; preserve drafts/cart and results of already executing transactions. Clear PIN drafts and require fresh authentication/review.
- Portable recovery excludes `auto-backup.json` and `auto-lock.json`; restoring preserves destination policies.

Verify opt-in/future-format refusal, Bangkok-day and successful-close triggers, retention ownership/failure behavior, locked/cashier maintenance, stale queue/sign-in revocation, in-flight committed results and draft preservation. Include `development_flow` combined restaurant/payment/tax/recovery regressions. Report native macOS and synthetic tests separately from unverified Windows/hardware.
