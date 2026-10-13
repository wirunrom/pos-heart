# Shifts and cash drawer

Mode: Operate. Follows `design/` (POS Design System v0.1, ScreenShiftClose) and `desktop.md`; preserves every shift invariant in AGENTS.md / docs/architecture.md.

Entry: workspace switcher (Alt 4), the shift status pill, Home's Shift & drawer card, or Sell's "Open a shift before taking payment" gate (which keeps the current bill).

No open shift: Open a new shift card (opening float money field, zero allowed, Open shift primary) beside the latest closed shift summary.

Open shift header: "Shift #N · opened HH:MM · duration" with tabs Overview / Count & close / Shift history.
- Overview: Drawer cash statement (opening, cash sales, cash refunds, cash in, cash out → expected) and Shift sales — not drawer cash (gross, refunds, net, per-method line); cash in/out form (segmented, amount, required reason, primary record button) and the paginated shift ledger (noncash entries muted and labelled as payment records).
- Count & close: Steps (Count → Reconcile → Confirm). Left: denomination table (1,000/500/100/50/20 notes, 10/5/2/1 coins, satang/other) with live amounts and counted total; draft autosaved per shift with time. Right: shift sales, drawer statement with counted, signed difference highlighted (short / over / balanced), reason field required when not balanced, Back to count and Close shift. Confirmation dialog names expected, counted, difference and reason. An open unpaid bill blocks closing.
- Shift history: numbered closed shifts with signed difference and status; read-only detail (summary, reason, breakdown, statements, ledger) with Print summary when a printer is set up.
- Printing: with a printer set up, the close confirmation reads "Close & print summary" and the summary (drawer statement, counted/difference, denominations, sales by method) prints after the close commits. A print failure shows the warning banner with Print again; the closed snapshot is unaffected.

Rules: drawer cash and daily/shift sales are separate values; only cash affects expected. Retries, revision and clock guards are enforced by the core. One local register. With staff sign-in, opening/closing records who did it (shown in shift detail and the printed summary) and cash in/out follows the role: a lock icon with "Needs approval" for Approval, disabled with "Your role cannot do this" for Deny (default cashier: cash in/out needs approval, closing a shift is allowed). A cash drawer is only reachable as a pulse through the receipt printer on cash payments; there is no drawer sensor.

Checks: 1000×700 and 1280×820, Light/Dark, Thai/English, Compact/Touch; open, cash in/out, cash sale minus change, short/over/balanced count with and without reason, confirmation Back, closed history, restart with a count draft. No Windows or physical-touch claims.
