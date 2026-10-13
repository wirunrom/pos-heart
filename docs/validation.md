# Validation evidence

Development verification; this is scoped evidence, not a production-readiness claim. POS Heart is a local Rust/egui app with SQLite and no HTTP service. Initial runtime data was isolated under `/private/tmp/pos-heart-smoke/demo`; the ORM/refund/report increment used `/private/tmp/pos-heart-features/demo`; appearance/layout checks used `/private/tmp/pos-heart-design-qa/demo`.

## Automated checks

`cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets --offline -- -D warnings` and `cargo test --locked --workspace --offline` passed for the ORM/refund/report increment; 17 tests passed. Baseline tests cover exact decimal money, atomic/idempotent checkout, restart persistence, insufficient-cash rollback, combined stock validation, untracked food, immutable receipt snapshots, duplicate SKU, future schema refusal, backup reopening and archived-product rejection.

Seven additional core tests cover full refunds, archived-product returns, changed tracking and overflow, duplicate/concurrent void requests, transaction rollback after audit insert failure, unchanged legacy receipts during migration, explicit legacy stock selection, Bangkok midnight boundaries, prior-day refunds and negative net, top-ten ordering, report overflow and backup/restart persistence. Assertions live in `crates/pos-core/tests/checkout.rs`, `crates/pos-core/tests/void_reporting.rs` and domain tests.

## Native runtime and database evidence

Verified on macOS at the initial 1280×820 viewport:

- Cart/payment left and products/search right; Thai/English interface inspected.
- Completed sale: total ฿45.50, cash ฿100.00, change ฿54.50; tracked product stocks then 23 and 7.
- Held bill survived app restart. Scrolling the right pane brought its full button into view.
- Product form saved `SMOKE001`, name `สินค้าทดสอบ / Test item`, price ฿12.75; catalog refreshed successfully.
- Thai item note was checked in stored SQLite data.
- Created backup returned `ok` from SQLite `PRAGMA integrity_check`.

The ORM/refund/report increment was also exercised through the native macOS app:

- Sold two waters and one notebook: total ฿55.50, tender ฿100.00, change ฿44.50; stock became 22 and 7.
- Full void required a reason and confirmation, recorded refund ฿55.50, marked the receipt VOIDED and restored stocks to 24 and 8.
- Sold one untracked food item for ฿69.00; daily summary showed 2 sales, 1 refund, gross ฿124.50, cash ฿200.00, change ฿75.50, refunds ฿55.50 and net ฿69.00.
- An invalid date (2026-02-30) showed an error while retaining the displayed report date/results. Previous day showed empty activity; Today restored current activity. English labels and VOIDED history were inspected.
- Restarted the latest binary from the delivered project: restored stocks, both receipts, VOIDED status and daily net ฿69.00 persisted. The English refund dialog was inspected without submitting another refund.
- Backup from the UI reopened independently with `integrity_check = ok`, schema version 2, two receipts and one cancellation.

Database checks establish the stored note and backup integrity; they do not verify every input method or an operator restore workflow.

## Still unverified

- Other display scaling, physical touch hardware and very long shop data. The minimum sale layout now has scoped macOS evidence below.
- Windows build/runtime in this session. CI configuration is provided; no remote CI success is claimed.
- Installers/signing, barcode scanner behavior, receipt printers and other shop hardware.
- Recovery on Windows, actual power-loss recovery and field testing (the restore UI now has scoped macOS evidence below).

For meaningful changes, run the [README checks](../README.md#checks) and record platform, viewport and observed results. Keep unexecuted checks distinct from successful runs.

## Light/dark and counter layout increment

The same 17 tests, locked offline workspace clippy with warnings denied, formatting and native build passed after the UI changes. No business schema/query/money changes were made.

Native macOS evidence at 1280×820 content size: all four pages inspected in Light and Dark; System initially adopted the dark OS appearance. At 1000×700, Dark Thai and Light English selling views kept total, cash, change, payment and hold actions visible. Screenshots include a 32-point native titlebar in addition to content size. Product tiles, fields and quantity controls remain accessible through egui.

F2 searched R001; clearing search restored the catalog. F4 focused cash. Sold ฿45.50 for ฿100.00 and showed ฿54.50 change. Full refund restored stock and the daily statement showed matching ฿45.50 sale/refund and net zero. Held a bill, restarted, verified Light preference and held bill persisted, resumed and paid it. A refund modal blocked a background Reports click (it cancelled the dialog instead of changing pages). Physical touch behavior and Windows are not claimed from these checks.

Semantic palette text contrast was calculated against actual surfaces: minimum 4.88:1 for the checked body/muted/accent/error roles. Disabled controls retain egui's separate treatment. Native code was reviewed visually; the Impeccable HTML/CSS detector was not applicable.

The finish review found one issue: refund actions were inside the scrolling body. The final implementation keeps cash-return guidance and Back/Confirm outside that scroll area. Recaptured Light/Dark refund dialogs at both 1280×820 and 1000×700 show the entire footer; an empty reason disables Confirm, entering a reason enables it, and Back cancels without writing a second refund. The scoped reviewer verdict was resolved / ship. Clippy and all 17 tests passed after this correction.

## Home, setup, Settings and navigation icons

Scoped macOS verification used `/private/tmp/pos-heart-home-first`, `/private/tmp/pos-heart-home-complete`, and `/private/tmp/pos-heart-home-demo/demo`; no real shop data was used. Locked offline build, fmt and warnings-denied workspace clippy passed. There are now 20 passing tests: the original 17 plus local profile round-trip/invalid-update preservation, malformed/future profile refusal, and selected-report-date versus Home-today isolation.

- Fresh real data directory displayed setup. Set up later persisted defaults and opened Home. Settings -> Open directly to Sell -> Save survived restart and opened Sell.
- Another fresh directory completed English setup with Heart Counter / Food & drinks; saved version-1 shop.json and opened Products. With that profile temporarily preserved outside its normal filename, the existing database reopened Home without forcing onboarding; the profile was then restored.
- Demo bypassed setup and loaded its isolated sample products. Home showed explicit Demo/offline status.
- Held one water; started a separate notebook bill. Returning Home preserved the active bill and disabled held Resume. Completed notebook for ฿35.50 exact cash, then Home showed gross/net ฿35.50, refunds zero, one sale. Resumed the water from Home into Sell at ฿10.00 and held it again.
- Selected the previous day in Reports (zero activity), then Home still showed today's ฿35.50. This complements the worker date-isolation test.
- Settings database backup completed; the independent SQLite snapshot reported `PRAGMA integrity_check = ok`. Backup excludes shop/theme/language preferences, as explained in UI.
- Tab moved keyboard focus to the labeled Sell icon button; Enter opened Sell. Settings/Refresh icon-only controls expose accessible names and hover descriptions. No icon fonts or network assets are used.
- Light/Dark captures cover Home and Settings at 1280×820 and 1000×700 content sizes, plus Thai and English setup. The small Home held row and lower Settings sections were scrolled into view. Values and primary selling actions remained readable.
- Closing the window with a draft profile displayed the blocking unsaved modal. Keep open retained the draft; Discard changes restored the saved profile. **macOS native Quit/Cmd+Q bypasses the framework window-close prompt**; Save settings / hold bills before Quit. This path is not claimed fixed.

Screenshots include a 32-point native titlebar. The inherited semantic palettes are unchanged. HTML/CSS detector is not applicable to native egui. Physical touch hardware, Windows, other display scaling, restore UI, shifts and field readiness remain unverified/out of scope.

Fresh Impeccable finish review inspected 16 native captures and the source/brief; disposition ship for this extension, with the native Quit limitation explicitly retained. Documentation updated to distinguish window close from native Quit.


## Shift management — 2026-10-13

Source staged separately from the Desktop checkout; every native scenario used an isolated Demo database in `/private/tmp/pos-heart-shifts-qa/demo`, never actual shop data. `cargo test --workspace` passed 27 tests (6 checkout, 7 shifts, 7 void/reporting, 3 desktop/profile, 4 domain). `cargo clippy --workspace --all-targets -- -D warnings`, formatting and macOS debug build passed. Windows runtime, installers/signing and physical touch devices remain unverified.

Shift integration coverage: require an open shift without altering stock/held cart on refusal; retained sale amount excludes change; opening/movement/closing retry identities; immutable closed count and signed difference; cross-shift refunds and retries after closing; partial-unique single-active guard and concurrent payment vs closing; revision and clock checks; positive amounts/reasons; checked overflow rollback; injected ledger-write failure rolls back receipt, stock, held removal and refund; 50-row cash/shift pagination; standalone backup reopening. Version-2 migration preserves receipt JSON and creates no fictitious old shifts; existing version-1/legacy stock tests still pass with explicit fixture shifts.

Native macOS UI checked with CUA at 1280×820 and 1000×700 content sizes, Thai/Dark and English/Light. Other language/theme combinations use the same shared semantic palette. Runtime flow: open float500; cashin20.50 with Thai reason, cashout10 with Thai reason → expected510.50; sale35.50 with tender100/change64.50 → expected546; close counted545.50 → shortage-0.50. Back from close confirmation preserves the count, then Confirm records a closed shift. Closed history and its read-only statement/ledger remain inspectable. New no-shift payment action opens Shifts while preserving current cart; opening another shift keeps the cart, and closing is disabled until hold/clear. Held cart survives restart. New shift opened545.50 then refunded the previous shift's35.50 receipt → expected510; old closed statement remains546/545.50/-0.50. Home daily gross/refund35.50/35.50/net0 stays separate from drawer510. Native Tab moved from cash amount to reason. Light/Dark and language preferences persisted across restart.

Captured evidence resides in the staging QA `.impeccable/review/` directory; it is not real shop data and is not copied into the source delivery. Captures include active/open/Home, cash and close confirmations, no-shift Sell, closed statement, and paginated ledger/history layout. The second inspection batch corrected ledger monetary alignment and stacked closed-history count/difference beside View shift. The native HTML/CSS detector is not applicable. Existing macOS Cmd+Q limitation remains as documented above.

Fresh Impeccable finish review: **ship**, required 11 native captures valid, no mandatory fixes at the supplied macOS screenshot/source scope. It did not rerun tests or certify other platforms. Fresh documenter completed read-only: surface brief matches implementation; incumbent DESIGN.md and .impeccable/design.json preserved without system/token changes.

## Payment methods — 2026-10-13

All payment testing uses isolated `/private/tmp/pos-heart-payments-qa/demo`; no real shop data. Locked workspace tests pass 33 cases: prior 27 plus 6 payment integration tests. Formatting, warnings-denied Clippy and macOS debug build passed. New integration coverage includes all three methods, exact decimals and zero noncash tender/change, method-specific daily totals, original-method refunds across shifts/days, immutable closed history, retry identity after closing, stale amount and shift rollback, noncash manual-drawer refusal, ledger-trigger failure rollback for checkout/refund, noncash bucket overflow, standalone backup reopening, and real v3 migration preserving JSON bytes/receipt sequence plus constrained methods and rebuilt refund foreign keys.

Native macOS: opening cash100; transfer notebook35.50; cash water10 with tender100/change90; card food69. Gross114.50. Returned transfer35.50 with required reason: stock restored, transfer net0, overall net79, expected drawer110. Card receipt shows method and total without fake cash/change. Back from review preserves cart; held cart survives restart and resets payment to Cash on resume. Native refund dialogs identify original Card/Transfer, explain external returns, and disable confirmation without a reason. A Demo-only external price update35.50→36 after opening review was refused at submission; UI refreshed total36, preserved the cart and retained stock8, with no fourth receipt. This is a fixture simulating stale catalog state, not a bank integration.

Screenshots in staging `.impeccable/review/payments-*.png` cover Thai/Dark and English/Light at 1280×820 and 1000×700 content sizes, review/refund modals, receipts, method reporting and separate shift buckets/ledger. The initial inspection corrected wrapping, compact empty state and report column alignment. Native code skips the HTML/CSS detector. No palette/token/assets changes. Windows, physical touch, external payment processing and production rollout are unverified/out of scope. Existing native Cmd+Q limitation remains.

Fresh finish review opened all 19 native payment captures and found one material issue: English noncash guidance wrapped at 1000×700 and clipped Hold. The final correction reserves measured wrapped text height and reduces only cart scrolling. Both affected captures were replaced; the same reviewer scored the sole fix **resolved**, disposition **ship** at that scoped verdict-pass level. Native F4 focused the cash field. Back then Hold preserved the unpaid bill without a fourth receipt. This is not a fresh certification of every platform or lifecycle path.

Fresh read-only documenter compared source, PRODUCT, payment surface brief, DESIGN and sidecar: **preserve**. No token or visual-world changes needed. DESIGN.md retains older cash-oriented behavioral examples (valid-cash payment and completion change); the current payment surface brief and PRODUCT describe noncash review and method/total feedback. This documentation drift was reported without rewriting the incumbent system. Demo SQLite integrity check returned ok, foreign-key check empty, with exactly 3 receipts (transfer3550/cash1000/card6900 satang), 1 transfer refund3550 and one held bill.

## Design system v0.1 redesign — 2026-10-13

Scope: whole desktop UI moved onto the user's POS Design System v0.1 (`design/`), plus the reference screens' features (categories and saved views, barcode, on-sale switch, bulk actions, CSV export, product options with prices, held-bill tab with autosaved current bill, quick cash, steps-based shift close with denomination count and difference reason, hourly report with last-week comparison). Schema 005 and 006 added.

Automated: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (56 tests) and `cargo build --release -p pos-desktop --bin pos-heart` passed on macOS. New coverage: migration 004→005/006 keeps products on sale with no barcode/options and refuses schema 7; category/view/barcode queries and counts; duplicate and blank barcodes; not-for-sale checkout refusal with stock, held cart and shift unchanged; all-or-nothing bulk changes; closing reason/breakdown validation and storage; shift numbering; hourly gross and restocked units; option pricing and receipt snapshots (including a noncash review that must match the option total); invalid/stale options rolling back checkout; option-definition validation; money/date formatting; quick-cash amounts; CSV quoting/formula guard and one-row-per-product export without overwrite; report percentage and axis steps.

Thai font check: a temporary egui harness rendered `ปั้น กี่ ที่นี่ ญาติ ฐาน ฎีกา ฤๅษี น้ำ ผู้ใหญ่` at 12–34 points with IBM Plex Sans Thai and Noto Sans Thai. Mark placement was equivalent; Plex lacks `ๅ`, so Noto Sans Thai stays as the fallback in every family. egui still has no GPOS shaping or Thai word segmentation.

Native macOS render evidence: debug builds launched against a disposable folder from `examples/seed_screens.rs` (sample products with options, a closed and an open shift, 18 receipts with one refund, cash in/out, two held bills and a restored current bill). Captured every workspace at 1000×700 Light Thai Compact and 1280×820 Dark English Touch, Sell also at 1280×820 Dark Thai Compact and 1000×700 Light English Touch, and overlays via `--preview`: cash with quick amount, transfer review, large-cash confirm, options sheet, held tab, paid state, undo toast, command palette, display popover, void dialog, receipt inspector, product inspector and sheet (with unsaved banner and "was" price), bulk bar, close shift reconcile and confirm. The restored current bill on launch exercised the `current-bill.txt` restore path. Fixes made from these captures: stat tiles laid out horizontally, cart footer height estimate, palette list height, reversed steps, banner action clipping, chart axis crowding, missing `≡` glyph.

Not verified by interaction: these captures show states set by preview presets, not clicked-through flows. Keyboard shortcuts (F2, F3, F4, F6, F9, Del, Ctrl K, Ctrl Z, Ctrl S, Alt 1–7), scanner-style Enter lookup, autosave writes while editing, window-close flush, CSV files opened in a spreadsheet and touch input were not exercised by a person or automation in the running app; their logic is covered by code review and the core tests above. Windows, physical touch screens, real barcode scanners, other display scaling and Cmd+Q behaviour remain unverified.

Finish review (Impeccable finish reviewer, 30 captures against `design/` and the reference comps): verdict "fix first" with eight findings, all addressed and recaptured: shift-close difference, reason and commit pinned below the scrolling breakdown; Thai-safe line breaking (`ui/text.rs`: breaks at spaces and `+ / ( )`, otherwise only between Thai character clusters, never stranding a leading vowel or splitting a mark — not dictionary segmentation); Home banner no longer stretches; touch hit targets ≥ 44 points for segmented controls, chips, small and icon buttons; compact order summary and bottom fade so touch at 1000×700 still shows bill lines; shorter chart on short windows and signed refunds in receipts by method; full-width category field; gear icon spike, per-method review wording, English plurals and keyboard hints hidden in touch. Keyboard fixes from code review: F9 on the Held tab no longer leaves a pending payment, Ctrl Z undoes on Sell while the search is empty, and `+`, `−` and Option+digit no longer type into the search box. A second review pass was not run.

Window sizing (2026-10-13): unit tests cover the zoom fit (1366×768 at 125% → 0.7, 1024×768 → 0.95, large screens 1.0, floor 0.6) and the full-screen preference. On this Mac (2560×1440) a 1920×1080 capture showed Shift centred at 1440 points and `window.txt = fullscreen` launched full screen. The auto-zoom path itself has not run on a real small or scaled Windows screen.

Thai justification fix (2026-10-13): wrapped Thai paragraphs inside two-column layouts (e.g. the open-shift explanation) were justified by egui, which stretches gaps between glyphs and detached vowels and tone marks. `ui.columns` uses a justified layout; screens now use `w::columns` (non-justified) and the report date popover sets a non-justified layout. Recaptured the open-shift screen in Dark Thai at 1280×820 and 1820×700: text renders normally.

## Stock history, period reports and partial refunds — 2026-10-13

Automated: fmt, warnings-denied clippy and `cargo test --workspace` (75 tests) pass. New core tests: stock receive/remove/count history, retry and conflict, stale count after a sale, invalid changes and untracked products, product edit keeping stock sold meanwhile; period days/hours/categories and range limits; partial refunds with option prices, stock return, drawer impact, per-line limits across refunds, retries and conflicts, method and void interplay, cross-shift attribution and retry after closing, report day attribution, and migration from schema 7. Desktop tests: comparison period.

Native renders on macOS: stock count dialog and inspector stock section (Light Thai 1280×820, Dark English touch), week report with daily chart and month report with categories. Partial refund dialog (`--preview refund`) captured in Light Thai Compact at 1280×820 and 1000×700 and Dark English Touch at 1000×700; the part-refunded receipt (`--preview history-refunded`) in Light Thai at 1280×820 shows the refund frame, "Part refunded" status and only "Refund items" (no whole-sale void). Fix from these captures: in Touch at 1000×700 the dialog overflowed the window, so the item list now reserves more height in Touch and scrolls; added spacing under the title and above the refund amount. The refund itself was not submitted through the UI; its logic is covered by the core tests.

## Discounts and PromptPay QR — 2026-10-13

Automated: fmt, warnings-denied clippy and `cargo test --workspace` (90 tests) pass on macOS. New core tests in `crates/pos-core/tests/discounts.rs`: line and bill discount pricing at checkout; a noncash review must match the discounted total; oversized or invalid discounts refuse the whole sale with stock rolled back, while a fully discounted bill is allowed; partial refunds of a discounted bill sum exactly to what was paid; reports show paid amounts and discounts given; held bills keep their discounts. Domain unit tests cover percent rounding, amount above price, invalid percentages, exact bill-discount allocation, refund shares adding up to the line, PromptPay ID parsing/masking and the EMVCo payload (static/dynamic, CRC) against the open-source promptpay-qr test vector. Desktop tests cover the optional `promptpay` field in shop.json (omitted when empty, validated on save). No schema change (still 8); legacy receipts deserialize with zero discounts.

Native renders on macOS (debug build, `seed_screens` sample data, `--preview`): discount dialog (Light Thai 1280×820); discounted cart (Light Thai 1280×820 and 1000×700, Dark English Touch 1000×700 and 1280×820) — narrow carts drop the F6/F7 hint chips and then show the discount as an icon button with tooltip; receipt with line/bill discounts and reason; refund dialog with after-discount amounts (Dark English); PromptPay transfer review (Light Thai Compact 1000×700, Dark English Touch 1000×700) and Settings with a PromptPay ID (Light Thai 1280×820). The rendered QR was compared module by module with the encoder's matrix and matched exactly in Light and Dark. The QR was not scanned with a real banking app, and the discount and payment flows shown were preview states, not clicked through end to end.

Not verified: Windows runtime, physical touch, scanning by Thai banking apps, and opening a shop.json that contains a PromptPay ID with an older app build (expected to be refused, as those builds deny unknown fields).

## Receipt printing — 2026-10-13

Automated: fmt, warnings-denied clippy and `cargo test --workspace` (95 tests) pass on macOS. New desktop unit tests: ESC/POS raster band packing, printer address parsing (default port 9100), `printer.json` round trip and refusal of a future version, a print job delivered to a local TCP listener, and Thai receipt rendering at both 58 mm and 80 mm widths.

Native evidence on macOS (debug build, `seed_screens` sample data, `--preview print-preview` and `--preview settings-printer`): rendered preview images of a 58 mm receipt (Thai, options, discounts, refund note, copy mark) and an 80 mm shift summary were inspected; the Settings → Receipt printer card was captured in Light Thai at 1000×700 and Dark English Touch at 1280×1300; the print-failure banner was captured. These are rendered layouts, not prints.

Not verified: any physical printer, paper cut or cash drawer pulse, a CUPS raw queue, network printing to real hardware, and Windows (which has no system-queue path; network only).

## Staff sign-in, PINs and approvals — 2026-10-13

Automated: fmt, warnings-denied clippy and `cargo test --workspace` (102 tests) pass on macOS. New integration tests in `crates/pos-core/tests/staff.rs`: the first staff member must be an owner and shops without staff need no sign-in; sales record who rang them up and cashiers need approval (with the approver recorded); wrong PINs lock sign-in and inactive staff cannot act; shifts and reports name the staff. Domain and core unit tests cover role permissions, PIN validation, PIN hashing/verification and the rounded-up discount share. Migration 009 (schema 9) adds the roster and settings tables without rewriting existing JSON.

Native captures on macOS (debug build, `seed_screens DIR --staff` sample roster, `--preview`): lock screen in Light Thai Compact 1280×820 and Dark English Touch 1000×700; approval dialog in Light Thai Compact 1000×700 and Dark English Touch 1000×700; Reports Staff tab in Light Thai 1280×820; Settings staff roster as a cashier (read-only note). These are preview states.

Not verified: Windows; clicking through a full approve-and-replay flow in the running UI (its logic is covered by the core tests); physical touch on the PIN pad. A PIN identifies the person at the counter and does not protect the database file.

## VAT and tax invoices — 2026-10-13

Automated: fmt, warnings-denied clippy and `cargo test --workspace` (107 tests) pass on macOS. New core tests in `crates/pos-core/tests/tax.rs`: receipts get a VAT snapshot only after VAT is enabled and later settings changes do not rewrite them; full tax invoices are numbered separately, a same-buyer retry returns the original, a different buyer conflicts, pre-VAT receipts are refused (`NotTaxReceipt`) and invalid buyers are refused; refunded sales are refused (`SaleRefunded`) and reports carry VAT in sales and refunds. Domain tests cover the Thai tax ID check digit and VAT-inclusive rounding. Migration 010 (schema 10) adds `tax_invoices`.

Native evidence on macOS (debug build, `seed_screens DIR --vat` with a fictional company and buyer, `--preview`): tax invoice dialog in Light Thai Compact at 1000×700 (and a taller window); history detail of an invoiced VAT receipt in Light Thai at 1280×820; refund dialog in Dark English Touch at 1000×700 after the `w::dialog_actions` height fix; rendered 80 mm VAT receipt and full tax invoice images (`print-preview`) inspected. These are preview states and rendered images, not prints.

Not verified: legal review of the receipt and invoice layout against Revenue Code s.86/4 and s.86/6 by an accountant; Revenue Department POS approval; credit notes (not implemented); Windows; any physical printer.

## Service charge, ++ prices and cash rounding — 2026-10-13

Automated: fmt, warnings-denied clippy and `cargo test --workspace` (113 tests) pass on macOS. New core tests in `crates/pos-core/tests/pricing.rs`: service charge skips exempt products and takeaway bills; cash rounds down while card pays the exact amount, a card review must equal the unrounded total, the drawer receives the rounded cash and a full refund equals the paid total; ++ prices add service then VAT, the VAT base plus VAT equals the total, and reports carry service charges and rounding. Domain tests cover rounding steps and modes, exact spreading and settings validation. Migration 011 (schema 11) adds `products.service_charge`.

Native captures on macOS (debug build, `seed_screens DIR --restaurant`: 10% service, cash rounded down to 0.25, household items exempt; `--preview`): Sell with service and rounding in Light Thai Compact 1280×820 and Dark English Touch 1000×700 (combined caption line); receipt with discounts, service and rounding in Light Thai 1280×820; Pricing card in Dark English at 1280×1300. These are preview states.

No legally mandated cash-rounding rule is claimed; the default rounds down in the customer's favour and accountant confirmation is recommended. Not verified: Windows, printer hardware.

## Custom roles — 2026-10-13

Automated: fmt, warnings-denied clippy, `cargo test --workspace` (115 tests) and a release build pass on macOS. New core test `custom_roles_allow_deny_or_need_approval` in `crates/pos-core/tests/staff.rs`: a custom role's discount limit; an allowed refund recorded with the `role_name` snapshot; cash out `Forbidden` even after an owner's approval; a shift lead approving a cashier's refund; no self-approval; `RoleInUse`; built-in role deletion refused. The existing staff tests were updated; domain tests cover role validation, the Approval default for missing entries and reading old role snapshots. Migration 012 (schema 12) adds `roles` and rebuilds `staff`.

Native captures on macOS (debug build, `seed_screens DIR --staff`, which now also adds the custom role หัวหน้ากะ held by a sample member; `--preview`): role editor in Light Thai Compact 1280×820 and Dark English Touch 1000×700 (after fixing duplicate widget IDs and alignment); a cashier's history detail with lock icons. These are preview states.

Not verified: Windows; clicking through creating, editing and deleting a role in the running UI (the logic is covered by the core tests).

## Recovery, CSV import, activity and estimated gross profit — 2026-10-13

Final macOS checks: formatting, warnings-denied workspace/all-targets Clippy, `cargo test --workspace --offline` **136 tests** (81 core/integration, 32 desktop, 23 domain), and debug native build pass. The existing local-TCP printer test requires localhost binding outside the restricted sandbox; its full rerun passed. `CARGO_INCREMENTAL=0` was used after regenerable incremental build cache exhausted disk space. No Windows runtime or release-build result is claimed for this increment.

New integration coverage: full backup/restore with shop and current-bill sidecars, device-setting retention, source/manifest checksum changes, future formats/schema, unsafe filenames, validation of old rows beyond the first UI page, interrupted activation, invalid stages preserving original data, safety-backup reopening, authorization before cart changes, and flushing both current and just-cleared bills. CSV covers quoted UTF-8/multiline fields, exact decimals/costs, 2 MiB/5,000 bounds, malformed/header/duplicate/current/archived conflicts, stale preview, permissions/approval attribution and stock/audit failure rollback. Activity covers persistence/filter/pagination, immutable staff/approver snapshots without PIN/hash, retries and transactional failure rollback. Profit covers unknown vs zero cost, catalog cost snapshots, migration from schema 12 with byte-identical legacy receipts, overflow rollback, service/options/++VAT/rounding, repeated partial refunds across days, exact VAT allocation and whole void reversal.

Native layout evidence: 28 debug captures in `.impeccable/review/increment-1-4/`, seven surfaces (`csv-import`, `csv-import-errors`, `backup-review`, `backup-confirm`, `activity`, `report-profit`, `product-cost`) in Light Thai Compact 1280×820, Dark English Touch 1000×700, Light English Touch 1280×820 and Dark Thai Compact 1000×700. Money remains grouped/right-aligned; narrow inspectors and import error lists scroll while actions stay accessible. These presets are layout evidence, not proof of clicked payments. A preview now waits for startup current-bill restoration before changing its state. The native egui platform does not use the web detector; existing Impeccable sidecar/platform-parser warnings were not treated as app defects and the token system was preserved.

Clicked through with CUA on isolated `/private/tmp/pos-heart-live-qa` data at 1280×820: imported the fictional CSV template (3 parsed products, 0 errors; Products 12 → 15), saw import/initial-stock events in Activity (`Alt 8`), created and inspected a real recovery folder (56 sales, 15 products, 3 held bills, schema 14), held the open bill with F4, changed/saved the fictional shop name after backup, then confirmed recovery. The prepared screen blocked further work and displayed the automatic safety-backup path. After closing/relaunching with the same explicit data directory, shop name returned to the backup value and Products showed 15; SQLite integrity was `ok`, counts were 56/15/3, and the previous directory plus a restorable safety manifest retained the post-backup shop name. This verifies an actual native prepare/restart/activate flow, not only a preview. The code also preserves printer/display settings; that part is covered by automated recovery tests rather than hardware interaction.

Still unverified: Windows build/runtime, physical touch/scanner/printer hardware, actual power failure/filesystem failure during the directory swap, a clicked-through recovery approval as a custom role, and a native end-to-end sale/refund using standard costs. Cost/payment/refund correctness is covered by core integration tests; gross profit remains a standard-cost estimate, not inventory valuation or net profit. Backup folders remain local until the operator copies them off-device; old standalone SQLite backups do not have the recovery-folder manifest.

## Tables and kitchen — 2026-10-13

Automated (macOS): fmt, warnings-denied clippy and `cargo test --workspace` **141 tests** pass. New integration tests in `crates/pos-core/tests/dining.rs` (3): one open bill per table, moving a bill by holding it on another table, refusing to switch off a busy table and duplicate active names in a zone; sends contain only differences, reductions produce cancellation tickets with an Activity `kitchen` event, bump/recall and line ticks persist; an autosave queued before a send cannot roll back the kitchen state, paying frees the table and an emptied sent bill produces a cancellation. Domain tests in `pos-domain/src/dining.rs` cover the kitchen diff and table validation. Migration 015 (schema 15) adds `dining_tables`, `kitchen_tickets` and `products.kitchen`; recovery validation reads the new tables and ticket JSON.

Native captures on macOS (debug build, `seed_screens DIR --tables`: zones ในร้าน T1–T8 and ระเบียง P1–P4, open bills with tickets on T2, T5 and P2, one unsent extra dish; `--page tables`, `--preview kitchen | table-bill | table-editor`): Tables floor in Light Thai Compact 1280×820 and Dark English Touch 1000×700; kitchen screen in Light Thai and Dark English Touch (after fixing a staircase layout to top-aligned wrapping cards); table bill in Sell in Light Thai 1280×820 and Dark Thai Touch 1000×700 (only one bill line visible there; the list scrolls); rendered kitchen ticket image (`print-preview` → `print-preview.kitchen.png`). These are preview states and rendered images.

Not verified: the kitchen screen as a real second OS window on a second monitor (captures embed it in the main window), any kitchen printer hardware, a clicked-through send/pay flow in the running UI (covered by core tests), and Windows.

## Print templates, preview and check bill — 2026-10-13

Automated on macOS: fmt, warnings-denied Clippy and `cargo test --workspace` (**144 tests**) pass. New desktop unit tests: template defaults and partial `templates` JSON (missing fields take defaults), logo import (resize to ≤268×160, 1-bit dither) and reload, and rendering of logo/QR placement (centred, QR about 45% of paper width) and text scaling by size.

Native captures on macOS (debug build, `seed_screens` sample data, presets `templates`, `templates-bill`, `templates-kitchen`, `receipt-preview`, `check-bill`): Printed documents card in Light Thai Compact at 1280×820 and the kitchen tab at 1000×700; Dark English Touch at 1280×820 (after fixing a collapsed preview column and an over-wide card); check bill preview with a PromptPay QR from Sell; receipt preview dialog in Dark English Touch at 1000×700; a receipt preview with an imported logo.

Not verified: printed output (logo, QR scanning, text sizes) on real printer hardware; drag-and-drop of image files on Windows; Windows generally.

## Split payment and split bill — 2026-10-14

Automated on macOS: three new integration tests in `crates/pos-core/tests/split.rs`: split payments reach each method while only the cash part reaches drawer expected (per-method report and shift buckets, cart-ID retry); split sales refund through their own methods (whole void per part, item refunds limited to a sale's method and its paid amount); splitting a table bill keeps the kitchen state and the table (nothing re-fires or cancels, retry returns the stored pair). Two domain unit tests cover `SplitPayment::normalized()` (merging, zero parts, fewer than two methods, cash given) and `split_cart` (units with their sent units, discounts, at least one unit moved and one left). No migration; receipts and voids without `tenders` read unchanged.

Native captures on macOS (debug build, `seed_screens` sample data, `--page sell --preview sell-split | sell-split-review | bill-split`) at 1280×820 in Dark Thai: the Sell footer in split mode; the split review dialog with a PromptPay QR for exactly the transfer part; the split-bill dialog. These are preview states, not a clicked-through payment.

Not verified: Windows; receipt printer and cash drawer hardware for split receipts and the drawer kick; scanning the transfer-part QR with a banking app; other palettes, languages, densities and 1000×700 for these surfaces.

## Credit notes — 2026-10-14

Automated on macOS: `crates/pos-core/tests/tax.rs::refunds_of_invoiced_sales_issue_numbered_credit_notes` (no note without a full invoice; a partial refund's note values 200 / 100 / 100 + VAT 7 = 107; a retried refund adds no second note; a second refund continues to a correct value of zero; a void's note numbers on; buyer and invoice references). The schema-version test now expects 16 and refuses 17; older fixtures drop `credit_notes`.

Native capture on macOS (debug build, `seed_screens DIR --vat`, which now refunds one item of the invoiced sale): `--page history --preview credit-note` (print preview of CN-000001, 80 mm, Dark Thai 1280×820; values 62.62 / 29.91 / 32.71 + VAT 2.29 = 35.00) and `--preview history-invoiced` (history detail with the TI and CN lines and the Credit note button).

Not verified: the layout against the Revenue Department's requirements by an accountant; printing on hardware; Windows; other palettes, languages and sizes.

## Promotions — 2026-10-14

Automated on macOS: three domain unit tests in `promotion.rs` (Bangkok date/weekday/minute incl. a leap day; best promotion per line, happy hour, manual discounts win; bill threshold, manual bill discount, coupons, max uses, end dates, days, midnight-wrapping hours, validation) and `crates/pos-core/tests/promotions.rs` (buy-2-get-1 plus spend-over at checkout with snapshots and use counts; outside the hours nothing applies; coupons need their code, are unique and stop when used up). The schema-version test expects 17 and refuses 18.

Native captures on macOS (debug build, `seed_screens DIR --restaurant --tables --promos`) at 1280×820 Dark Thai: Settings → Promotions list, the promotion editor (percent off a category, 14:00–17:00), Sell with the spend-over promotion in the discount row, and the coupon dialog.

Not verified: other palettes, languages, densities and 1000×700; a clicked-through checkout with a coupon; Windows.

## Development round: combined flows, automatic backups and idle lock — 2026-10-14

Development only, using fictional data on macOS. No real counter equipment is required for this round. Formatting, warnings-denied workspace/all-targets Clippy, debug build and workspace tests passed: **173 tests** (93 core/integration, 50 desktop, 30 domain). The local-TCP printer simulation needs loopback access outside the restricted sandbox; the complete final run passed with that access. `CARGO_INCREMENTAL=0` kept regenerable compiler-cache growth bounded. No schema migration or network service was added.

`crates/pos-core/tests/development_flow.rs` adds three combined regressions: sent table/options/coupon/service/VAT → split bill preserving kitchen state → split cash/transfer payment → full invoice → cross-shift, next-Bangkok-day partial refunds and credit notes → close/retry → backup/reopen; injected credit-note and late coupon-use failures roll back money, stock, ledger, history and held kitchen state before a successful idempotent retry. Original receipt bytes remain unchanged. The combined scenario asserts sale-day revenue 582.61 / VAT 38.11 / covered gross profit 143.25, refund-day amounts 466.09 / VAT 30.49 / profit −114.60, and exact method/drawer allocation.

Seven automatic-backup tests cover bounded/versioned config, Bangkok-day and shift deduplication after restart, last verified snapshot status (also after disabling scheduling), opt-in maintenance while staff are locked, interactive authorization, selective retention, damaged/orphan/manual/safety preservation, refusal of symlinks or future/broken metadata and failure before pruning. Four idle-policy tests cover monotonic deadlines, native input versus background activity, disabled/no-staff behavior and future/corrupt config preservation. Three worker tests cover revocation with a full queue, stale writes/SignIn/approval, preserved autosaves, and stale sign-in replies. Recovery also verifies stale paid-bill marker omission and destination device-policy preservation.

Native debug layout captures in `.impeccable/review/development-round/` cover split payment, split review, split bill, credit-note preview, promotion editor, coupon dialog, ordinary cash payment, automatic backup settings, idle settings and locked state. Four variants: Light Thai Compact 1280×820, Dark English Touch 1000×700, Light English Touch 1280×820 and Dark Thai Compact 1000×700. These exercise each palette/density/language/size, not every Cartesian combination. The short Touch sale view initially clipped payment controls and overlapped cart rows; the final footer pins the exact total and commit action, scrolls payment details separately and clips rows to their allocated area. All affected captures were replaced and inspected. Modals now suppress background toasts so Cancel/Confirm remain visible. Long Settings sections scroll; the automatic-backup footer was also inspected after scrolling in the live 1000×700 Touch window. Debug idle-lock captures age the monotonic timer deliberately; they alone do not prove a wall-clock wait or clicked sign-in.

Clicked native CUA flow on isolated `/private/tmp/pos-heart-dev-live`, Dark English Touch 1000×700: saved opt-in daily/close-shift policy (keep 7), used Back up now/retry and saw the resulting `.posbackup` path. Independent SQLite integrity check returned `ok`, schema 17; index records one owned snapshot for the Bangkok day. Home subsequently omitted the false “No backup” warning. Saved a one-minute idle policy. Manual Lock/switch user hid workspace contents; entering the seeded owner's existing PIN restored the same 9-item bill (3307.75), cash draft 4000.00 and change 692.25 without recording a payment. After leaving the native window idle beyond the configured minute, the actual timer also showed the lock screen; signing in again restored those same values. Tab moved keyboard focus to an accessible product action. This complements the artificially aged debug captures.

Still deferred: physical printer/scanner/drawer/touch equipment, Windows runtime, real power loss, bank QR verification, and a clicked native combined checkout/refund scenario (the combined financial path is verified by core tests). Automatic backups capture the last committed/autosaved state while the app is open, remain local, and do not flush unsaved UI editors or replace off-device copies. Idle lock gates the application session; it does not encrypt or protect the SQLite file from direct access.

## Ingredients and recipes — 2026-10-14

Automated on macOS: domain unit tests in `ingredient.rs` (recipe use per ingredient, half-unit cost, recipe validation, removal below zero, request validation), `format::qty`/`parse_qty` round trips, and `crates/pos-core/tests/ingredients.rs` (unknown ingredient refused in a recipe; duplicate names refused; three plates deduct rice and take eggs below zero without refusing the sale; a checkout retry deducts nothing more; sale movements carry the sale ID; a partial refund keeps the use; a void gives it back; receive retry, count, and edits never changing stock). The schema-version test expects 18 and refuses 19; older fixtures drop the new tables and column. Full workspace: 177 tests.

Native captures on macOS (debug build, `seed_screens DIR --restaurant --ingredients`) in Dark Thai: Products → Ingredients at 1280×820 (low badge, SideNav count), the open history row, and the product editor's recipe section at 1280×1500.

Not verified: a clicked-through sale and void updating the list; other palettes, languages, densities and 1000×700; Windows.

## New surfaces in Light English Touch 1000×700 — 2026-10-14

Native captures on macOS (debug build, `seed_screens DIR --restaurant --tables --promos --vat --ingredients`, Light, English, Touch, 1000×700): `sell-split`, `bill-split`, `sell-split-review`, `settings-promotions`, `promo-editor`, `sell-coupon`, `ingredients` and `credit-note`. Found and fixed: on this short window the split footer pushed the cash part below the scrollable payment details; compact windows now put Transfer and Card side by side and drop the equal-shares caption, so the cash part shows without scrolling (the cash-received field may still need a short scroll inside the bounded footer). Also seen: the Ingredients item sits at the bottom of the Products SideNav and needs a scroll at this size.

Not verified: Windows; hardware; clicked-through flows.

## Option recipes and promotion/ingredient reports — 2026-10-14

Automated on macOS: `ingredient.rs::option_recipes_add_and_remove_without_going_below_zero`, signed-quantity format tests, `tests/ingredients.rs::option_choices_change_what_a_plate_uses` (plain, + egg and no-egg lines deduct 2 + 2 + 0 eggs; a void gives them back; rows naming another product's choice are refused) and report assertions in `tests/ingredients.rs` (net rice use after sale, partial refund and void) and `tests/promotions.rs` (bills and amounts per promotion, removed again by a void). Full workspace: 179 tests. The schema-version test expects 19 and refuses 20.

Native captures on macOS (debug build, `seed_screens DIR --restaurant --ingredients --promos`, which now sells two basil-rice plates and one spend-over bill): `product-recipe` in Dark Thai 1280×820 scrolled to the recipe and its "Adjust by option" rows; `report-promos` in Dark Thai 1280×820; `report-ingredients` in Light English 1000×700.

Not verified: Touch density for these; Windows. Seen, not changed: at 1000×700 the report's profit rows touch the right edge next to the page scrollbar (existing layout).
